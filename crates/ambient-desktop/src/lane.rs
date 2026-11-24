//! Folder work for the window.
//!
//! The caret's thread paints and takes keys. Two threads touch the folder.
//! One only pulls the conversation on screen. The other reads notes, builds
//! the group list, and runs a full sync. A third thread only checks spelling.
//! It never takes a lock. Notes and conversations use different locks. A full
//! sync takes the notes lock first, then the chat lock, so a save cannot land
//! in the middle of a sync. A result is sent when it is finished. The screen
//! keeps what it already drew until then.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use ambient_cli::{ChatMessage, NoteSummary, Notebook};
use iced::futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use iced::futures::stream::{self, Stream, StreamExt};

use super::markdown;

static INCOMING: Mutex<Option<UnboundedReceiver<Done>>> = Mutex::new(None);
static FRESH: Mutex<Option<(u64, Notebook)>> = Mutex::new(None);
static SAVED: Mutex<Option<(u64, Notebook)>> = Mutex::new(None);
static OPEN_SIDE: Mutex<Option<SideDone>> = Mutex::new(None);
static OPEN_INBOX: Mutex<Option<Vec<ChatRow>>> = Mutex::new(None);
static WATCH: Mutex<Option<String>> = Mutex::new(None);
static BOOK_GEN: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug)]
pub(super) struct NoteFile {
    pub id: String,
    pub title: String,
    pub body: String,
}

#[derive(Clone, Debug, Default)]
pub(super) struct OpenSnap {
    pub selected: Option<String>,
    pub title: String,
    pub stored: String,
    pub unsaved: Vec<(Option<String>, String, String)>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct SideDone {
    /// The notes the open note links to, as id and title.
    pub links_to: Vec<(String, String)>,
    /// Every link between two notes, from id to id. Drawn by the graph.
    pub edges: Vec<(String, String)>,
    pub backlinks: Vec<(String, String)>,
    pub known: BTreeSet<String>,
}

/// One conversation for the list. Built off the caret's thread.
#[derive(Clone, Debug)]
pub(super) struct ChatRow {
    pub id: String,
    pub label: String,
    pub other: String,
    pub members: usize,
    pub started: bool,
    pub others: Vec<String>,
    pub preview: String,
    pub time: String,
    pub count: usize,
}

pub(super) enum Job {
    Hello(PathBuf),
    Watch(Option<String>),
    Pull,
    Send {
        id: String,
        text: String,
        files: Vec<PathBuf>,
        cite: Option<String>,
    },
    Sync {
        writes: u64,
    },
    Side(OpenSnap),
    Search(String),
    Inbox,
    Proof(String),
    Version {
        id: String,
        version: String,
    },
}

#[derive(Clone, Debug)]
pub(super) enum Done {
    Synced {
        detail: String,
        failed: bool,
        transport: bool,
        pending: bool,
        writes: u64,
        notes: Vec<NoteSummary>,
        side: SideDone,
    },
    Chat {
        id: String,
        messages: Vec<ChatMessage>,
    },
    Inbox(Vec<ChatRow>),
    Found {
        query: String,
        notes: Vec<NoteHit>,
        chats: Vec<ChatHit>,
    },
    Proof {
        text: String,
        warning: Option<String>,
        words: BTreeSet<String>,
    },
    Version {
        id: String,
        title: String,
        body: String,
        current: bool,
    },
    SendFailed {
        text: String,
        message: String,
    },
    Side(SideDone),
    Notice(String),
}

/// One note the search found, with the line that matched.
#[derive(Clone, Debug)]
pub(super) struct NoteHit {
    pub note: NoteSummary,
    pub line: String,
    /// Line index of `line` in the note. Zero when nothing matched.
    pub at: usize,
}

/// One conversation line the search found.
#[derive(Clone, Debug)]
pub(super) struct ChatHit {
    pub id: String,
    pub label: String,
    pub line: String,
}

/// One conversation the search can scan without opening the log again.
struct CorpusRow {
    id: String,
    label: String,
    lines: Vec<String>,
}

enum ChatWork {
    Hello(PathBuf),
    Watch(Option<String>),
    Pull,
    Send {
        id: String,
        text: String,
        files: Vec<PathBuf>,
        cite: Option<String>,
    },
    Inbox,
}

enum FolderWork {
    Hello(PathBuf),
    Side(OpenSnap),
    Sync { writes: u64 },
    Search(String),
    Version { id: String, version: String },
}

pub(super) fn start() -> std::sync::mpsc::Sender<Job> {
    let (done_tx, done_rx) = unbounded();
    *INCOMING.lock().unwrap_or_else(PoisonError::into_inner) = Some(done_rx);
    let (work_tx, work_rx) = std::sync::mpsc::channel();
    let (chat_tx, chat_rx) = std::sync::mpsc::channel();
    let (folder_tx, folder_rx) = std::sync::mpsc::channel();
    let (proof_tx, proof_rx) = std::sync::mpsc::channel();
    let chat_done = done_tx.clone();
    let proof_done = done_tx.clone();
    std::thread::spawn(move || route(&work_rx, &chat_tx, &folder_tx, &proof_tx));
    std::thread::spawn(move || chat_run(&chat_rx, &chat_done));
    std::thread::spawn(move || proof_run(&proof_rx, &proof_done));
    std::thread::spawn(move || folder_run(&folder_rx, &done_tx));
    work_tx
}

pub(super) fn listen() -> impl Stream<Item = Done> + Send {
    let rx = INCOMING
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take();
    stream::unfold(rx, |mut rx| async move {
        let next = rx.as_mut()?.next().await?;
        Some((next, rx))
    })
}

pub(super) fn park_side(side: SideDone) {
    *OPEN_SIDE.lock().unwrap_or_else(PoisonError::into_inner) = Some(side);
}

pub(super) fn take_side() -> Option<SideDone> {
    OPEN_SIDE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take()
}

pub(super) fn park_inbox(rows: Vec<ChatRow>) {
    *OPEN_INBOX.lock().unwrap_or_else(PoisonError::into_inner) = Some(rows);
}

pub(super) fn take_inbox() -> Option<Vec<ChatRow>> {
    OPEN_INBOX
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take()
}

pub(super) fn watched() -> Option<String> {
    WATCH.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

fn stamp_book(book: Notebook) -> (u64, Notebook) {
    let generation = BOOK_GEN.fetch_add(1, Ordering::Relaxed);
    (generation, book)
}

/// The notebook a save just wrote. The window installs it only if nothing newer arrived.
pub(super) fn publish_saved(book: Notebook) {
    *SAVED.lock().unwrap_or_else(PoisonError::into_inner) = Some(stamp_book(book));
}

fn publish_fresh(book: Notebook) {
    *FRESH.lock().unwrap_or_else(PoisonError::into_inner) = Some(stamp_book(book));
}

/// Newest notebook newer than `have`. Drops an older one so it cannot be installed later.
pub(super) fn take_latest(have: u64) -> Option<(u64, Notebook)> {
    let fresh = FRESH.lock().unwrap_or_else(PoisonError::into_inner).take();
    let saved = SAVED.lock().unwrap_or_else(PoisonError::into_inner).take();
    let best = match (fresh, saved) {
        (Some(left), Some(right)) => Some(if left.0 >= right.0 { left } else { right }),
        (Some(one), None) | (None, Some(one)) => Some(one),
        (None, None) => None,
    };
    best.filter(|(generation, _)| *generation > have)
}

pub(super) fn files_of(book: &Notebook) -> Vec<NoteFile> {
    let mut files = Vec::new();
    let _ = book.visit_held(|id, title, body, conflict| {
        if conflict {
            return;
        }
        files.push(NoteFile {
            id: id.to_owned(),
            title: title.to_owned(),
            body: body.to_owned(),
        });
    });
    files
}

pub(super) fn compute(files: &[NoteFile], open: &OpenSnap) -> SideDone {
    let rows = overlay(files, open);
    // What each note links to, read once. The reading pieces and the graph both use it.
    let targets_of: Vec<Vec<String>> = rows
        .iter()
        .map(|row| {
            let (prose, _) = ambient_cli::split_note_tags(&row.body);
            markdown::chapter_names(&prose)
        })
        .collect();
    let mut edges: Vec<(String, String)> = Vec::new();
    for (row, targets) in rows.iter().zip(&targets_of) {
        for target in targets {
            if let Some(other) = rows.iter().find(|other| same_piece(target, other))
                && other.id != row.id
                && !edges
                    .iter()
                    .any(|(from, to)| *from == row.id && *to == other.id)
            {
                edges.push((row.id.clone(), other.id.clone()));
            }
        }
    }
    let title_of = |id: &str| {
        rows.iter()
            .find(|row| row.id == id)
            .map(|row| {
                if row.title.is_empty() {
                    "Untitled".to_owned()
                } else {
                    row.title.clone()
                }
            })
            .unwrap_or_default()
    };
    let links_to = open
        .selected
        .as_ref()
        .map(|id| {
            edges
                .iter()
                .filter(|(from, _)| from == id)
                .map(|(_, to)| (to.clone(), title_of(to)))
                .collect()
        })
        .unwrap_or_default();
    let mut known = BTreeSet::new();
    for row in &rows {
        known.insert(row.id.clone());
        if !row.title.is_empty() {
            known.insert(row.title.to_lowercase());
        }
    }
    let backlinks = backlinks_of(&rows, open);
    SideDone {
        links_to,
        edges,
        backlinks,
        known,
    }
}

/// Conversation list. The caller holds the chat lock.
pub(super) fn inbox_of(dir: &Path) -> Vec<ChatRow> {
    let Ok(chats) = ambient_cli::list_conversations(dir) else {
        return Vec::new();
    };
    let tz = super::clock::load_zone().0;
    let now = super::clock::now_ms();
    chats
        .into_iter()
        .map(|chat| {
            let messages = ambient_cli::chat_messages_in(dir, &chat.id).unwrap_or_default();
            let count = messages.len();
            let (preview, time) = messages.last().map_or_else(
                || (String::new(), String::new()),
                |last| {
                    let when = super::clock::when(last.unix_ms, &tz, now);
                    (super::chat_preview(&last.text), when.time)
                },
            );
            ChatRow {
                id: chat.id,
                label: chat.label,
                other: chat.other,
                members: chat.members,
                started: chat.started,
                others: chat.others,
                preview,
                time,
                count,
            }
        })
        .collect()
}

fn route(
    rx: &Receiver<Job>,
    chat: &Sender<ChatWork>,
    folder: &Sender<FolderWork>,
    proof: &Sender<String>,
) {
    while let Ok(job) = rx.recv() {
        match job {
            Job::Hello(path) => {
                let _ = chat.send(ChatWork::Hello(path.clone()));
                let _ = folder.send(FolderWork::Hello(path));
            }
            Job::Watch(id) => {
                WATCH
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .clone_from(&id);
                let _ = chat.send(ChatWork::Watch(id));
            }
            Job::Pull => {
                let _ = chat.send(ChatWork::Pull);
            }
            Job::Send {
                id,
                text,
                files,
                cite,
            } => {
                let _ = chat.send(ChatWork::Send {
                    id,
                    text,
                    files,
                    cite,
                });
            }
            Job::Sync { writes } => {
                let _ = folder.send(FolderWork::Sync { writes });
            }
            Job::Side(snap) => {
                let _ = folder.send(FolderWork::Side(snap));
            }
            Job::Search(query) => {
                let _ = folder.send(FolderWork::Search(query));
            }
            Job::Proof(text) => {
                let _ = proof.send(text);
            }
            Job::Version { id, version } => {
                let _ = folder.send(FolderWork::Version { id, version });
            }
            Job::Inbox => {
                let _ = chat.send(ChatWork::Inbox);
            }
        }
    }
}

fn chat_run(rx: &Receiver<ChatWork>, tx: &UnboundedSender<Done>) {
    let mut dir: Option<PathBuf> = None;
    let mut watch: Option<String> = None;
    let mut next_pull = Instant::now();
    loop {
        let wait = if watch.is_some() {
            next_pull.saturating_duration_since(Instant::now())
        } else {
            Duration::from_hours(1)
        };
        let mut batch = Vec::new();
        match rx.recv_timeout(wait) {
            Ok(job) => batch.push(job),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        while let Ok(job) = rx.try_recv() {
            batch.push(job);
        }
        let mut pull = false;
        let mut force = false;
        let mut inbox = false;
        for job in batch {
            match job {
                ChatWork::Hello(path) => {
                    dir = Some(path);
                    inbox = true;
                }
                ChatWork::Watch(id) => {
                    if watch != id {
                        watch = id;
                        next_pull = Instant::now();
                        force = watch.is_some();
                        inbox = watch.is_some();
                    }
                }
                ChatWork::Pull => {
                    pull = true;
                    force = true;
                }
                ChatWork::Send {
                    id,
                    text,
                    files,
                    cite,
                } => {
                    send_one(dir.as_deref(), &id, &text, &files, cite.as_deref(), tx);
                    next_pull = Instant::now() + Duration::from_millis(350);
                }
                ChatWork::Inbox => inbox = true,
            }
        }
        if inbox && let Some(path) = dir.clone() {
            publish_inbox(&path, tx);
        }
        if pull || (watch.is_some() && Instant::now() >= next_pull) {
            if let Some(id) = watch.clone() {
                pull_one(dir.as_deref(), &id, force, tx);
            }
            next_pull = Instant::now() + Duration::from_millis(350);
        }
    }
}

fn folder_run(rx: &Receiver<FolderWork>, tx: &UnboundedSender<Done>) {
    let mut dir: Option<PathBuf> = None;
    let mut book: Option<Notebook> = None;
    let mut snap = OpenSnap::default();
    let mut chats: Option<(u64, Vec<CorpusRow>)> = None;
    loop {
        let mut batch = Vec::new();
        match rx.recv() {
            Ok(job) => batch.push(job),
            Err(_) => break,
        }
        while let Ok(job) = rx.try_recv() {
            batch.push(job);
        }
        let mut sync: Option<u64> = None;
        let mut side = false;
        let mut search: Option<String> = None;
        let mut version: Option<(String, String)> = None;
        for job in batch {
            match job {
                FolderWork::Hello(path) => {
                    if dir.as_ref() != Some(&path) {
                        book = None;
                        chats = None;
                    }
                    dir = Some(path);
                }
                FolderWork::Side(next) => {
                    snap = next;
                    side = true;
                }
                FolderWork::Sync { writes } => sync = Some(writes),
                FolderWork::Search(query) => search = Some(query),
                FolderWork::Version { id, version: at } => version = Some((id, at)),
            }
        }
        if side {
            publish_side(dir.as_deref(), &mut book, &snap, tx);
        }
        if let Some(query) = search {
            search_one(dir.as_deref(), &mut book, &mut chats, &query, tx);
        }
        if let Some(writes) = sync {
            full_sync(dir.as_deref(), &mut book, &snap, writes, tx);
        }
        if let Some((id, at)) = version {
            version_one(dir.as_deref(), &mut book, &id, &at, tx);
        }
    }
}

fn send_one(
    dir: Option<&Path>,
    id: &str,
    text: &str,
    files: &[PathBuf],
    cite: Option<&str>,
    tx: &UnboundedSender<Done>,
) {
    let Some(dir) = dir else {
        return;
    };
    let wrote = {
        let _guard = super::chat_lock();
        (|| {
            let mut body = text.to_owned();
            for path in files {
                let cite = super::files::store(dir, id, path)?;
                if !body.is_empty() && !body.ends_with('\n') {
                    body.push('\n');
                }
                body.push_str(&cite);
            }
            if body.trim().is_empty() {
                return Err("Write a message, or add a file.".to_owned());
            }
            ambient_cli::send_chat_citing(dir, id, &body, cite).map_err(|err| err.to_string())?;
            Ok(())
        })()
    };
    if let Err(err) = wrote {
        let _ = tx.unbounded_send(Done::SendFailed {
            text: text.to_owned(),
            message: err,
        });
        return;
    }
    push_chat(dir, id, tx);
    publish_inbox(dir, tx);
    {
        let _guard = super::chat_lock();
        let _ = ambient_cli::sync_conversation(dir, id);
    }
    push_chat(dir, id, tx);
    publish_inbox(dir, tx);
}

fn pull_one(dir: Option<&Path>, id: &str, force: bool, tx: &UnboundedSender<Done>) {
    let Some(dir) = dir else {
        return;
    };
    if force {
        push_chat(dir, id, tx);
    }
    let detail = {
        let _guard = super::chat_lock();
        ambient_cli::sync_conversation(dir, id).map_err(|err| err.to_string())
    };
    match detail {
        Ok(text) if text != "Nothing new." => {
            push_chat(dir, id, tx);
            publish_inbox(dir, tx);
        }
        Ok(_) | Err(_) => {}
    }
}

fn push_chat(dir: &Path, id: &str, tx: &UnboundedSender<Done>) {
    let _chat = super::chat_lock();
    let Ok(messages) = ambient_cli::chat_messages_in(dir, id) else {
        return;
    };
    let _ = tx.unbounded_send(Done::Chat {
        id: id.to_owned(),
        messages,
    });
}

fn publish_inbox(dir: &Path, tx: &UnboundedSender<Done>) {
    let _chat = super::chat_lock();
    let _ = tx.unbounded_send(Done::Inbox(inbox_of(dir)));
}

fn publish_side(
    dir: Option<&Path>,
    book: &mut Option<Notebook>,
    snap: &OpenSnap,
    tx: &UnboundedSender<Done>,
) {
    let Some(dir) = dir else {
        return;
    };
    let Ok(files) = files_cached(dir, book) else {
        return;
    };
    let _ = tx.unbounded_send(Done::Side(compute(&files, snap)));
}

fn search_one(
    dir: Option<&Path>,
    book: &mut Option<Notebook>,
    chats: &mut Option<(u64, Vec<CorpusRow>)>,
    query: &str,
    tx: &UnboundedSender<Done>,
) {
    let Some(dir) = dir else {
        return;
    };
    let found = {
        let _notes = super::notes_lock();
        if book.is_none() {
            match Notebook::open(dir) {
                Ok(opened) => *book = Some(opened),
                Err(err) => {
                    let _ = tx.unbounded_send(Done::Notice(err.to_string()));
                    return;
                }
            }
        }
        book.as_ref()
            .map(|opened| opened.search(query).unwrap_or_default())
            .unwrap_or_default()
    };
    let mut notes = Vec::new();
    if let Some(opened) = book.as_ref() {
        for note in found {
            let (at, line) = opened
                .show_held(&note.id)
                .ok()
                .and_then(|shown| {
                    shown
                        .heads
                        .first()
                        .map(|head| snippet_line(&head.body, query))
                })
                .unwrap_or((0, String::new()));
            notes.push(NoteHit { note, line, at });
        }
    }
    let chats = chat_matches(dir, chats, query);
    let _ = tx.unbounded_send(Done::Found {
        query: query.to_owned(),
        notes,
        chats,
    });
}

fn proof_one(text: &str, tx: &UnboundedSender<Done>) {
    let warning = markdown::broken_mark(text);
    let words = markdown::misspellings(text);
    let _ = tx.unbounded_send(Done::Proof {
        text: text.to_owned(),
        warning,
        words,
    });
}

fn proof_run(rx: &Receiver<String>, tx: &UnboundedSender<Done>) {
    while let Ok(mut text) = rx.recv() {
        while let Ok(next) = rx.try_recv() {
            text = next;
        }
        proof_one(&text, tx);
    }
}

fn version_one(
    dir: Option<&Path>,
    book: &mut Option<Notebook>,
    id: &str,
    version: &str,
    tx: &UnboundedSender<Done>,
) {
    let Some(dir) = dir else {
        return;
    };
    {
        let _notes = super::notes_lock();
        if book.is_none() {
            match Notebook::open(dir) {
                Ok(opened) => *book = Some(opened),
                Err(err) => {
                    let _ = tx.unbounded_send(Done::Notice(err.to_string()));
                    return;
                }
            }
        }
        if let Some(opened) = book.as_ref()
            && let Err(err) = opened.warm()
        {
            let _ = tx.unbounded_send(Done::Notice(err.to_string()));
            return;
        }
    }
    let found = book
        .as_ref()
        .and_then(|opened| opened.snapshot_held(id, version).ok().flatten());
    match found {
        Some(snap) => {
            let _ = tx.unbounded_send(Done::Version {
                id: id.to_owned(),
                title: snap.title,
                body: snap.body,
                current: snap.current,
            });
        }
        None => {
            let _ = tx.unbounded_send(Done::Notice(
                "That version is not in this folder.".to_owned(),
            ));
        }
    }
}

fn snippet_line(body: &str, query: &str) -> (usize, String) {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return (0, String::new());
    }
    for (index, line) in body.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("%%ambient-tags:") {
            continue;
        }
        if trimmed.to_lowercase().contains(&needle) {
            return (index, clip_line(trimmed));
        }
    }
    (0, String::new())
}

fn clip_line(line: &str) -> String {
    let mut out = String::new();
    for ch in line.chars() {
        if out.chars().count() >= 80 {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}

fn chat_matches(
    dir: &Path,
    chats: &mut Option<(u64, Vec<CorpusRow>)>,
    query: &str,
) -> Vec<ChatHit> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }
    refresh_corpus(dir, chats);
    if let Some((_, rows)) = chats.as_ref() {
        return hits_in(rows, &needle);
    }
    let _chat = super::chat_lock();
    hits_in(&load_corpus(dir), &needle)
}

fn refresh_corpus(dir: &Path, chats: &mut Option<(u64, Vec<CorpusRow>)>) {
    let generation = ambient_cli::conversation_generation(dir);
    let fresh = match (chats.as_ref(), generation) {
        (Some((have, _)), Some(now)) => *have == now,
        _ => false,
    };
    if fresh {
        return;
    }
    let _chat = super::chat_lock();
    let generation = ambient_cli::conversation_generation(dir);
    let Some(generation) = generation else {
        *chats = None;
        return;
    };
    *chats = Some((generation, load_corpus(dir)));
}

/// The caller holds the chat lock.
fn load_corpus(dir: &Path) -> Vec<CorpusRow> {
    let Ok(chats) = ambient_cli::list_conversations(dir) else {
        return Vec::new();
    };
    chats
        .into_iter()
        .map(|chat| {
            let lines = ambient_cli::chat_messages_in(dir, &chat.id)
                .unwrap_or_default()
                .into_iter()
                .map(|message| message.text)
                .collect();
            CorpusRow {
                id: chat.id,
                label: chat.label,
                lines,
            }
        })
        .collect()
}

fn hits_in(rows: &[CorpusRow], needle: &str) -> Vec<ChatHit> {
    let mut hits = Vec::new();
    for row in rows {
        let Some(line) = row.lines.iter().find_map(|text| {
            text.lines()
                .find(|line| line.to_lowercase().contains(needle))
                .map(clip_line)
        }) else {
            continue;
        };
        hits.push(ChatHit {
            id: row.id.clone(),
            label: row.label.clone(),
            line,
        });
    }
    hits
}

fn full_sync(
    dir: Option<&Path>,
    book: &mut Option<Notebook>,
    snap: &OpenSnap,
    writes: u64,
    tx: &UnboundedSender<Done>,
) {
    let Some(dir) = dir else {
        return;
    };
    let notes_guard = super::notes_lock();
    let chat_guard = super::chat_lock();
    let outcome = super::sync_outcome(dir);
    if outcome.failed {
        let _ = tx.unbounded_send(Done::Synced {
            detail: outcome.detail,
            failed: true,
            transport: outcome.transport,
            pending: true,
            writes,
            notes: Vec::new(),
            side: SideDone::default(),
        });
        return;
    }
    if !super::content_arrived(&outcome.detail) {
        let _ = tx.unbounded_send(Done::Synced {
            detail: outcome.detail,
            failed: false,
            transport: false,
            pending: outcome.pending,
            writes,
            notes: Vec::new(),
            side: SideDone::default(),
        });
        return;
    }
    let loaded = Notebook::open(dir)
        .map_err(|err| err.to_string())
        .map(|opened| {
            let notes = opened.list_held().unwrap_or_default();
            let files = files_of(&opened);
            (opened, notes, files)
        });
    let (notes, files) = match loaded {
        Ok((opened, notes, files)) => {
            *book = None;
            publish_fresh(opened);
            (notes, files)
        }
        Err(err) => {
            let _ = tx.unbounded_send(Done::Notice(err));
            return;
        }
    };
    let pending = ambient_cli::has_unsent(dir).unwrap_or(true) || outcome.pending;
    let detail = outcome.detail;
    drop(chat_guard);
    drop(notes_guard);
    let side = compute(&files, snap);
    if super::content_arrived(&detail) {
        publish_inbox(dir, tx);
    }
    let _ = tx.unbounded_send(Done::Synced {
        detail,
        failed: false,
        transport: false,
        pending,
        writes,
        notes,
        side,
    });
}

fn files_cached(dir: &Path, book: &mut Option<Notebook>) -> Result<Vec<NoteFile>, String> {
    let _notes = super::notes_lock();
    if book.is_none() {
        *book = Some(Notebook::open(dir).map_err(|err| err.to_string())?);
    }
    let Some(opened) = book.as_ref() else {
        return Err("folder is not open".to_owned());
    };
    // A save in between changed the log. Project it here, not on the caret's thread.
    opened
        .list()
        .map_err(|err| err.to_string())
        .map(|_| files_of(opened))
}

fn overlay(files: &[NoteFile], open: &OpenSnap) -> Vec<NoteFile> {
    let mut rows = files.to_vec();
    for (key, title, body) in &open.unsaved {
        let Some(id) = key else { continue };
        if let Some(row) = rows.iter_mut().find(|row| &row.id == id) {
            row.title.clone_from(title);
            row.body.clone_from(body);
        }
    }
    if let Some(id) = &open.selected {
        if let Some(row) = rows.iter_mut().find(|row| &row.id == id) {
            row.title.clone_from(&open.title);
            row.body.clone_from(&open.stored);
        } else {
            rows.push(NoteFile {
                id: id.clone(),
                title: open.title.clone(),
                body: open.stored.clone(),
            });
        }
    }
    rows
}

fn same_piece(target: &str, note: &NoteFile) -> bool {
    target == note.id || (!note.title.is_empty() && target.eq_ignore_ascii_case(&note.title))
}

fn backlinks_of(rows: &[NoteFile], open: &OpenSnap) -> Vec<(String, String)> {
    let Some(id) = &open.selected else {
        return Vec::new();
    };
    let title = open.title.clone();
    let mut out = Vec::new();
    for row in rows {
        if &row.id == id {
            continue;
        }
        let (prose, _) = ambient_cli::split_note_tags(&row.body);
        let hit = markdown::chapter_names(&prose).iter().any(|target| {
            target == id || (!title.is_empty() && target.eq_ignore_ascii_case(&title))
        });
        if hit {
            let label = if row.title.is_empty() {
                "Untitled".to_owned()
            } else {
                row.title.clone()
            };
            out.push((row.id.clone(), label));
        }
    }
    out
}
