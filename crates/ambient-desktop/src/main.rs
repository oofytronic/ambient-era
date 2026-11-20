//! Linux notes window.
//!
//! Iced draws the controls in a system window. There is no webview. The folder
//! is the same one the `ambient` CLI uses. Pass a path, or it uses `./notebook`.
//! Sync, pairing, and chat call `ambient-cli`. This file does not seal events.
//! The relay it can start is cleartext on `127.0.0.1` only.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ambient_cli::{Draft, NoteSummary, Notebook};
use iced::clipboard;
use iced::keyboard::Key;
use iced::widget::canvas;
use iced::widget::image as iced_image;
use iced::widget::{
    button, checkbox, column, container, mouse_area, row, scrollable, space, stack, text,
    text_editor, text_input, tooltip,
};
use iced::{Color, Element, Fill, Font, Size, Task, Theme};
use image::ImageEncoder;
use qrcode::QrCode;

mod clock;
mod files;
mod lane;
mod markdown;
mod profile;

use markdown::FormatKind;

const SANS_BYTES: &[u8] = include_bytes!("../fonts/DMSans-Regular.ttf");
const SANS_BOLD_BYTES: &[u8] = include_bytes!("../fonts/DMSans-Bold.ttf");
const SANS_ITALIC_BYTES: &[u8] = include_bytes!("../fonts/DMSans-Italic.ttf");
const SERIF_BYTES: &[u8] = include_bytes!("../fonts/CrimsonPro-Regular.ttf");

fn sans() -> Font {
    Font::with_name("DM Sans 9pt")
}

fn serif() -> Font {
    Font::with_name("Crimson Pro")
}

fn main() -> iced::Result {
    iced::application(App::boot, App::update, App::view)
        .title(App::title)
        .subscription(App::subscription)
        .window_size((1280.0, 820.0))
        .theme(theme)
        .default_font(sans())
        .font(SANS_BYTES)
        .font(SANS_BOLD_BYTES)
        .font(SANS_ITALIC_BYTES)
        .font(SERIF_BYTES)
        .run()
}

fn theme(app: &App) -> Theme {
    let ink = Ink::from_dark(app.is_dark());
    Theme::custom(
        if app.is_dark() {
            "parchment-dark"
        } else {
            "parchment-light"
        },
        iced::theme::Palette {
            background: ink.bg,
            text: ink.text,
            primary: ink.accent,
            success: Color::from_rgb8(46, 140, 96),
            warning: Color::from_rgb8(176, 122, 48),
            danger: Color::from_rgb8(184, 78, 62),
        },
    )
}

#[allow(clippy::struct_excessive_bools)]
struct App {
    dir: PathBuf,
    notebook: Option<Notebook>,
    /// Generation of `notebook`. An older snapshot is never installed over a newer one.
    book_gen: u64,
    search_hits: Option<Vec<NoteSummary>>,
    open_error: String,
    catalog: Vec<NoteSummary>,
    notes: Vec<NoteSummary>,
    tasks: Vec<TaskRow>,
    query: String,
    tag: Option<String>,
    selected: Option<String>,
    title_text: String,
    body: markdown::NoteBody,
    conflicts: Vec<(String, String, String)>,
    note_writer: String,
    message: String,
    screen: Screen,
    chats: Vec<ambient_cli::ConversationSummary>,
    chat_id: Option<String>,
    devices: Vec<ambient_cli::DeviceLine>,
    chat: Vec<ChatBubble>,
    draft_editor: text_editor::Content,
    my_card: String,
    their_card: String,
    relay_ready: bool,
    relay_ours: bool,
    sync_open: bool,
    sync_detail: String,
    pending: bool,
    net_mode: ambient_cli::NetMode,
    pin_draft: String,
    backup_draft: String,
    advanced: bool,
    net_up: bool,
    unsaved: Vec<(Option<String>, String, String)>,
    disk_title: String,
    disk_body: String,
    recovery_phrase: String,
    restore_phrase: String,
    contact_link: String,
    show_qr: bool,
    qr_bytes: Option<Vec<u8>>,
    unlock_input: String,
    sheet: Sheet,
    appearance: profile::Appearance,
    system_dark: bool,
    loaded: bool,
    saving: bool,
    save_again: bool,
    save_token: u64,
    your_name: String,
    device_draft: String,
    name_draft: String,
    call_draft: String,
    wipe_draft: String,
    aliases: BTreeMap<String, String>,
    /// Words this person added to the spellchecker. Kept on this computer, in `dictionary`.
    my_words: BTreeSet<String>,
    /// Where backups go, as typed. Empty until the person chooses, and then the default is used.
    backup_to: String,
    /// When the last backup finished, in unix milliseconds, or zero.
    backup_at: u64,
    /// A copy is being made. It holds both locks, so a second one waits for this one.
    backup_busy: bool,
    syncing: bool,
    edited_at: Option<Instant>,
    /// Bumps when the pause wait starts over, so a late wake cannot save another note.
    pause_token: u64,
    pause_armed: bool,
    /// Bumps when the search wait starts over. Only the latest query is run.
    search_token: u64,
    search_armed: bool,
    scroll_token: u64,
    scroll_armed: bool,
    writes: u64,
    size: Size,
    detail: bool,
    pending_files: Vec<PathBuf>,
    task_draft: String,
    confirm_delete: bool,
    send_open: bool,
    send_choice: Option<String>,
    boot_at: Instant,
    breeze_frame: u32,
    /// Byte where the current drag started. Empty when the mouse is up.
    drag_from: Option<usize>,
    /// Previous press: when, byte, and how many clicks landed on that line.
    last_press: Option<(Instant, usize, u8)>,
    /// Steps for the note that is open. Not written into the log.
    history: NoteHistory,
    /// Set after a new folder is created, until the welcome is finished.
    welcome: Option<WelcomeStep>,
    /// False only until the first welcome is finished. Older folders stay true.
    seen: bool,
    task_query: String,
    /// Note id and checkbox line being renamed in the task list.
    task_rename: Option<(String, usize)>,
    task_rename_text: String,
    /// Last line and clock label for each conversation. Display only.
    chat_last: BTreeMap<String, (String, String)>,
    /// How many messages each conversation had last time the list was read.
    chat_count: BTreeMap<String, usize>,
    /// How many this computer has already looked at. A local file, not the log.
    seen_count: BTreeMap<String, usize>,
    chat_query: String,
    hide_done: bool,
    confirm_clear: bool,
    /// The Tasks note is shown over the Tasks tab.
    tasks_note_open: bool,
    task_due_text: String,
    find_open: bool,
    find_query: String,
    /// Index of the current find hit. `usize::MAX` means the next jump starts at the first.
    find_at: usize,
    command_down: bool,
    shift_down: bool,
    /// True only while the button that started a selection is still down.
    pointer_down: bool,
    /// Where that press landed. A small move is still a click.
    press_x: f32,
    press_y: f32,
    /// The pointer moved far enough to be a drag, not a click.
    dragged: bool,
    /// This press already chose a word, a line, or a shift range.
    kept_click: bool,
    note_search: bool,
    task_search: bool,
    chat_search: bool,
    /// Writing with nothing else in view: the panels are away and the title is small.
    focus: bool,
    /// The right panel shows this note's settings, not its links.
    note_panel: bool,
    /// The note whose row the pointer is over, so its delete button shows.
    hover_row: Option<String>,
    /// The formatting bar, while writing in focus. Hidden until pulled in.
    slide_bar: Slide,
    /// The conversation whose name is being typed. Otherwise the name is plain text.
    alias_editing: Option<String>,
    invite_pick: Option<InvitePick>,
    /// Undo for notes that are not on screen. Still memory, still not the log.
    histories: BTreeMap<String, NoteHistory>,
    history_order: Vec<String>,
    history_key: String,
    /// Contents note for the chapter rail. An empty string is the unsaved draft.
    book_id: Option<String>,
    /// Linked chapters drawn as one reading, not the editor.
    reading: bool,
    /// A missing chapter to create once the open note finishes saving.
    pending_chapter: Option<String>,
    /// The note list on the left. Memory only.
    show_list: bool,
    /// The chapter panel on the right. Memory only.
    show_context: bool,
    /// The chapter panel was closed by hand, so a group does not open it again.
    context_shut: bool,
    /// Title last checked for a group, and that group's id when one names it.
    group_cache: Option<(String, Option<String>)>,
    /// Tags on the open note. Not words in the writing.
    tags: Vec<String>,
    tag_draft: String,
    /// Group note id and the link targets under it.
    groups: Vec<(String, Vec<String>)>,
    /// Notes that sit under a group, so the list does not repeat them.
    pieces: BTreeSet<String>,
    cached_chapters: Vec<Chapter>,
    cached_backlinks: Vec<(String, String)>,
    /// Ids and lowercase titles a note link can open. Empty until the list is read.
    known_links: Option<Arc<BTreeSet<String>>>,
    /// Body last sent to the proof job. Unchanged text is not sent again.
    proof_body: String,
    /// One quiet line. Empty when the marks are closed.
    proof_warning: String,
    /// Misspelled words from the last proof. The caret thread only reads this.
    misspellings: Arc<BTreeSet<String>>,
    /// Matching line for each note id from the last search.
    search_lines: BTreeMap<String, String>,
    /// Conversations whose lines matched the note search.
    search_chats: Vec<lane::ChatHit>,
    /// An older snapshot on screen. It is not the editor and it is not saved.
    older: Option<OlderNote>,
    /// Note id of a version open that has not arrived yet.
    version_wait: Option<String>,
    /// Note cited by the next chat message.
    chat_cite: Option<(String, String)>,
    cite_open: bool,
    /// Note cited by the next task.
    task_cite: Option<(String, String)>,
    task_cite_open: bool,
    /// Background folder work. The caret's thread only posts a job.
    lane: std::sync::mpsc::Sender<lane::Job>,
    /// Every group's pieces, filled off the caret's thread.
    group_views: Vec<lane::GroupView>,
}

/// Which side of an invite this computer is on. One path, not both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InvitePick {
    Theirs,
    Mine,
}

/// One wait. A new token drops the previous wait.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Wake {
    Pause(u64),
    Search(u64),
    /// The last scroll was a moment ago, so the scrollbar can go away.
    Scroll(u64),
}

#[derive(Clone, Debug)]
enum Message {
    QueryChanged(String),
    PickTag(Option<String>),
    Select(String),
    NewNote,
    TitleChanged(String),
    BodyEdited(text_editor::Action),
    /// Press on a letter. A click puts the caret there. A drag starts from there.
    SelectFrom {
        byte: usize,
        x: f32,
        y: f32,
    },
    /// Drag across letters, once the pointer has actually moved.
    ExtendTo {
        byte: usize,
        x: f32,
        y: f32,
    },
    /// The mouse button lifted.
    SelectStop,
    SetOpenTask {
        line: usize,
        done: bool,
    },
    CopyNote,
    CutNote,
    Save,
    ShowNotes,
    ShowTasks,
    ShowChat,
    ToggleList,
    ToggleContext,
    TagDraft(String),
    AddTag,
    RemoveTag(String),
    CompleteLink(String),
    CompleteVersion {
        id: String,
        version: String,
    },
    CloseVersion,
    OpenCited(String),
    ToggleChatCite,
    PickChatCite(String),
    ClearChatCite,
    ToggleTaskCite,
    PickTaskCite(String),
    ClearTaskCite,
    Lane(lane::Done),
    AutoSync,
    ToggleSyncPop,
    ToggleRelay,
    ToggleAdvanced,
    PinDraft(String),
    BackupDraft(String),
    UseThisComputer,
    SavePinned,
    OpenPaired,
    OpenPerson,
    #[allow(dead_code)]
    CopyCard,
    #[allow(dead_code)]
    MyCardChanged(String),
    TheirCardChanged(String),
    StartConversation,
    DraftEdited(text_editor::Action),
    SendChat,
    ShowPhrase,
    CopyPhrase,
    RestorePhraseChanged(String),
    StartRestore,
    CopyLink,
    ToggleQr,
    SendNoteToChat,
    UnlockChanged(String),
    SubmitUnlock,
    ShowInvite,
    ShowSettings,
    CloseSheet,
    SelectChat(String),
    SetTask {
        id: String,
        line: usize,
        done: bool,
    },
    Revoke(String),
    ToggleTheme,
    Resized(Size),
    FileDropped(PathBuf),
    PickFile,
    FilePicked(Result<PathBuf, String>),
    DropPending(usize),
    OpenPath(PathBuf),
    Format(FormatKind),
    /// Tab or Shift+Tab in the writing: nest the list items under the caret, or bring them out.
    ShiftItems {
        back: bool,
    },
    PasteNote,
    Pasted(Option<String>),
    ClearSelect,
    OpenChapter(String),
    ToggleRead,
    ChapterMade(Result<MadeChapter, String>),
    TaskDraft(String),
    AddTask,
    AskDelete,
    CancelDelete,
    DeleteNote,
    Deleted(Result<Vec<NoteSummary>, String>),
    ChooseSend(String),
    ConfirmSend,
    CancelSend,
    Breeze,
    Woke(Wake),
    /// The first wheel tick of a burst. Starts the wake that hides the scrollbar again.
    Scrolled,
    Opened(Result<(), String>),
    Saved(
        u64,
        Result<(String, Vec<NoteSummary>), String>,
        String,
        String,
    ),
    Applied(Result<Vec<NoteSummary>, String>),
    DeviceDraft(String),
    NameDraft(String),
    FinishOnboarding,
    PickAppearance(profile::Appearance),
    CallDraft(String),
    AliasDraft(String, String),
    /// The pencil beside a conversation's name: start typing it, or stop.
    EditAlias,
    FinishAlias,
    /// The icon at the top right of a note: write with nothing else in view, or come back.
    ToggleFocus,
    /// Enter in the title: start writing.
    TitleSubmit,
    /// Ctrl or Cmd + Shift + D: tell the spellchecker the word at the caret is fine.
    AddWord,
    /// Enter in a field that has nothing to submit. It exists so the key is taken by the field and
    /// does not fall through to the page.
    FieldDone,
    /// A key that nothing handled, while the writing page is up. See `ignored_key`.
    TypedIgnored(Typed),
    /// The text in the box that says where a backup goes.
    BackupTo(String),
    /// Copy this notebook's folder to another place.
    BackUp,
    /// A copy finished, or failed.
    BackedUp(Result<profile::BackupReport, String>),
    /// Show or close the Tasks note over the Tasks tab.
    ToggleTasksNote,
    /// The icon at the top of the right panel: this note's settings, or its links.
    ToggleNoteSettings,
    /// The pointer went onto (true) or off (false) a note's row.
    RowHover(String, bool),
    /// The delete button on a note's row.
    AskDeleteNote(String),
    /// The button beside the small title in focus: pull the formatting bar in, or send it away.
    FocusBar,
    /// A frame of a slide. It changes nothing. Handling it redraws.
    SlideTick,
    WipeDraft(String),
    ExportNotes,
    ConfirmWipe,
    Undo,
    Redo,
    TaskQuery(String),
    BeginTaskEdit {
        id: String,
        line: usize,
    },
    TaskLabel(String),
    CommitTaskRename,
    CancelTaskEdit,
    RemoveTask {
        id: String,
        line: usize,
    },
    CopyChat(String),
    WelcomeStored,
    WelcomeDone,
    ToggleDone,
    AskClearDone,
    CancelClearDone,
    ClearDone,
    MoveTask {
        id: String,
        line: usize,
        up: bool,
    },
    DueDraft(String),
    ChatQuery(String),
    FindOpen,
    FindQuery(String),
    FindNext,
    FindPrev,
    Mods {
        command: bool,
        shift: bool,
    },
    PickInvite(InvitePick),
    ToggleNoteSearch,
    ToggleTaskSearch,
    ToggleChatSearch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sheet {
    None,
    Invite,
    Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Screen {
    Notes,
    Tasks,
    Chat,
}

struct TaskRow {
    note_id: String,
    note_title: String,
    line: usize,
    done: bool,
    text: String,
}

struct OlderNote {
    id: String,
    title: String,
    body: String,
}

struct Chapter {
    name: String,
    /// The link as written: a note id, or a title on an older link.
    target: String,
    id: Option<String>,
    words: usize,
    body: String,
}

#[derive(Clone, Debug)]
struct SavedDraft {
    id: String,
    title: String,
    body: String,
}

#[derive(Clone, Debug)]
struct MadeChapter {
    notes: Vec<NoteSummary>,
    id: String,
    saved: Option<SavedDraft>,
}

/// One step back. The next save writes whatever the page shows. It does not rewrite the log.
#[derive(Clone)]
struct EditSnap {
    title: String,
    body: String,
    cursor: text_editor::Cursor,
}

impl EditSnap {
    fn same_text(&self, title: &str, body: &str) -> bool {
        self.title == title && self.body == body
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Burst {
    Type,
    Erase,
    Break,
}

enum HistGate {
    Coalesce {
        burst: Option<Burst>,
        at: Option<Instant>,
    },
    Step {
        pushed: bool,
        dropped: Option<EditSnap>,
        redo: Vec<EditSnap>,
        burst: Option<Burst>,
        at: Option<Instant>,
    },
}

struct NoteHistory {
    undo: Vec<EditSnap>,
    redo: Vec<EditSnap>,
    burst: Option<Burst>,
    at: Option<Instant>,
}

impl Default for NoteHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl NoteHistory {
    const CAP: usize = 80;

    fn new() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            burst: None,
            at: None,
        }
    }

    fn clear(&mut self) {
        *self = Self::new();
    }

    fn coalesced(&self, burst: Burst, now: Instant) -> bool {
        matches!(burst, Burst::Type | Burst::Erase)
            && self.burst == Some(burst)
            && self
                .at
                .is_some_and(|at| now.saturating_duration_since(at) < Duration::from_millis(800))
            && !self.undo.is_empty()
    }

    fn begin(
        &mut self,
        title: &str,
        body: &str,
        cursor: text_editor::Cursor,
        burst: Burst,
        now: Instant,
    ) -> HistGate {
        if self.coalesced(burst, now) {
            let gate = HistGate::Coalesce {
                burst: self.burst,
                at: self.at,
            };
            self.burst = Some(burst);
            self.at = Some(now);
            return gate;
        }
        let redo = std::mem::take(&mut self.redo);
        let previous_burst = self.burst;
        let previous_at = self.at;
        let snap = EditSnap {
            title: title.to_owned(),
            body: body.to_owned(),
            cursor,
        };
        let mut pushed = false;
        let mut dropped = None;
        if self
            .undo
            .last()
            .is_none_or(|top| !top.same_text(&snap.title, &snap.body))
        {
            self.undo.push(snap);
            pushed = true;
            if self.undo.len() > Self::CAP {
                dropped = Some(self.undo.remove(0));
            }
        }
        self.burst = if burst == Burst::Break {
            None
        } else {
            Some(burst)
        };
        self.at = Some(now);
        HistGate::Step {
            pushed,
            dropped,
            redo,
            burst: previous_burst,
            at: previous_at,
        }
    }

    fn abort(&mut self, gate: HistGate) {
        match gate {
            HistGate::Coalesce { burst, at } => {
                self.burst = burst;
                self.at = at;
            }
            HistGate::Step {
                pushed,
                dropped,
                redo,
                burst,
                at,
            } => {
                if pushed {
                    self.undo.pop();
                }
                if let Some(dropped) = dropped {
                    self.undo.insert(0, dropped);
                }
                self.redo = redo;
                self.burst = burst;
                self.at = at;
            }
        }
    }

    fn undo(&mut self, current: EditSnap) -> Option<EditSnap> {
        while self
            .undo
            .last()
            .is_some_and(|top| top.same_text(&current.title, &current.body))
        {
            self.undo.pop();
        }
        let previous = self.undo.pop()?;
        self.redo.push(current);
        self.burst = None;
        self.at = None;
        Some(previous)
    }

    fn redo(&mut self, current: EditSnap) -> Option<EditSnap> {
        let next = self.redo.pop()?;
        if self
            .undo
            .last()
            .is_none_or(|top| !top.same_text(&current.title, &current.body))
        {
            self.undo.push(current);
            if self.undo.len() > Self::CAP {
                self.undo.remove(0);
            }
        }
        self.burst = None;
        self.at = None;
        Some(next)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WelcomeStep {
    Phrase,
    Guide,
}

impl App {
    fn boot() -> (Self, Task<Message>) {
        let dir = std::env::args()
            .nth(1)
            .map_or_else(|| PathBuf::from("notebook"), PathBuf::from);
        Self::at(dir)
    }

    /// The app for one folder. A test uses this with a folder of its own.
    #[allow(clippy::too_many_lines)]
    fn at(dir: PathBuf) -> (Self, Task<Message>) {
        let mut app = Self {
            dir,
            notebook: None,
            book_gen: 0,
            search_hits: None,
            open_error: String::new(),
            catalog: Vec::new(),
            notes: Vec::new(),
            tasks: Vec::new(),
            query: String::new(),
            tag: None,
            selected: None,
            title_text: String::new(),
            body: markdown::NoteBody::new(),
            conflicts: Vec::new(),
            note_writer: String::new(),
            message: String::new(),
            screen: Screen::Notes,
            chats: Vec::new(),
            chat_id: None,
            devices: Vec::new(),
            chat: Vec::new(),
            draft_editor: text_editor::Content::new(),
            my_card: String::new(),
            their_card: String::new(),
            relay_ready: false,
            relay_ours: false,
            sync_open: false,
            sync_detail: "Relay is starting.".to_owned(),
            pending: false,
            net_mode: ambient_cli::NetMode::ThisComputer,
            pin_draft: String::new(),
            backup_draft: String::new(),
            advanced: false,
            net_up: false,
            unsaved: Vec::new(),
            disk_title: String::new(),
            disk_body: String::new(),
            recovery_phrase: String::new(),
            restore_phrase: String::new(),
            contact_link: String::new(),
            show_qr: false,
            qr_bytes: None,
            unlock_input: String::new(),
            sheet: Sheet::None,
            appearance: profile::Appearance::Dark,
            system_dark: true,
            loaded: false,
            saving: false,
            save_again: false,
            save_token: 0,
            your_name: String::new(),
            device_draft: String::new(),
            name_draft: String::new(),
            call_draft: String::new(),
            wipe_draft: String::new(),
            aliases: BTreeMap::new(),
            my_words: BTreeSet::new(),
            backup_to: String::new(),
            backup_at: 0,
            backup_busy: false,
            syncing: false,
            edited_at: None,
            pause_token: 0,
            pause_armed: false,
            search_token: 0,
            search_armed: false,
            scroll_token: 0,
            scroll_armed: false,
            writes: 0,
            size: Size::new(1280.0, 820.0),
            detail: false,
            pending_files: Vec::new(),
            task_draft: String::new(),
            confirm_delete: false,
            send_open: false,
            send_choice: None,
            boot_at: Instant::now(),
            breeze_frame: 0,
            drag_from: None,
            last_press: None,
            history: NoteHistory::new(),
            welcome: None,
            seen: true,
            task_query: String::new(),
            task_rename: None,
            task_rename_text: String::new(),
            chat_last: BTreeMap::new(),
            chat_count: BTreeMap::new(),
            seen_count: BTreeMap::new(),
            chat_query: String::new(),
            hide_done: false,
            confirm_clear: false,
            tasks_note_open: false,
            task_due_text: String::new(),
            find_open: false,
            find_query: String::new(),
            find_at: usize::MAX,
            command_down: false,
            shift_down: false,
            pointer_down: false,
            press_x: 0.0,
            press_y: 0.0,
            dragged: false,
            kept_click: false,
            note_search: false,
            task_search: false,
            chat_search: false,
            focus: false,
            note_panel: false,
            hover_row: None,
            slide_bar: Slide::settled(true),
            alias_editing: None,
            invite_pick: None,
            histories: BTreeMap::new(),
            history_order: Vec::new(),
            history_key: String::new(),
            book_id: None,
            reading: false,
            pending_chapter: None,
            show_list: true,
            show_context: false,
            context_shut: false,
            group_cache: None,
            tags: Vec::new(),
            tag_draft: String::new(),
            groups: Vec::new(),
            pieces: BTreeSet::new(),
            cached_chapters: Vec::new(),
            cached_backlinks: Vec::new(),
            known_links: None,
            lane: lane::start(),
            group_views: Vec::new(),
            proof_body: String::new(),
            proof_warning: String::new(),
            misspellings: Arc::new(BTreeSet::new()),
            search_lines: BTreeMap::new(),
            search_chats: Vec::new(),
            older: None,
            version_wait: None,
            chat_cite: None,
            cite_open: false,
            task_cite: None,
            task_cite_open: false,
        };
        let saved = profile::load_profile(&app.dir);
        app.appearance = saved.appearance;
        app.seen = saved.seen;
        app.your_name.clone_from(&saved.name);
        app.name_draft.clone_from(&saved.name);
        app.device_draft.clone_from(&saved.device);
        app.backup_to.clone_from(&saved.backup_to);
        app.backup_at = saved.backup_at;
        app.system_dark = profile::system_is_dark();
        for (id, name) in profile::load_names(&app.dir) {
            app.aliases.insert(id, name);
        }
        app.my_words = profile::load_words(&app.dir).into_iter().collect();
        if !app.my_words.is_empty() {
            app.share_words();
        }
        for (id, count) in profile::load_seen(&app.dir) {
            app.seen_count.insert(id, count);
        }
        let task = if app.dir.join("key").exists() {
            open_task(app.dir.clone())
        } else {
            app.loaded = true;
            Task::none()
        };
        app.boot_at = Instant::now();
        (app, task)
    }

    fn subscription(app: &App) -> iced::Subscription<Message> {
        let window = iced::event::listen_with(|event, status, _id| match event {
            iced::Event::Window(
                iced::window::Event::Resized(size) | iced::window::Event::Opened { size, .. },
            ) => Some(Message::Resized(size)),
            iced::Event::Window(iced::window::Event::FileDropped(path)) => {
                Some(Message::FileDropped(path))
            }
            iced::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(mods)) => {
                Some(Message::Mods {
                    command: mods.control() || mods.command(),
                    shift: mods.shift(),
                })
            }
            iced::Event::Mouse(iced::mouse::Event::WheelScrolled { .. }) => scroll_seen(),
            iced::Event::Mouse(
                iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)
                | iced::mouse::Event::CursorLeft,
            )
            | iced::Event::Window(iced::window::Event::Unfocused)
            | iced::Event::Touch(
                iced::touch::Event::FingerLifted { .. } | iced::touch::Event::FingerLost { .. },
            ) => Some(Message::SelectStop),
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. })
                if status == iced::event::Status::Ignored
                    && (modifiers.control() || modifiers.command())
                    && matches!(key.as_ref(), Key::Character(text) if text.eq_ignore_ascii_case("f")) =>
            {
                Some(Message::FindOpen)
            }
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. })
                if status == iced::event::Status::Ignored
                    && (modifiers.control() || modifiers.command())
                    && matches!(key.as_ref(), Key::Character(".")) =>
            {
                Some(Message::ToggleFocus)
            }
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key,
                modifiers,
                text,
                ..
            }) if status == iced::event::Status::Ignored => {
                ignored_key(&key, text.as_deref(), modifiers).map(Message::TypedIgnored)
            }
            _ => None,
        });
        let mut parts = vec![window];
        if app.editing_note() && !app.pointer_down && app.body.selection_bounds().is_some() {
            parts.push(iced::event::listen_with(
                |event, _status, _id| match event {
                    iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. })
                        if matches!(
                            key.as_ref(),
                            Key::Named(iced::keyboard::key::Named::Escape)
                        ) =>
                    {
                        Some(Message::ClearSelect)
                    }
                    _ => None,
                },
            ));
        }
        if app.breeze_on() {
            parts.push(
                iced::time::every(std::time::Duration::from_millis(32)).map(|_| Message::Breeze),
            );
        }
        if app.pause_armed {
            parts.push(iced::Subscription::run_with(
                Wake::Pause(app.pause_token),
                wake_stream,
            ));
        }
        if app.search_armed {
            parts.push(iced::Subscription::run_with(
                Wake::Search(app.search_token),
                wake_stream,
            ));
        }
        if app.slide_bar.moving() {
            parts.push(
                iced::time::every(std::time::Duration::from_millis(16)).map(|_| Message::SlideTick),
            );
        }
        if app.scroll_armed {
            parts.push(iced::Subscription::run_with(
                Wake::Scroll(app.scroll_token),
                wake_stream,
            ));
        }
        let remote = app.net_mode != ambient_cli::NetMode::ThisComputer;
        if app.notebook.is_some() && (app.relay_ready || remote) {
            parts.push(
                iced::time::every(std::time::Duration::from_secs(3)).map(|_| Message::AutoSync),
            );
        }
        parts.push(iced::Subscription::run(lane_messages));
        iced::Subscription::batch(parts)
    }

    #[allow(clippy::unused_self)]
    fn title(&self) -> String {
        "Aerodash".to_owned()
    }

    #[allow(clippy::too_many_lines)]
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::QueryChanged(query) => {
                self.query = query;
                self.search_hits = None;
                self.search_lines.clear();
                self.search_chats.clear();
                if self.query.trim().is_empty() {
                    self.disarm_search();
                } else {
                    self.arm_search();
                }
                self.refresh_shown();
                Task::none()
            }
            Message::PickTag(tag) => {
                self.tag = if self.tag == tag { None } else { tag };
                self.refresh_shown();
                Task::none()
            }
            Message::Select(id) => {
                self.show_note(&id);
                Task::none()
            }
            Message::NewNote => {
                self.clear_editor();
                iced::widget::operation::focus(iced::widget::Id::new("note-title"))
            }
            Message::TitleChanged(title) => {
                if title != self.title_text {
                    self.revise(Burst::Type, |app| {
                        app.title_text = title;
                        true
                    });
                }
                Task::none()
            }
            Message::ShiftItems { back } => {
                match markdown::shift_items(&self.body.text(), &self.body.cursor(), back) {
                    Some(change) => {
                        self.drag_from = None;
                        self.apply_change(change);
                        self.note_follow(true)
                    }
                    None => Task::none(),
                }
            }
            Message::BodyEdited(action) => {
                if matches!(
                    action,
                    text_editor::Action::Click(_) | text_editor::Action::Drag(_)
                ) {
                    return Task::none();
                }
                let structured = match &action {
                    text_editor::Action::Edit(text_editor::Edit::Enter) => {
                        markdown::continue_enter(&self.body.text(), self.body.cursor())
                    }
                    text_editor::Action::Edit(text_editor::Edit::Backspace) => {
                        markdown::exit_empty_item(&self.body.text(), self.body.cursor())
                    }
                    _ => None,
                };
                if let Some(change) = structured {
                    self.drag_from = None;
                    self.apply_change(change);
                    return self.note_follow(true);
                }
                if let Some(burst) = edit_burst(&action) {
                    self.revise(burst, |app| app.body.edit(&action));
                    return self.note_follow(true);
                }
                if self.body.edit(&action) {
                    self.arm_pause();
                }
                self.note_follow(true)
            }
            Message::SelectFrom { byte, x, y } => {
                if self.command_down && self.try_open_at(byte) {
                    self.drag_from = None;
                    self.dragged = false;
                    return Task::none();
                }
                self.press_x = x;
                self.press_y = y;
                self.dragged = false;
                self.arm_press(byte);
                self.note_follow(false)
            }
            Message::ExtendTo { byte, x, y } => {
                if !self.pointer_down {
                    self.drag_from = None;
                    return Task::none();
                }
                let Some(anchor) = self.drag_from else {
                    return Task::none();
                };
                let dx = x - self.press_x;
                let dy = y - self.press_y;
                // A click moves a little. A drag has to leave the letter.
                if dx * dx + dy * dy < 1_024.0 {
                    return Task::none();
                }
                self.dragged = true;
                let byte = markdown::snap_caret(&self.body.text(), byte);
                self.body.select_between(anchor, byte);
                Task::none()
            }
            Message::SelectStop => {
                let was_down = self.pointer_down;
                let dragged = self.dragged;
                let kept = self.kept_click;
                self.pointer_down = false;
                self.dragged = false;
                self.kept_click = false;
                if self.drag_from.take().is_some() {
                    if dragged || kept {
                        self.body.end_select();
                        if dragged {
                            // The next click places the caret. It is not a second click.
                            self.last_press = None;
                        }
                    } else {
                        let byte = markdown::snap_caret(&self.body.text(), self.body.caret_byte());
                        self.body.place_byte(byte);
                    }
                }
                if was_down {
                    self.note_follow(false)
                } else {
                    Task::none()
                }
            }
            Message::SetOpenTask { line, done } => {
                if let Some(id) = self.selected.clone() {
                    let follow = self.set_task(&id, line, done);
                    let back = self.note_follow(false);
                    if self.editor_dirty() {
                        return Task::batch([follow, back, self.start_save(true)]);
                    }
                    return Task::batch([follow, back]);
                }
                let text = self.body.text();
                if let Some(next) = markdown::set_task_line(&text, line, done) {
                    self.revise(Burst::Break, |app| {
                        let cursor = app.body.cursor();
                        app.body.load(&next);
                        app.body.place(cursor);
                        true
                    });
                }
                self.note_follow(false)
            }
            Message::CopyNote => {
                if let Some(text) = self.body.selection() {
                    clipboard::write(text)
                } else {
                    Task::none()
                }
            }
            Message::CutNote => {
                if let Some(text) = self.body.selection() {
                    self.revise(Burst::Break, |app| {
                        app.body
                            .edit(&text_editor::Action::Edit(text_editor::Edit::Delete))
                    });
                    Task::batch([clipboard::write(text), self.note_follow(true)])
                } else {
                    Task::none()
                }
            }
            Message::Save => self.start_save(true),
            Message::ShowNotes => {
                if self.compact()
                    && self.screen == Screen::Notes
                    && self.detail
                    && self.sheet == Sheet::None
                {
                    self.detail = false;
                } else {
                    self.screen = Screen::Notes;
                    self.sheet = Sheet::None;
                    if self.compact() {
                        self.detail = false;
                    }
                }
                self.sync_watch();
                Task::none()
            }
            Message::ShowTasks => {
                self.tasks_note_open = false;
                self.rebuild_tasks();
                self.screen = Screen::Tasks;
                self.sheet = Sheet::None;
                self.detail = false;
                self.sync_watch();
                focus_named("task-input")
            }
            Message::ShowChat => {
                if self.compact()
                    && self.screen == Screen::Chat
                    && self.detail
                    && self.sheet == Sheet::None
                {
                    self.detail = false;
                } else {
                    self.remember_unsaved();
                    self.screen = Screen::Chat;
                    self.alias_editing = None;
                    self.sheet = Sheet::None;
                    if self.compact() {
                        self.detail = false;
                    }
                    self.sync_watch();
                    self.post(lane::Job::Pull);
                }
                Task::none()
            }
            Message::ToggleList => {
                self.show_list = !self.show_list;
                Task::none()
            }
            Message::ToggleContext => {
                self.show_context = !self.show_context;
                self.context_shut = !self.show_context;
                Task::none()
            }
            Message::TagDraft(text) => {
                self.tag_draft = text;
                Task::none()
            }
            Message::AddTag => {
                self.add_tag();
                Task::none()
            }
            Message::RemoveTag(tag) => {
                self.remove_tag(&tag);
                Task::none()
            }
            Message::CompleteLink(id) => self.complete_link(&id, None),
            Message::CompleteVersion { id, version } => {
                self.complete_link(&id, Some(version.as_str()))
            }
            Message::CloseVersion => {
                self.close_version();
                Task::none()
            }
            Message::OpenCited(target) => {
                let (name, version) = markdown::split_version(&target);
                self.open_note_link(&name, version.as_deref());
                Task::none()
            }
            Message::ToggleChatCite => {
                self.cite_open = !self.cite_open;
                Task::none()
            }
            Message::PickChatCite(id) => {
                self.pick_cite(&id, false);
                Task::none()
            }
            Message::ClearChatCite => {
                self.chat_cite = None;
                Task::none()
            }
            Message::ToggleTaskCite => {
                self.task_cite_open = !self.task_cite_open;
                Task::none()
            }
            Message::PickTaskCite(id) => {
                self.pick_cite(&id, true);
                Task::none()
            }
            Message::ClearTaskCite => {
                self.task_cite = None;
                Task::none()
            }
            Message::Lane(done) => {
                self.apply_lane(done);
                Task::none()
            }
            Message::AutoSync => self.begin_sync(),
            Message::ToggleSyncPop => {
                self.sync_open = !self.sync_open;
                Task::none()
            }
            Message::ToggleRelay => {
                if self.relay_ready && !self.relay_ours {
                    "This window did not start the relay.".clone_into(&mut self.sync_detail);
                    Task::none()
                } else if self.relay_ready {
                    self.stop_relay();
                    Task::none()
                } else {
                    self.start_relay();
                    self.begin_sync()
                }
            }
            Message::ToggleAdvanced => {
                self.advanced = !self.advanced;
                Task::none()
            }
            Message::PinDraft(text) => {
                self.pin_draft = text;
                Task::none()
            }
            Message::BackupDraft(text) => {
                self.backup_draft = text;
                Task::none()
            }
            Message::UseThisComputer => {
                self.use_this_computer();
                self.begin_sync()
            }
            Message::SavePinned => {
                self.save_pinned();
                self.begin_sync()
            }
            Message::OpenPaired => {
                self.open_paired();
                Task::none()
            }
            Message::OpenPerson => {
                self.open_person();
                Task::none()
            }
            Message::CopyCard => self.copy_card(),
            Message::MyCardChanged(card) => {
                self.my_card = card;
                Task::none()
            }
            Message::TheirCardChanged(card) => {
                self.their_card = card;
                Task::none()
            }
            Message::StartConversation => {
                self.start_chat();
                Task::none()
            }
            Message::DraftEdited(action) => {
                self.draft_editor.perform(action);
                Task::none()
            }
            Message::SendChat => {
                self.send_chat();
                Task::none()
            }
            Message::ShowPhrase => {
                self.show_phrase();
                Task::none()
            }
            Message::CopyPhrase => self.copy_phrase(),
            Message::RestorePhraseChanged(phrase) => {
                self.restore_phrase = phrase;
                Task::none()
            }
            Message::StartRestore => {
                self.start_restore();
                Task::none()
            }
            Message::CopyLink => self.copy_link(),
            Message::ToggleQr => {
                self.toggle_qr();
                Task::none()
            }
            Message::SendNoteToChat => {
                self.send_note_to_chat();
                Task::none()
            }
            Message::SelectChat(id) => {
                if self.chat_id.as_deref() != Some(id.as_str()) {
                    self.chat.clear();
                }
                self.chat_id = Some(id);
                self.screen = Screen::Chat;
                self.sheet = Sheet::None;
                self.detail = true;
                self.sync_watch();
                self.post(lane::Job::Pull);
                Task::none()
            }
            Message::SetTask { id, line, done } => {
                let follow = self.set_task(&id, line, done);
                if self.editor_dirty() && self.selected.as_deref() == Some(id.as_str()) {
                    Task::batch([follow, self.start_save(true)])
                } else {
                    follow
                }
            }
            Message::Revoke(id) => {
                self.revoke(&id);
                Task::none()
            }
            Message::UnlockChanged(text) => {
                self.unlock_input = text;
                Task::none()
            }
            Message::SubmitUnlock => self.submit_unlock(),
            Message::ShowInvite => {
                if let Ok(link) = ambient_cli::contact_link(&self.dir) {
                    if link != self.contact_link {
                        self.show_qr = false;
                        self.qr_bytes = None;
                    }
                    self.contact_link = link;
                }
                self.invite_pick = None;
                self.sheet = Sheet::Invite;
                Task::none()
            }
            Message::ShowSettings => {
                self.devices = self
                    .notebook
                    .as_ref()
                    .and_then(|book| book.devices_held().ok())
                    .unwrap_or_default();
                self.sheet = Sheet::Settings;
                Task::none()
            }
            Message::CloseSheet => {
                self.sheet = Sheet::None;
                Task::none()
            }
            Message::ToggleTheme => {
                self.appearance = if self.is_dark() {
                    profile::Appearance::Light
                } else {
                    profile::Appearance::Dark
                };
                self.persist_profile();
                Task::none()
            }
            Message::Resized(size) => {
                let was_compact = self.compact();
                self.size = size;
                if !was_compact && self.compact() {
                    self.detail = match self.screen {
                        Screen::Notes => self.selected.is_some() || self.editor_dirty(),
                        Screen::Chat => self.chat_id.is_some(),
                        Screen::Tasks => false,
                    };
                }
                Task::none()
            }
            Message::FileDropped(path) => {
                self.stage_file(path);
                Task::none()
            }
            Message::PickFile => self.pick_file(),
            Message::FilePicked(result) => {
                match result {
                    Ok(path) => self.stage_file(path),
                    Err(err) => {
                        if err != "No file chosen." {
                            self.message = err;
                        }
                    }
                }
                Task::none()
            }
            Message::DropPending(index) => {
                if index < self.pending_files.len() {
                    self.pending_files.remove(index);
                }
                Task::none()
            }
            Message::OpenPath(path) => {
                if let Err(err) = files::open_with_system(&path) {
                    self.message = err;
                }
                Task::none()
            }
            Message::Format(kind) => {
                self.insert_mark(kind);
                self.note_follow(true)
            }
            Message::PasteNote => clipboard::read().map(Message::Pasted),
            Message::Pasted(Some(text)) if !text.is_empty() => {
                self.revise(Burst::Break, |app| {
                    let before = app.body.text();
                    app.body
                        .perform(&text_editor::Action::Edit(text_editor::Edit::Paste(
                            Arc::new(text),
                        )));
                    app.body.text() != before
                });
                self.note_follow(true)
            }
            // A paste that was handled elsewhere, and a frame of a slide: handling them redraws.
            Message::Pasted(_) | Message::SlideTick | Message::FieldDone => Task::none(),
            Message::ClearSelect => {
                let cursor = self.body.cursor();
                self.body.place(text_editor::Cursor {
                    position: cursor.position,
                    selection: None,
                });
                Task::none()
            }
            Message::OpenChapter(name) => self.open_chapter(&name),
            Message::ToggleRead => {
                if self.cached_chapters.len() >= 2 {
                    self.reading = !self.reading;
                }
                Task::none()
            }
            Message::ChapterMade(result) => self.finish_chapter(result),
            Message::TaskDraft(text) => {
                self.task_draft = text;
                Task::none()
            }
            Message::AddTask => {
                let follow = self.add_loose_task();
                // The cursor stays in the field, so the next task can be typed at once.
                let back = focus_named("task-input");
                if self.editor_dirty() {
                    Task::batch([follow, self.start_save(true), back])
                } else {
                    Task::batch([follow, back])
                }
            }
            Message::AskDelete => {
                if self.selected.is_some() || self.editor_dirty() {
                    self.confirm_delete = true;
                }
                Task::none()
            }
            Message::CancelDelete => {
                self.confirm_delete = false;
                Task::none()
            }
            Message::DeleteNote => self.delete_open_note(),
            Message::Deleted(result) => self.finish_delete(result),
            Message::ChooseSend(id) => {
                self.send_choice = Some(id);
                Task::none()
            }
            Message::ConfirmSend => {
                self.confirm_send();
                Task::none()
            }
            Message::CancelSend => {
                self.send_open = false;
                Task::none()
            }
            Message::Breeze => {
                self.breeze_frame = self.breeze_frame.wrapping_add(1);
                Task::none()
            }
            Message::Woke(wake) => match wake {
                Wake::Pause(token) if self.pause_armed && token == self.pause_token => {
                    self.pause_armed = false;
                    self.maybe_proof();
                    self.start_save(true)
                }
                Wake::Search(token) if self.search_armed && token == self.search_token => {
                    self.search_armed = false;
                    if !self.query.trim().is_empty() {
                        self.post(lane::Job::Search(self.query.clone()));
                    }
                    Task::none()
                }
                Wake::Scroll(token) if self.scroll_armed && token == self.scroll_token => {
                    // Handling the wake redraws, and the style sees whether the last scroll is
                    // old, so the thumb goes. If the pane is still moving, look again later.
                    if scrolled_recently() {
                        self.scroll_token = self.scroll_token.wrapping_add(1);
                    } else {
                        self.scroll_armed = false;
                        SCROLL_WAKE.store(false, std::sync::atomic::Ordering::Relaxed);
                    }
                    Task::none()
                }
                Wake::Pause(_) | Wake::Search(_) | Wake::Scroll(_) => Task::none(),
            },
            Message::Scrolled => {
                self.scroll_token = self.scroll_token.wrapping_add(1);
                self.scroll_armed = true;
                Task::none()
            }
            Message::Opened(result) => self.finish_open(result),
            Message::Saved(token, result, title, body) => {
                self.finish_save(token, result, title, body)
            }
            Message::Applied(result) => self.finish_applied(result),
            Message::DeviceDraft(text) => {
                self.device_draft = text;
                Task::none()
            }
            Message::NameDraft(text) => {
                self.name_draft = text;
                self.your_name.clone_from(&self.name_draft);
                self.persist_profile();
                Task::none()
            }
            Message::FinishOnboarding => self.finish_onboarding(),
            Message::PickAppearance(appearance) => {
                self.appearance = appearance;
                self.persist_profile();
                Task::none()
            }
            Message::CallDraft(text) => {
                self.call_draft = text;
                Task::none()
            }
            Message::AliasDraft(id, text) => {
                if text.trim().is_empty() {
                    self.aliases.remove(&id);
                } else {
                    self.aliases.insert(id, text);
                }
                self.persist_names();
                Task::none()
            }
            Message::EditAlias => match self.chat_id.clone() {
                Some(id) if self.alias_editing.as_deref() == Some(id.as_str()) => {
                    self.alias_editing = None;
                    Task::none()
                }
                Some(id) => {
                    self.alias_editing = Some(id);
                    focus_named("chat-alias")
                }
                None => Task::none(),
            },
            Message::ToggleFocus => {
                // Focus is for writing a note in the wide layout. Elsewhere there is nothing to turn on.
                if !self.focus && (!self.editing_note() || self.compact()) {
                    return Task::none();
                }
                self.focus = !self.focus;
                // Going in, the bar slides away. Coming out it is back at once.
                self.slide_bar.set(!self.focus);
                if !self.focus {
                    self.slide_bar.jump(true);
                }
                Task::none()
            }
            Message::TitleSubmit => focus_named("note-body"),
            Message::AddWord => {
                self.add_word();
                Task::none()
            }
            Message::TypedIgnored(typed) => self.type_ignored(&typed),
            Message::BackupTo(text) => {
                self.backup_to = text;
                Task::none()
            }
            Message::BackUp => self.start_backup(),
            Message::BackedUp(result) => {
                self.finish_backup(result);
                Task::none()
            }
            Message::ToggleTasksNote => {
                self.tasks_note_open = !self.tasks_note_open;
                Task::none()
            }
            Message::ToggleNoteSettings => {
                self.note_panel = !self.note_panel;
                self.show_context = true;
                self.context_shut = false;
                Task::none()
            }
            Message::RowHover(id, over) => {
                if over {
                    self.hover_row = Some(id);
                } else if self.hover_row.as_deref() == Some(id.as_str()) {
                    self.hover_row = None;
                }
                Task::none()
            }
            Message::AskDeleteNote(id) => {
                if self.selected.as_deref() != Some(id.as_str()) {
                    self.show_note(&id);
                }
                if self.selected.as_deref() == Some(id.as_str()) {
                    self.confirm_delete = true;
                }
                Task::none()
            }
            Message::FocusBar => {
                self.slide_bar.set(!self.slide_bar.open);
                Task::none()
            }
            Message::FinishAlias => {
                self.alias_editing = None;
                Task::none()
            }
            Message::WipeDraft(text) => {
                self.wipe_draft = text;
                Task::none()
            }
            Message::ExportNotes => self.export_notes(),
            Message::ConfirmWipe => self.wipe_notebook(),
            Message::Undo => {
                self.undo_note();
                self.note_follow(true)
            }
            Message::Redo => {
                self.redo_note();
                self.note_follow(true)
            }
            Message::TaskQuery(query) => {
                self.task_query = query;
                Task::none()
            }
            Message::BeginTaskEdit { id, line } => {
                let label = self
                    .tasks
                    .iter()
                    .find(|task| task.note_id == id && task.line == line)
                    .map(|task| task.text.clone())
                    .unwrap_or_default();
                markdown::due_day(&markdown::without_cites(&label))
                    .unwrap_or("")
                    .clone_into(&mut self.task_due_text);
                self.task_rename_text = markdown::task_words(&label);
                self.task_rename = Some((id, line));
                Task::none()
            }
            Message::TaskLabel(text) => {
                self.task_rename_text = text;
                Task::none()
            }
            Message::DueDraft(text) => {
                self.task_due_text = text;
                Task::none()
            }
            Message::CommitTaskRename => self.commit_task_rename(),
            Message::CancelTaskEdit => {
                self.task_rename = None;
                self.task_rename_text.clear();
                self.task_due_text.clear();
                Task::none()
            }
            Message::RemoveTask { id, line } => self.remove_listed_task(&id, line),
            Message::MoveTask { id, line, up } => self.move_listed_task(&id, line, up),
            Message::ToggleDone => {
                self.hide_done = !self.hide_done;
                Task::none()
            }
            Message::AskClearDone => {
                if self.tasks.iter().any(|task| task.done) {
                    self.confirm_clear = true;
                }
                Task::none()
            }
            Message::CancelClearDone => {
                self.confirm_clear = false;
                Task::none()
            }
            Message::ClearDone => self.clear_done_tasks(),
            Message::CopyChat(text) => {
                self.say("Message copied.");
                clipboard::write(text)
            }
            Message::ChatQuery(query) => {
                self.chat_query = query;
                Task::none()
            }
            Message::FindOpen => {
                self.find_open = !self.find_open;
                if self.find_open {
                    self.find_at = usize::MAX;
                    self.screen = Screen::Notes;
                    self.sheet = Sheet::None;
                    self.detail = true;
                }
                Task::none()
            }
            Message::FindQuery(query) => {
                self.find_query = query;
                self.find_at = usize::MAX;
                Task::none()
            }
            Message::FindNext => {
                self.seek_find(true);
                self.note_reveal()
            }
            Message::FindPrev => {
                self.seek_find(false);
                self.note_reveal()
            }
            Message::Mods { command, shift } => {
                self.command_down = command;
                self.shift_down = shift;
                Task::none()
            }
            Message::PickInvite(pick) => {
                self.invite_pick = Some(pick);
                Task::none()
            }
            Message::ToggleNoteSearch => self.toggle_note_search(),
            Message::ToggleTaskSearch => self.toggle_task_search(),
            Message::ToggleChatSearch => self.toggle_chat_search(),
            Message::WelcomeStored => {
                if !self.recovery_phrase.is_empty() {
                    self.welcome = Some(WelcomeStep::Guide);
                }
                Task::none()
            }
            Message::WelcomeDone => {
                self.welcome = None;
                self.seen = true;
                self.persist_profile();
                self.say("This notebook is on this computer.");
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        if self.breeze_on() {
            return self.breeze_screen();
        }
        if self.open_error == "this folder is locked" {
            return self.unlock_screen();
        }
        if self.notebook.is_none() && self.open_error.is_empty() {
            return self.onboarding_screen();
        }
        if self.welcome.is_some() {
            return self.welcome_screen();
        }
        if self.compact() {
            return column![
                self.compact_title(),
                hairline(),
                self.compact_body(),
                hairline(),
                self.bottom_bar(),
            ]
            .height(Fill)
            .into();
        }
        let focused = self.focused();
        let mut body = row![].width(Fill).height(Fill);
        if self.show_list && !focused {
            body = body.push(self.sidebar()).push(hairline_v());
        }
        if !focused {
            body = body.push(self.edge_toggle(true));
        }
        body = body.push(self.main());
        if self.context_room() && !focused {
            body = body.push(self.edge_toggle(false));
            if self.show_context {
                body = body.push(hairline_v()).push(self.context_pane());
            }
        }
        column![self.toolbar(), hairline(), body]
            .width(Fill)
            .height(Fill)
            .into()
    }

    fn toolbar(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let mut bar = row![
            text("Aerodash")
                .font(serif())
                .size(if self.size.width < 980.0 { 20 } else { 24 })
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center)
        .padding([10.0, 16.0]);
        if self.size.width >= 900.0 {
            bar = bar.push(
                text(self.message.as_str())
                    .size(13)
                    .color(ink.muted)
                    .width(Fill),
            );
        } else {
            bar = bar.push(space().width(Fill));
        }
        bar = bar.push(icon_button(
            gear_handle(self.is_dark()),
            Message::ShowSettings,
        ));
        bar = bar.push(icon_button(
            mode_handle(self.is_dark()),
            Message::ToggleTheme,
        ));
        if self.notebook.is_some() && !self.editor_dirty() {
            bar = bar.push(text("✓").size(18).color(Color::from_rgb8(46, 140, 96)));
        }
        bar = bar.push(anchored(
            icon_button(globe_handle(self.globe_color()), Message::ToggleSyncPop),
            self.sync_open.then(|| self.sync_card()),
        ));
        container(bar).width(Fill).into()
    }

    fn globe_color(&self) -> [u8; 3] {
        let on = if self.net_mode == ambient_cli::NetMode::ThisComputer {
            self.relay_ready
        } else {
            self.net_up
        };
        if !on {
            [140, 133, 122]
        } else if self.pending {
            [184, 78, 62]
        } else {
            [46, 140, 96]
        }
    }

    fn sync_card(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let on = if self.net_mode == ambient_cli::NetMode::ThisComputer {
            self.relay_ready
        } else {
            self.net_up
        };
        let relay = match self.net_mode {
            ambient_cli::NetMode::ThisComputer if self.relay_ready && self.relay_ours => {
                "Relay is on. Cleartext on this computer only. Not a public server."
            }
            ambient_cli::NetMode::ThisComputer if self.relay_ready => {
                "Relay is on, started by another window. Cleartext on this computer only."
            }
            ambient_cli::NetMode::ThisComputer => "Relay is off.",
            ambient_cli::NetMode::List => "Using the introduction list.",
            ambient_cli::NetMode::Pin => "Using a pinned relay.",
        };
        let switch = if self.relay_ready && self.relay_ours {
            "Turn relay off"
        } else if self.relay_ready {
            "Relay is not this window's"
        } else {
            "Turn relay on"
        };
        let state = if self.editor_dirty() {
            "Still writing."
        } else if on && self.pending {
            "Changes have not synced."
        } else if on {
            "Synced."
        } else {
            "Saved on this computer."
        };
        let cadence = if self.net_mode == ambient_cli::NetMode::ThisComputer && !self.relay_ready {
            "Turn the relay on to sync."
        } else if on {
            "Syncs on its own every few seconds."
        } else if self.sync_detail == "No relay answered." {
            "No relay answered."
        } else {
            "Syncs on its own every few seconds."
        };
        let line = iced::Length::Fixed(if self.advanced { 340.0 } else { 276.0 });
        let switch_button = if self.relay_ready && !self.relay_ours {
            ghost_label(switch, Message::ToggleRelay)
        } else {
            action_label(switch, Message::ToggleRelay)
        };
        let mut column = column![
            text(state).font(serif()).size(20).width(line),
            text(cadence).size(13).color(ink.muted).width(line),
            text(relay).size(13).color(ink.muted).width(line),
            text(self.sync_detail.as_str())
                .size(13)
                .color(ink.text)
                .width(line),
            switch_button,
            ghost_label(
                if self.advanced {
                    "Hide advanced"
                } else {
                    "Advanced"
                },
                Message::ToggleAdvanced,
            ),
        ]
        .spacing(8);
        if self.advanced {
            column = column
                .push(text_input("Pinned relay", &self.pin_draft).on_input(Message::PinDraft));
            column = column.push(
                text_input("Backup relay", &self.backup_draft).on_input(Message::BackupDraft),
            );
            column = column.push(action_label("Save pinned relay", Message::SavePinned));
            column = column.push(ghost_label("This computer", Message::UseThisComputer));
            if ambient_cli::INTRODUCTION.is_empty() {
                column = column.push(
                    text("No public relays are listed yet.")
                        .size(13)
                        .color(ink.muted)
                        .width(line),
                );
            }
        }
        container(column)
            .padding(14)
            .width(if self.advanced { 380 } else { 300 })
            .style(card_style)
            .into()
    }

    fn sidebar(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let tasks_name = self.tasks_tab();
        let chat_name = self.chat_tab();
        let modes = container(
            row![
                segment("Notes", self.screen == Screen::Notes, Message::ShowNotes),
                segment(
                    &tasks_name,
                    self.screen == Screen::Tasks,
                    Message::ShowTasks,
                ),
                segment(&chat_name, self.screen == Screen::Chat, Message::ShowChat),
            ]
            .spacing(4),
        )
        .padding(4)
        .width(Fill)
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Color(ink.bg)),
            border: iced::Border {
                radius: 10.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            ..Default::default()
        });
        let mut side = column![modes]
            .spacing(14)
            .padding(16)
            .width(Fill)
            .height(Fill);
        if self.screen == Screen::Chat {
            side = side.push(action_fill("Invite", Message::ShowInvite));
            if self.chats.is_empty() {
                side = side.push(text("No conversations yet.").size(13).color(ink.muted));
            }
            for chat in &self.chats {
                let selected = self.chat_id.as_deref() == Some(chat.id.as_str());
                let (preview, time) = self.chat_last.get(&chat.id).cloned().unwrap_or_default();
                side = side.push(chat_row(
                    &self.chat_name(chat),
                    &preview,
                    &time,
                    &chat.id,
                    selected,
                    self.chat_is_new(&chat.id),
                ));
            }
        } else if self.screen == Screen::Tasks {
            let open = self.tasks.iter().filter(|task| !task.done).count();
            side = side.push(text(format!("{open} open")).size(13).color(ink.muted));
            side = side.push(
                text("A box writes back into its note. One added here is the Tasks note.")
                    .size(13)
                    .color(ink.muted),
            );
        } else {
            side = side.push(self.note_index());
        }
        container(side)
            .width(self.side_width())
            .height(Fill)
            .style(surface_pane)
            .into()
    }

    fn toggle_note_search(&mut self) -> Task<Message> {
        if self.note_search || !self.query.trim().is_empty() {
            self.note_search = false;
            self.query.clear();
            self.refresh_shown();
            Task::none()
        } else {
            self.note_search = true;
            focus_named("note-search")
        }
    }

    fn toggle_task_search(&mut self) -> Task<Message> {
        if self.task_search || !self.task_query.trim().is_empty() {
            self.task_search = false;
            self.task_query.clear();
            Task::none()
        } else {
            self.task_search = true;
            focus_named("task-search")
        }
    }

    fn toggle_chat_search(&mut self) -> Task<Message> {
        if self.chat_search || !self.chat_query.trim().is_empty() {
            self.chat_search = false;
            self.chat_query.clear();
            Task::none()
        } else {
            self.chat_search = true;
            focus_named("chat-search")
        }
    }

    fn note_index(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let searching = self.note_search || !self.query.trim().is_empty();
        let mut block = column![row![
            space().width(Fill),
            mini_button(
                mini_handle(Mini::Search, self.is_dark()),
                if searching { "Close search" } else { "Search" },
                Message::ToggleNoteSearch,
            ),
        ]]
        .spacing(8)
        .width(Fill)
        .height(Fill);
        if searching {
            block = block.push(
                text_input("Search notes", &self.query)
                    .id(iced::widget::Id::new("note-search"))
                    .on_input(Message::QueryChanged)
                    .on_submit(Message::FieldDone)
                    .padding(10)
                    .size(14)
                    .style(field_style),
            );
        }
        if self.notes.is_empty() {
            let empty = if self.catalog.is_empty() {
                "No notes yet."
            } else {
                "No notes match."
            };
            block = block.push(text(empty).size(13).color(ink.muted));
        }
        let mut list = column![].spacing(2).width(Fill);
        let tagged = self.catalog.iter().any(|note| !note.tags.is_empty());
        if tagged {
            list = list.push(
                self.tag_row(
                    None,
                    self.catalog
                        .iter()
                        .filter(|note| !is_tasks_note(note))
                        .count(),
                ),
            );
            let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
            for note in self.catalog.iter().filter(|note| !is_tasks_note(note)) {
                for tag in &note.tags {
                    *counts.entry(tag.as_str()).or_default() += 1;
                }
            }
            for (tag, count) in counts {
                list = list.push(self.tag_row(Some(tag), count));
            }
        } else if !self.catalog.is_empty() {
            list = list.push(
                text("Tags are added on the note.")
                    .size(12)
                    .color(ink.muted),
            );
        }
        if self.query.trim().is_empty() && self.tag.is_none() {
            for note in &self.notes {
                if self.pieces.contains(&note.id) {
                    continue;
                }
                list = list.push(self.note_row(note, 0.0));
                if let Some((_, targets)) = self.groups.iter().find(|(id, _)| id == &note.id) {
                    for target in targets {
                        if let Some(piece) =
                            self.notes.iter().find(|item| Self::target_is(target, item))
                        {
                            list = list.push(self.note_row(piece, 16.0));
                        }
                    }
                }
            }
        } else {
            for note in &self.notes {
                list = list.push(self.note_row(note, 0.0));
            }
        }
        if !self.query.trim().is_empty() && !self.search_chats.is_empty() {
            list = list.push(text("Conversations").size(12).color(ink.muted).width(Fill));
            for chat in &self.search_chats {
                list = list.push(self.chat_hit_row(chat));
            }
        }
        block
            .push(vscroll(list).height(Fill).width(Fill))
            .push(action_fill("New note", Message::NewNote))
            .into()
    }

    fn tag_row(&self, tag: Option<&str>, count: usize) -> Element<'_, Message> {
        let ink = self.ink();
        let active = self.tag.as_deref() == tag;
        let label = tag.map_or_else(|| "All".to_owned(), ToOwned::to_owned);
        let chosen = tag.map(str::to_owned);
        button(
            row![
                text(label).size(13).width(Fill),
                text(count.to_string()).size(12).color(ink.muted),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .width(Fill)
        .padding([4.0, 8.0])
        .on_press(Message::PickTag(chosen))
        .style(list_row(active))
        .into()
    }

    fn note_row(&self, note: &NoteSummary, indent: f32) -> Element<'_, Message> {
        let ink = self.ink();
        let selected = self.selected.as_deref() == Some(note.id.as_str());
        let title = if note.title.is_empty() {
            "Untitled".to_owned()
        } else {
            note.title.clone()
        };
        let mut lines = column![text(title).font(serif()).size(17).width(Fill)].spacing(2);
        if !self.query.trim().is_empty()
            && let Some(line) = self
                .search_lines
                .get(&note.id)
                .filter(|line| !line.is_empty())
        {
            lines = lines.push(text(line.clone()).size(12).color(ink.muted).width(Fill));
        }
        let mut meta = note.writer.clone();
        if note.conflict {
            if !meta.is_empty() {
                meta.push_str(" · ");
            }
            meta.push_str("conflict");
        }
        if !note.tags.is_empty() {
            if !meta.is_empty() {
                meta.push_str(" · ");
            }
            for (index, tag) in note.tags.iter().take(3).enumerate() {
                if index > 0 {
                    meta.push(' ');
                }
                meta.push_str(tag);
            }
        }
        if !meta.is_empty() {
            lines = lines.push(text(meta).size(12).color(ink.muted).width(Fill));
        }
        let face = button(lines)
            .width(Fill)
            .padding(iced::Padding {
                top: 8.0,
                right: 10.0,
                bottom: 8.0,
                left: 10.0 + indent,
            })
            .on_press(Message::Select(note.id.clone()))
            .style(list_row(selected));
        let over = self.hover_row.as_deref() == Some(note.id.as_str());
        let mut layers = stack![face];
        if over {
            layers = layers.push(
                container(mini_button(
                    mini_handle(Mini::Trash, self.is_dark()),
                    "Delete this note",
                    Message::AskDeleteNote(note.id.clone()),
                ))
                .width(Fill)
                .height(Fill)
                .align_x(iced::alignment::Horizontal::Right)
                .align_y(iced::alignment::Vertical::Center)
                .padding([0.0, 4.0]),
            );
        }
        mouse_area(layers)
            .on_enter(Message::RowHover(note.id.clone(), true))
            .on_exit(Message::RowHover(note.id.clone(), false))
            .into()
    }

    fn chat_hit_row(&self, hit: &lane::ChatHit) -> Element<'_, Message> {
        let ink = self.ink();
        let label = self
            .aliases
            .get(&hit.id)
            .filter(|name| !name.is_empty())
            .cloned()
            .unwrap_or_else(|| hit.label.clone());
        let id = hit.id.clone();
        button(
            column![
                text(label).size(15).width(Fill),
                text(hit.line.clone()).size(12).color(ink.muted).width(Fill),
            ]
            .spacing(2),
        )
        .width(Fill)
        .padding([8.0, 10.0])
        .on_press(Message::SelectChat(id))
        .style(list_row(false))
        .into()
    }

    fn tasks_pane(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let add = row![
            text_input("Add a task", &self.task_draft)
                .id(iced::widget::Id::new("task-input"))
                .on_input(Message::TaskDraft)
                .on_submit(Message::AddTask)
                .padding(10)
                .width(Fill)
                .style(field_style),
            action_label("Add", Message::AddTask),
            ghost_label(
                if self.task_cite.is_some() {
                    "Cited"
                } else {
                    "Cite"
                },
                Message::ToggleTaskCite,
            ),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .width(Fill);
        let searching = self.task_search || !self.task_query.trim().is_empty();
        let mut tools = row![
            mini_button(
                mini_handle(Mini::Search, self.is_dark()),
                if searching { "Close search" } else { "Search" },
                Message::ToggleTaskSearch,
            ),
            ghost_label(
                if self.hide_done {
                    "Show done"
                } else {
                    "Hide done"
                },
                Message::ToggleDone,
            ),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);
        if self.tasks.iter().any(|task| task.done) {
            tools = tools.push(ghost_label("Clear done", Message::AskClearDone));
        }
        if self.catalog.iter().any(is_tasks_note) {
            tools = tools.push(ghost_label("The Tasks note", Message::ToggleTasksNote));
        }
        let list = self.task_groups(&ink);
        let mut page = column![add].spacing(12).width(Fill).height(Fill);
        if let Some((_, title)) = &self.task_cite {
            page = page.push(
                row![
                    text(title.clone()).size(13).width(Fill),
                    ghost_label("Remove", Message::ClearTaskCite),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            );
        }
        if self.task_cite_open {
            page = page.push(self.cite_menu(true));
        }
        if searching {
            page = page.push(
                text_input("Search tasks", &self.task_query)
                    .id(iced::widget::Id::new("task-search"))
                    .on_input(Message::TaskQuery)
                    .padding(10)
                    .width(Fill)
                    .style(field_style),
            );
        }
        page = page
            .push(tools)
            .push(text("This goes in a note named Tasks. A box in any other note shows up here too. A date at the end, like 2026-11-20, is the due day.").size(13).color(ink.muted));
        if self.confirm_clear {
            page = page.push(self.clear_confirm());
        }
        let page = page.push(vscroll(list).height(Fill).width(Fill));
        if self.tasks_note_open {
            return stack![
                page,
                container(self.tasks_note_card())
                    .width(Fill)
                    .height(Fill)
                    .align_x(iced::alignment::Horizontal::Center)
                    .align_y(iced::alignment::Vertical::Center),
            ]
            .width(Fill)
            .height(Fill)
            .into();
        }
        page.into()
    }

    fn task_groups(&self, ink: &Ink) -> Element<'_, Message> {
        let mut list = column![].spacing(14).width(Fill);
        let open: Vec<&TaskRow> = self
            .tasks
            .iter()
            .filter(|task| !task.done && self.task_hit(task))
            .collect();
        let done: Vec<&TaskRow> = self
            .tasks
            .iter()
            .filter(|task| task.done && self.task_hit(task) && !self.hide_done)
            .collect();
        if self.tasks.is_empty() {
            list = list.push(text("Nothing to do yet.").font(serif()).size(32));
            list = list.push(
                text("Or write a line like - [ ] call the clinic in any note.")
                    .size(14)
                    .color(ink.muted),
            );
        } else if open.is_empty() && done.is_empty() {
            let empty = if self.task_query.trim().is_empty() {
                "Nothing open."
            } else {
                "No tasks match."
            };
            list = list.push(text(empty).font(serif()).size(32));
        } else if open.is_empty() {
            list = list.push(text("Nothing open.").font(serif()).size(32));
        } else {
            list = list.push(text("Open").font(serif()).size(32));
            for task in open {
                list = list.push(self.task_row(task));
            }
        }
        if !done.is_empty() {
            list = list.push(text("Done").font(serif()).size(22));
            for task in done {
                list = list.push(self.task_row(task));
            }
        }
        list.into()
    }

    fn task_hit(&self, task: &TaskRow) -> bool {
        let query = self.task_query.trim();
        if query.is_empty() {
            return true;
        }
        let query = query.to_lowercase();
        task.text.to_lowercase().contains(&query) || task.note_title.to_lowercase().contains(&query)
    }

    fn task_row(&self, task: &TaskRow) -> Element<'_, Message> {
        let ink = self.ink();
        let id = task.note_id.clone();
        let line = task.line;
        let open_id = task.note_id.clone();
        let title = if task.note_title.is_empty() {
            "Untitled".to_owned()
        } else {
            task.note_title.clone()
        };
        let editing = self
            .task_rename
            .as_ref()
            .is_some_and(|(note, row)| note == &task.note_id && *row == task.line);
        let words = if task.done { ink.muted } else { ink.text };
        let main: Element<'_, Message> = if editing {
            row![
                text_input("Task", &self.task_rename_text)
                    .on_input(Message::TaskLabel)
                    .on_submit(Message::CommitTaskRename)
                    .padding(8)
                    .width(Fill)
                    .style(field_style),
                text_input("2026-11-20", &self.task_due_text)
                    .on_input(Message::DueDraft)
                    .on_submit(Message::CommitTaskRename)
                    .padding(8)
                    .width(140)
                    .style(field_style),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .into()
        } else {
            let plain = markdown::without_cites(&task.text);
            let words_text = markdown::task_words(&task.text);
            let label = if words_text.is_empty() {
                "Empty task".to_owned()
            } else {
                words_text
            };
            let mut lines = column![text(label).size(16).color(words)]
                .spacing(2)
                .width(Fill);
            if let Some(day) = markdown::due_day(&plain) {
                lines = lines.push(text(format!("Due {day}")).size(12).color(ink.muted));
            }
            lines = lines.push(text(title).size(12).color(ink.muted));
            button(lines)
                .width(Fill)
                .padding([2, 0])
                .on_press(Message::Select(open_id))
                .style(quiet_style)
                .into()
        };
        let line_row = row![
            checkbox(task.done).on_toggle(move |done| Message::SetTask {
                id: id.clone(),
                line,
                done,
            }),
            main,
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center)
        .width(Fill);
        let mut block = column![line_row].spacing(4).width(Fill);
        for (target, title) in markdown::task_cites(&task.text) {
            let target = target.clone();
            block = block.push(
                button(text(title).size(13))
                    .padding([2, 0])
                    .on_press(Message::OpenCited(target))
                    .style(quiet_style),
            );
        }
        block.push(self.task_actions(task, editing)).into()
    }

    fn task_actions(&self, task: &TaskRow, editing: bool) -> Element<'_, Message> {
        let line = task.line;
        let dark = self.is_dark();
        if editing {
            return row![ghost_label("Cancel", Message::CancelTaskEdit)]
                .spacing(8)
                .into();
        }
        let mut actions = row![].spacing(4);
        if self.task_neighbor(task, true) {
            actions = actions.push(mini_button(
                mini_handle(Mini::Up, dark),
                "Up",
                Message::MoveTask {
                    id: task.note_id.clone(),
                    line,
                    up: true,
                },
            ));
        }
        if self.task_neighbor(task, false) {
            actions = actions.push(mini_button(
                mini_handle(Mini::Down, dark),
                "Down",
                Message::MoveTask {
                    id: task.note_id.clone(),
                    line,
                    up: false,
                },
            ));
        }
        actions = actions.push(mini_button(
            mini_handle(Mini::Pencil, dark),
            "Rename",
            Message::BeginTaskEdit {
                id: task.note_id.clone(),
                line,
            },
        ));
        actions = actions.push(mini_button(
            mini_handle(Mini::Trash, dark),
            "Remove",
            Message::RemoveTask {
                id: task.note_id.clone(),
                line,
            },
        ));
        actions.into()
    }

    fn rebuild_tasks(&mut self) {
        let selected = self.selected.clone();
        let unsaved = self.unsaved.clone();
        let mut rows = Vec::new();
        if let Some(book) = self.notebook.as_ref() {
            let _ = book.visit_held(|id, title, body, conflict| {
                if selected.as_deref() == Some(id) {
                    return;
                }
                if let Some((_, draft_title, draft_body)) = unsaved
                    .iter()
                    .find(|(key, _, _)| key.as_deref() == Some(id))
                {
                    push_tasks(&mut rows, id, draft_title, draft_body);
                    return;
                }
                if !conflict {
                    push_tasks(&mut rows, id, title, body);
                }
            });
        }
        if let Some(id) = &selected
            && self.conflicts.is_empty()
        {
            push_tasks(&mut rows, id, &self.title_text, &self.body.text());
        }
        rows.sort_by(|left, right| {
            left.done
                .cmp(&right.done)
                .then_with(|| {
                    match (
                        markdown::due_day(&left.text),
                        markdown::due_day(&right.text),
                    ) {
                        (Some(earlier), Some(later)) => earlier.cmp(later),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => std::cmp::Ordering::Equal,
                    }
                })
                .then_with(|| left.note_title.cmp(&right.note_title))
                .then_with(|| left.line.cmp(&right.line))
        });
        self.tasks = rows;
    }

    fn task_source(&self, id: &str) -> Option<(String, bool, String)> {
        if self.selected.as_deref() == Some(id) {
            return Some((
                self.body.text(),
                !self.conflicts.is_empty(),
                self.title_text.clone(),
            ));
        }
        if let Some((_, title, body)) = self
            .unsaved
            .iter()
            .find(|(key, _, _)| key.as_deref() == Some(id))
        {
            return Some((body.clone(), false, title.clone()));
        }
        let shown = self.notebook.as_ref()?.show_held(id).ok()?;
        let head = shown.heads.first()?;
        Some((head.body.clone(), shown.conflict, head.title.clone()))
    }

    fn set_task(&mut self, id: &str, line: usize, done: bool) -> Task<Message> {
        let Some((body, conflict, title)) = self.task_source(id) else {
            self.say("That note is not here.");
            return Task::none();
        };
        if conflict {
            self.show_note(id);
            self.say("Two versions. Write the one you want to keep, then save.");
            return Task::none();
        }
        let Some(next) = markdown::set_task_line(&body, line, done) else {
            return Task::none();
        };
        self.put_note_text(id, &title, &next)
    }

    fn commit_task_rename(&mut self) -> Task<Message> {
        let Some((id, line)) = self.task_rename.clone() else {
            return Task::none();
        };
        let label = self.task_rename_text.clone();
        let marks = self
            .tasks
            .iter()
            .find(|task| task.note_id == id && task.line == line)
            .map(|task| markdown::cite_marks(&task.text))
            .unwrap_or_default();
        if label.trim().is_empty() && self.task_due_text.trim().is_empty() {
            self.say("Write the task first.");
            return Task::none();
        }
        let due = self.task_due_text.trim();
        if !due.is_empty() && !markdown::is_due_day(due) {
            self.say("Use a date like 2026-11-20.");
            return Task::none();
        }
        let mut label = markdown::join_due(&label, due);
        for mark in marks {
            label.push(' ');
            label.push_str(&mark);
        }
        let Some((body, conflict, title)) = self.task_source(&id) else {
            self.say("That note is not here.");
            return Task::none();
        };
        if conflict {
            self.show_note(&id);
            self.say("Two versions. Write the one you want to keep, then save.");
            return Task::none();
        }
        self.task_rename = None;
        self.task_rename_text.clear();
        self.task_due_text.clear();
        let Some(next) = markdown::rename_task_line(&body, line, &label) else {
            return Task::none();
        };
        self.store_task_edit(&id, &title, &next)
    }

    fn remove_listed_task(&mut self, id: &str, line: usize) -> Task<Message> {
        if self
            .task_rename
            .as_ref()
            .is_some_and(|(note, row)| note == id && *row == line)
        {
            self.task_rename = None;
            self.task_rename_text.clear();
        }
        let Some((body, conflict, title)) = self.task_source(id) else {
            self.say("That note is not here.");
            return Task::none();
        };
        if conflict {
            self.show_note(id);
            self.say("Two versions. Write the one you want to keep, then save.");
            return Task::none();
        }
        let Some(next) = markdown::remove_task_line(&body, line) else {
            return Task::none();
        };
        self.store_task_edit(id, &title, &next)
    }

    fn move_listed_task(&mut self, id: &str, line: usize, up: bool) -> Task<Message> {
        let Some((body, conflict, title)) = self.task_source(id) else {
            self.say("That note is not here.");
            return Task::none();
        };
        if conflict {
            self.show_note(id);
            self.say("Two versions. Write the one you want to keep, then save.");
            return Task::none();
        }
        let Some(next) = markdown::move_task_line(&body, line, up) else {
            return Task::none();
        };
        self.store_task_edit(id, &title, &next)
    }

    fn clear_done_tasks(&mut self) -> Task<Message> {
        self.confirm_clear = false;
        let mut by_note: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for task in &self.tasks {
            if task.done {
                by_note
                    .entry(task.note_id.clone())
                    .or_default()
                    .push(task.line);
            }
        }
        let mut writes = Vec::new();
        for (id, mut lines) in by_note {
            lines.sort_unstable();
            lines.dedup();
            let Some((body, conflict, title)) = self.task_source(&id) else {
                continue;
            };
            if conflict {
                self.show_note(&id);
                self.say("Two versions. Write the one you want to keep, then save.");
                continue;
            }
            let mut next = body.clone();
            for line in lines.into_iter().rev() {
                if let Some(updated) = markdown::remove_task_line(&next, line) {
                    next = updated;
                }
            }
            if next != body {
                writes.push(self.store_task_edit(&id, &title, &next));
            }
        }
        if writes.is_empty() {
            Task::none()
        } else {
            self.say("Cleared. The older lines stay in the log.");
            Task::batch(writes)
        }
    }

    fn task_neighbor(&self, task: &TaskRow, up: bool) -> bool {
        self.tasks.iter().any(|other| {
            other.note_id == task.note_id
                && if up {
                    other.line < task.line
                } else {
                    other.line > task.line
                }
        })
    }

    fn clear_confirm(&self) -> Element<'_, Message> {
        let ink = self.ink();
        column![
            text(
                "Clear done tasks? Those lines leave their notes. The older copy stays in the log."
            )
            .size(14)
            .color(ink.muted),
            row![
                action_label("Clear done", Message::ClearDone),
                ghost_label("Cancel", Message::CancelClearDone),
            ]
            .spacing(8),
        ]
        .spacing(8)
        .into()
    }

    fn tasks_tab(&self) -> String {
        let open = self.tasks.iter().filter(|task| !task.done).count();
        if open == 0 {
            "Tasks".to_owned()
        } else {
            format!("Tasks {open}")
        }
    }

    fn chat_tab(&self) -> String {
        let fresh = self
            .chats
            .iter()
            .filter(|chat| self.chat_is_new(&chat.id))
            .count();
        if fresh == 0 {
            "Chat".to_owned()
        } else {
            format!("Chat {fresh}")
        }
    }

    fn chat_is_new(&self, id: &str) -> bool {
        match (
            self.chat_count.get(id).copied(),
            self.seen_count.get(id).copied(),
        ) {
            (Some(count), Some(seen)) => count > seen,
            _ => false,
        }
    }

    fn store_task_edit(&mut self, id: &str, title: &str, next: &str) -> Task<Message> {
        let follow = self.put_note_text(id, title, next);
        if self.editor_dirty() && self.selected.as_deref() == Some(id) {
            Task::batch([follow, self.start_save(true)])
        } else {
            follow
        }
    }

    fn put_note_text(&mut self, id: &str, title: &str, next: &str) -> Task<Message> {
        if self.selected.as_deref() == Some(id) {
            let title = title.to_owned();
            let next = next.to_owned();
            self.revise(Burst::Break, |app| {
                let before = app.body.text();
                let before_title = app.title_text.clone();
                let cursor = app.body.cursor();
                app.title_text.clone_from(&title);
                app.body.load(&next);
                app.body.place(cursor);
                app.body.text() != before || app.title_text != before_title
            });
            if self.screen == Screen::Tasks {
                self.rebuild_tasks();
            }
            return Task::none();
        }
        if let Some(slot) = self
            .unsaved
            .iter_mut()
            .find(|(key, _, _)| key.as_deref() == Some(id))
        {
            title.clone_into(&mut slot.1);
            next.clone_into(&mut slot.2);
        }
        self.apply_note(Some(id.to_owned()), title.to_owned(), next.to_owned())
    }

    fn main(&self) -> Element<'_, Message> {
        let body = match self.sheet {
            Sheet::Invite => self.invite_sheet(),
            Sheet::Settings => self.settings_sheet(),
            Sheet::None if self.screen == Screen::Chat => self.chat_pane(),
            Sheet::None if self.screen == Screen::Tasks => self.tasks_pane(),
            Sheet::None => self.notes_pane(),
        };
        container(body)
            .padding([self.gutter(), self.gutter_x()])
            .width(Fill)
            .height(Fill)
            .into()
    }

    fn unlock_screen(&self) -> Element<'_, Message> {
        let ink = self.ink();
        container(
            column![
                text("This folder is locked").font(serif()).size(32),
                text("The passphrase unwraps the key file for this run. It is not the root, not the recovery phrase, and not a platform passkey.")
                    .size(14)
                    .color(ink.muted),
                text_input("Passphrase", &self.unlock_input)
                    .on_input(Message::UnlockChanged)
                    .on_submit(Message::SubmitUnlock)
                    .secure(true)
                    .padding(10)
                    .style(field_style),
                action_label("Unlock", Message::SubmitUnlock),
                text(self.message.as_str()).size(13).color(ink.muted),
            ]
            .spacing(14)
            .max_width(460),
        )
        .padding(48)
        .width(Fill)
        .height(Fill)
        .into()
    }

    fn notes_pane(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let mut body = column![].spacing(14).height(Fill);
        if self.compact() && self.detail {
            body = body.push(
                row![
                    ghost_label("‹", Message::ShowNotes),
                    space().width(Fill),
                    ghost_label(
                        if self.show_context { "›" } else { "‹" },
                        Message::ToggleContext,
                    ),
                ]
                .align_y(iced::Alignment::Center),
            );
        }
        if !self.open_error.is_empty() {
            body = body.push(text(format!(
                "Could not open this folder. {}",
                self.open_error
            )));
        }
        if !self.conflicts.is_empty() {
            let mut versions = column![
                text("Two versions. Write the one you want to keep, then save.").color(ink.muted)
            ]
            .spacing(8);
            for (index, (writer, title, note_body)) in self.conflicts.iter().enumerate() {
                let number = index.saturating_add(1);
                versions = versions.push(text(format!("Version {number} · {writer}: {title}")));
                versions = versions.push(text(note_body).color(ink.muted));
            }
            body = body.push(vscroll(versions).height(160));
        }
        body = body.push(self.note_column());
        let page = if self.send_open {
            body = body.push(text("Which chat?").size(13).color(ink.muted));
            let mut choices = column![].spacing(4).width(Fill);
            for chat in &self.chats {
                let chosen = self.send_choice.as_deref() == Some(chat.id.as_str());
                let id = chat.id.clone();
                choices = choices.push(
                    button(text(self.chat_name(chat)).size(15).width(Fill))
                        .width(Fill)
                        .padding([8.0, 10.0])
                        .on_press(Message::ChooseSend(id))
                        .style(list_row(chosen)),
                );
            }
            let list_h = self
                .chats
                .iter()
                .fold(0.0_f32, |height, _| (height + 40.0).min(168.0))
                .max(40.0);
            body = body.push(vscroll(choices).height(list_h).width(Fill));
            body = body.push(
                row![
                    action_label("Send", Message::ConfirmSend),
                    ghost_label("Cancel", Message::CancelSend),
                ]
                .spacing(8),
            );
            body.into()
        } else if self.confirm_delete {
            Self::overlay_corner(body.into(), self.delete_confirm())
        } else if self.compact() {
            self.overlay_tools(body.into(), true)
        } else {
            body.into()
        };
        self.focus_chrome(page)
    }

    /// The icon at the top right of a note that goes in and out of focus. In focus it has the
    /// small title beside it, and a button that brings the formatting bar in or sends it away.
    fn focus_chrome<'a>(&'a self, page: Element<'a, Message>) -> Element<'a, Message> {
        if self.compact() || self.reading || self.older.is_some() || !self.editing_note() {
            return page;
        }
        let mut corner = row![].spacing(6).align_y(iced::Alignment::Center);
        if self.focused() {
            corner = corner
                .push(
                    text_input("Title", &self.title_text)
                        .id(iced::widget::Id::new("note-title"))
                        .on_input(Message::TitleChanged)
                        .on_submit(Message::TitleSubmit)
                        .font(serif())
                        .size(15)
                        .padding([2.0, 0.0])
                        .width(220)
                        .align_x(iced::alignment::Horizontal::Right)
                        .style(title_style),
                )
                .push(ghost_label("Aa", Message::FocusBar));
        }
        corner = corner.push(mini_button(
            mini_handle(Mini::Focus, self.is_dark()),
            if self.focus {
                "Leave focus"
            } else {
                "Write without distractions"
            },
            Message::ToggleFocus,
        ));
        stack![
            page,
            container(corner)
                .width(Fill)
                .height(Fill)
                .align_x(iced::alignment::Horizontal::Right)
                .align_y(iced::alignment::Vertical::Top)
                .padding(iced::Padding {
                    top: 0.0,
                    right: 2.0,
                    bottom: 0.0,
                    left: 0.0,
                }),
        ]
        .width(Fill)
        .height(Fill)
        .into()
    }

    /// Put `card` over the page in the lower right corner, where the round buttons are. A
    /// question about something those buttons did appears under the pointer, not across the page.
    fn overlay_corner<'a>(
        page: Element<'a, Message>,
        card: Element<'a, Message>,
    ) -> Element<'a, Message> {
        stack![
            page,
            container(card)
                .width(Fill)
                .height(Fill)
                .align_x(iced::alignment::Horizontal::Right)
                .align_y(iced::alignment::Vertical::Bottom)
                .padding(iced::Padding {
                    top: 0.0,
                    right: 2.0,
                    bottom: 2.0,
                    left: 0.0,
                }),
        ]
        .width(Fill)
        .height(Fill)
        .into()
    }

    /// The round buttons, on a narrow screen. A wide window has the same things in the left
    /// panel (add, delete on a row) and the note's settings (send to a chat).
    fn overlay_tools<'a>(
        &'a self,
        page: Element<'a, Message>,
        editing: bool,
    ) -> Element<'a, Message> {
        let (cluster, _) = self.tool_cluster(editing);
        Self::overlay_corner(page, cluster)
    }

    /// The round buttons, and how many there are.
    fn tool_cluster(&self, editing: bool) -> (Element<'_, Message>, usize) {
        let mut tools = row![circle_button("+", "New note", Message::NewNote)].spacing(8);
        let mut count = 1;
        if editing && (self.selected.is_some() || self.editor_dirty()) {
            let hint = if self.selected.is_some() {
                "Delete"
            } else {
                "Discard"
            };
            tools = tools.push(circle_button("×", hint, Message::AskDelete));
            count += 1;
        }
        if editing
            && (self.selected.is_some()
                || !self.body.text().is_empty()
                || !self.title_text.is_empty())
        {
            tools = tools.push(circle_button("↗", "Send to chat", Message::SendNoteToChat));
            count += 1;
        }
        (tools.into(), count)
    }

    fn delete_confirm(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let saved = self.selected.is_some();
        let prompt = if saved {
            "Delete this note? It leaves the list. Unsaved edits are dropped. The older copy stays in the log."
        } else {
            "Discard this unsaved note?"
        };
        let verb = if saved { "Delete note" } else { "Discard" };
        container(
            column![
                text(prompt).size(14).color(ink.muted),
                row![
                    ghost_label("Cancel", Message::CancelDelete),
                    space().width(Fill),
                    action_label(verb, Message::DeleteNote),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            ]
            .spacing(12),
        )
        .width(320)
        .padding(14)
        .style(card_style)
        .into()
    }

    fn chat_pane(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let mut lines = column![].spacing(8);
        if self.chat.is_empty() && self.pending_files.is_empty() {
            lines = lines.push(text("No messages yet.").color(ink.muted));
            lines = lines.push(action_label("Invite someone", Message::ShowInvite));
        }
        let query = self.chat_query.trim().to_lowercase();
        let width = self.bubble_width();
        let mut day = String::new();
        let mut shown = 0_usize;
        for line in &self.chat {
            if !query.is_empty() && !line.text.to_lowercase().contains(&query) {
                continue;
            }
            shown = shown.saturating_add(1);
            if !line.day.is_empty() && line.day != day {
                day.clone_from(&line.day);
                lines = lines.push(
                    container(text(&line.day).size(12).color(ink.muted))
                        .width(Fill)
                        .align_x(iced::alignment::Horizontal::Center)
                        .padding([6, 0]),
                );
            }
            lines = lines.push(self.chat_bubble(line, width));
        }
        if shown == 0 && !query.is_empty() {
            lines = lines.push(text("No messages match.").color(ink.muted));
        }
        let composer = self.composer();
        let mut page = column![].spacing(12).width(Fill).height(Fill);
        if self.chat_id.is_some() {
            page = page.push(self.chat_header());
        }
        page = page.push(
            vscroll(lines.width(Fill))
                .height(Fill)
                .width(Fill)
                .anchor_bottom(),
        );
        page = page.push(composer);
        container(page).width(Fill).height(Fill).clip(true).into()
    }

    fn composer(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let narrow = self.chat_inner() < 420.0;
        let field = text_editor::TextEditor::new(&self.draft_editor)
            .placeholder("Message")
            .height(72)
            .padding(10)
            .on_action(Message::DraftEdited)
            .key_binding(composer_binding)
            .style(field_editor_style);
        let controls: Element<'_, Message> = if narrow {
            column![
                field,
                row![
                    ghost_label("Add", Message::PickFile),
                    ghost_label(
                        if self.chat_cite.is_some() {
                            "Cited"
                        } else {
                            "Cite"
                        },
                        Message::ToggleChatCite,
                    ),
                    space().width(Fill),
                    action_label("Send", Message::SendChat),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .width(Fill),
            ]
            .spacing(8)
            .width(Fill)
            .into()
        } else {
            row![
                ghost_label("Add", Message::PickFile),
                ghost_label(
                    if self.chat_cite.is_some() {
                        "Cited"
                    } else {
                        "Cite"
                    },
                    Message::ToggleChatCite,
                ),
                field,
                action_label("Send", Message::SendChat),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .width(Fill)
            .into()
        };
        let mut composer = column![].spacing(8).width(Fill);
        if !self.pending_files.is_empty() {
            let mut chips = column![].spacing(4);
            for (index, path) in self.pending_files.iter().enumerate() {
                let name = path
                    .file_name()
                    .and_then(|text| text.to_str())
                    .unwrap_or("file");
                chips = chips.push(
                    row![
                        text(name).size(13).width(Fill),
                        ghost_label_owned("Remove".to_owned(), Message::DropPending(index)),
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                );
            }
            composer = composer.push(chips);
            composer = composer.push(
                text(
                    "The file stays on this computer. The message can sync. The file cannot, yet.",
                )
                .size(12)
                .color(ink.muted),
            );
        }
        if let Some((_, title)) = &self.chat_cite {
            composer = composer.push(
                row![
                    text(title.clone()).size(13).width(Fill),
                    ghost_label("Remove", Message::ClearChatCite),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            );
        }
        if self.cite_open {
            composer = composer.push(self.cite_menu(false));
        }
        composer
            .push(controls)
            .push(
                text("Shift+Enter for a new line.")
                    .size(12)
                    .color(ink.muted),
            )
            .into()
    }

    fn chat_header(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let Some(id) = &self.chat_id else {
            return space().height(0).into();
        };
        let editing = self.alias_editing.as_deref() == Some(id.as_str());
        let searching = self.chat_search || !self.chat_query.trim().is_empty();
        let search = mini_button(
            mini_handle(Mini::Search, self.is_dark()),
            if searching { "Close search" } else { "Search" },
            Message::ToggleChatSearch,
        );
        // The name is plain text. The pencil turns it into a field, so the header does not
        // look like a form every time the chat is open.
        let mut head = column![].spacing(4);
        if editing {
            let draft = self.aliases.get(id).cloned().unwrap_or_default();
            let id = id.clone();
            head = head
                .push(
                    row![
                        text_input("What do you call them?", &draft)
                            .id(iced::widget::Id::new("chat-alias"))
                            .on_input(move |text| Message::AliasDraft(id.clone(), text))
                            .on_submit(Message::FinishAlias)
                            .padding(10)
                            .width(Fill)
                            .style(field_style),
                        ghost_label("Done", Message::FinishAlias),
                        search,
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                )
                .push(
                    text("Only this computer remembers it.")
                        .size(12)
                        .color(ink.muted),
                );
        } else {
            let name = self
                .chats
                .iter()
                .find(|chat| &chat.id == id)
                .map_or_else(|| "Conversation".to_owned(), |chat| self.chat_name(chat));
            head = head.push(
                row![
                    text(name).font(serif()).size(22).width(Fill),
                    mini_button(
                        mini_handle(Mini::Pencil, self.is_dark()),
                        "Rename",
                        Message::EditAlias,
                    ),
                    search,
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            );
        }
        if searching {
            head = head.push(
                text_input("Search this conversation", &self.chat_query)
                    .id(iced::widget::Id::new("chat-search"))
                    .on_input(Message::ChatQuery)
                    .padding(10)
                    .style(field_style),
            );
        }
        head.into()
    }

    fn invite_sheet(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let mut sheet = column![
            Self::sheet_title("Invite"),
            text("One of you starts. The other waits.")
                .size(14)
                .color(ink.muted),
            row![
                choice_button(
                    "I have their link",
                    self.invite_pick == Some(InvitePick::Theirs),
                    Message::PickInvite(InvitePick::Theirs),
                ),
                choice_button(
                    "Send them my link",
                    self.invite_pick == Some(InvitePick::Mine),
                    Message::PickInvite(InvitePick::Mine),
                ),
            ]
            .spacing(8)
            .width(Fill),
        ]
        .spacing(14);
        sheet = match self.invite_pick {
            Some(InvitePick::Theirs) => sheet.push(self.invite_theirs()),
            Some(InvitePick::Mine) => sheet.push(self.invite_mine()),
            None => sheet.push(
                text("Pick one. You do not do both.")
                    .size(13)
                    .color(ink.muted),
            ),
        };
        vscroll(sheet).height(Fill).into()
    }

    fn invite_theirs(&self) -> Element<'_, Message> {
        let ink = self.ink();
        column![
            text("Paste the link they sent you. You start the conversation. They wait.")
                .size(14)
                .color(ink.muted),
            text_input("Their link", &self.their_card)
                .on_input(Message::TheirCardChanged)
                .padding(10)
                .style(field_style),
            text("What do you call them? Leave it blank if you want. Only this computer remembers it.")
                .size(13)
                .color(ink.muted),
            text_input("A name on this computer", &self.call_draft)
                .on_input(Message::CallDraft)
                .padding(10)
                .style(field_style),
            action_label("Start conversation", Message::StartConversation),
        ]
        .spacing(12)
        .into()
    }

    fn invite_mine(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let mut body = column![
            text("Copy this and send it. They paste it and start. You wait. Do not paste anything here.")
                .size(14)
                .color(ink.muted),
            text("Your link").size(12).color(ink.muted),
            container(text(&self.contact_link).size(14))
                .padding(14)
                .width(Fill)
                .style(field_well),
            row![
                ghost_label("Copy link", Message::CopyLink),
                ghost_label(
                    if self.show_qr { "Hide QR" } else { "Show QR" },
                    Message::ToggleQr,
                ),
            ]
            .spacing(8),
        ]
        .spacing(12);
        if self.show_qr
            && let Some(bytes) = &self.qr_bytes
        {
            body = body.push(
                iced_image::Image::new(iced_image::Handle::from_bytes(bytes.clone()))
                    .width(200)
                    .height(200),
            );
        }
        body = body.push(
            text("They show up in Chat once they start. Nothing else happens on this page.")
                .size(13)
                .color(ink.muted),
        );
        body.into()
    }

    fn devices_sheet(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let mut body =
            column![
            text("A passphrase locks the key file on this computer. It is not the recovery phrase.")
                .size(14)
                .color(ink.muted),
            text(self.dir.display().to_string())
                .size(13)
                .color(ink.muted),
            text_input("Passphrase", &self.unlock_input)
                .on_input(Message::UnlockChanged)
                .secure(true)
                .padding(12)
                .style(field_style),
            action_label("Lock the key file", Message::SubmitUnlock),
        ]
            .spacing(12);
        body = body.push(text("Computers").size(13).color(ink.muted));
        if self.devices.is_empty() {
            body = body.push(text("No other computers yet.").size(14).color(ink.muted));
        }
        let can_revoke = self.notebook.as_ref().is_some_and(Notebook::holds_root);
        for device in &self.devices {
            body = body.push(Self::device_row(device, can_revoke));
        }
        if self.notebook.as_ref().is_some_and(Notebook::holds_root) {
            body = body.push(
                text("Another window on this computer does not hold the main key. Revoke only stops new notes. Notes already written stay.")
                    .size(14)
                    .color(ink.muted),
            );
            body = body.push(
                row![
                    ghost_label("Open the other window", Message::OpenPaired),
                    ghost_label("Open another person", Message::OpenPerson),
                ]
                .spacing(8),
            );
        }
        settings_card("Devices", body.into())
    }

    fn device_row(device: &ambient_cli::DeviceLine, can_revoke: bool) -> Element<'static, Message> {
        let state = if device.revoked {
            "Revoked"
        } else if device.current {
            "This computer"
        } else {
            "Active"
        };
        let label = device.label.clone();
        let mut line = row![
            column![
                text(label.clone()).size(16),
                text(state).size(12).style(|theme: &Theme| {
                    let ink = ink_of(theme);
                    iced::widget::text::Style {
                        color: Some(ink.muted),
                    }
                }),
            ]
            .spacing(2)
            .width(Fill)
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center);
        if can_revoke && !device.current && !device.revoked {
            line = line.push(ghost_label_owned(
                format!("Revoke {label}"),
                Message::Revoke(device.id.clone()),
            ));
        }
        line.into()
    }

    fn recovery_sheet(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let mut body = column![
            text("This phrase can rebuild the notebook on a new computer. It does not bring back old device keys. Keep it offline.")
                .size(14)
                .color(ink.muted),
            action_label("Show recovery phrase", Message::ShowPhrase),
        ]
        .spacing(14);
        if !self.recovery_phrase.is_empty() {
            body = body.push(
                container(text(&self.recovery_phrase).size(15))
                    .padding(14)
                    .width(Fill)
                    .style(field_well),
            );
            body = body.push(ghost_label("Copy phrase", Message::CopyPhrase));
        }
        body = body.push(
            text_input("Paste a recovery phrase", &self.restore_phrase)
                .on_input(Message::RestorePhraseChanged)
                .padding(12)
                .style(field_style),
        );
        body = body.push(action_label(
            "Restore on a new computer",
            Message::StartRestore,
        ));
        settings_card("Recovery", body.into())
    }

    fn sheet_title(title: &str) -> Element<'static, Message> {
        let title = title.to_owned();
        row![
            text(title).font(serif()).size(32).width(Fill),
            ghost_label("Back", Message::CloseSheet),
        ]
        .align_y(iced::Alignment::Center)
        .into()
    }

    fn reload(&mut self) {
        let found = {
            let Some(book) = self.notebook.as_ref() else {
                self.tasks.clear();
                self.catalog.clear();
                self.notes.clear();
                return;
            };
            book.list_held()
        };
        match found {
            Ok(notes) => self.catalog = notes,
            Err(err) => self.message = err.to_string(),
        }
        if self.tag.as_ref().is_some_and(|tag| {
            !self
                .catalog
                .iter()
                .any(|note| note.tags.iter().any(|item| item == tag))
        }) {
            self.tag = None;
        }
        self.refresh_shown();
        if self.screen == Screen::Tasks {
            self.rebuild_tasks();
        }
    }

    fn refresh_shown(&mut self) {
        let base = if self.query.is_empty() {
            self.catalog.clone()
        } else if let Some(hits) = &self.search_hits {
            hits.clone()
        } else {
            let needle = self.query.to_lowercase();
            self.catalog
                .iter()
                .filter(|note| note.title.to_lowercase().contains(&needle))
                .cloned()
                .collect()
        };
        // The Tasks note is the Tasks tab's. It is not in the list of notes.
        let base = base.into_iter().filter(|note| !is_tasks_note(note));
        self.notes = match &self.tag {
            Some(tag) => base
                .filter(|note| note.tags.iter().any(|item| item == tag))
                .collect(),
            None => base.collect(),
        };
    }

    /// The Tasks note's words, for showing it over the Tasks tab.
    fn tasks_note_text(&self) -> Option<String> {
        let id = self
            .catalog
            .iter()
            .find(|note| is_tasks_note(note))
            .map(|note| note.id.clone())?;
        self.task_source(&id).map(|(body, _, _)| body)
    }

    fn tasks_note_card(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let body = self.tasks_note_text().unwrap_or_default();
        let shown = if body.trim().is_empty() {
            "Nothing in it yet.".to_owned()
        } else {
            body
        };
        container(
            column![
                row![
                    text("The Tasks note").font(serif()).size(18),
                    space().width(Fill),
                    ghost_label("Close", Message::ToggleTasksNote),
                ]
                .align_y(iced::Alignment::Center),
                text("Every task added here is a line in it. Check one in the list, not here.")
                    .size(12)
                    .color(ink.muted),
                vscroll(text(shown).size(14).width(Fill))
                    .height(Fill)
                    .width(Fill),
            ]
            .spacing(10),
        )
        .width(460)
        .height(360)
        .padding(16)
        .style(card_style)
        .into()
    }

    fn reading_chat(&self) -> bool {
        self.screen == Screen::Chat && self.sheet == Sheet::None && (!self.compact() || self.detail)
    }

    fn mark_open_chat(&mut self) {
        let Some(id) = self.chat_id.clone() else {
            return;
        };
        let Some(count) = self.chat_count.get(&id).copied() else {
            return;
        };
        if self.seen_count.get(&id).copied() == Some(count) {
            return;
        }
        self.seen_count.insert(id, count);
        self.persist_seen();
    }

    fn persist_seen(&mut self) {
        let rows: Vec<(String, usize)> = self
            .seen_count
            .iter()
            .map(|(id, count)| (id.clone(), *count))
            .collect();
        if let Err(err) = profile::store_seen(&self.dir, &rows) {
            self.message = err;
        }
    }

    fn show_note(&mut self, id: &str) {
        self.older = None;
        self.version_wait = None;
        self.send_open = false;
        self.confirm_delete = false;
        if self.selected.as_deref() == Some(id) && self.editor_dirty() {
            self.screen = Screen::Notes;
            self.sheet = Sheet::None;
            return;
        }
        if self.selected.as_deref() != Some(id) {
            self.remember_unsaved();
            self.find_at = usize::MAX;
            self.swap_history(id);
        }
        let stash = self
            .unsaved
            .iter()
            .find(|(key, _, _)| key.as_deref() == Some(id))
            .map(|(_, title, body)| (title.clone(), body.clone()));
        let Some(book) = self.notebook.as_ref() else {
            return;
        };
        match book.show_held(id) {
            Ok(note) => {
                let draft = Draft::from_note(&note);
                self.selected = Some(id.to_owned());
                self.disk_title.clone_from(&draft.title);
                self.disk_body.clone_from(&draft.body);
                if let Some((title, body)) = stash {
                    self.title_text = title;
                    self.load_sealed(&body);
                    self.arm_pause();
                } else {
                    self.title_text.clone_from(&draft.title);
                    self.load_sealed(&draft.body);
                    self.disarm_pause();
                }
                self.conflicts = draft.conflicts;
                self.note_writer = note
                    .heads
                    .first()
                    .map(|head| head.writer.clone())
                    .unwrap_or_default();
                self.save_token = self.save_token.wrapping_add(1);
                self.screen = Screen::Notes;
                self.sheet = Sheet::None;
                self.detail = true;
                self.message.clear();
                self.reading = false;
                self.touch_group();
                self.reset_proof();
                self.maybe_proof();
            }
            Err(err) => self.message = err.to_string(),
        }
    }

    fn clear_editor(&mut self) {
        self.older = None;
        self.version_wait = None;
        self.send_open = false;
        self.confirm_delete = false;
        self.remember_unsaved();
        self.find_at = usize::MAX;
        self.swap_history("");
        self.save_token = self.save_token.wrapping_add(1);
        let stash = self
            .unsaved
            .iter()
            .find(|(key, _, _)| key.is_none())
            .map(|(_, title, body)| (title.clone(), body.clone()));
        self.selected = None;
        self.disk_title.clear();
        self.disk_body.clear();
        self.conflicts.clear();
        self.note_writer.clear();
        self.screen = Screen::Notes;
        self.sheet = Sheet::None;
        self.detail = true;
        self.reading = false;
        if let Some((title, body)) = stash {
            self.title_text = title;
            self.load_sealed(&body);
            self.arm_pause();
            self.reset_proof();
            self.maybe_proof();
            self.touch_group();
            return;
        }
        self.title_text.clear();
        self.tags.clear();
        self.tag_draft.clear();
        self.body.load("");
        self.disarm_pause();
        self.message.clear();
        self.reset_proof();
        self.maybe_proof();
        self.touch_group();
    }

    fn remember_unsaved(&mut self) {
        if !self.editor_dirty() {
            return;
        }
        let key = self.selected.clone();
        self.unsaved.retain(|(id, _, _)| id != &key);
        self.unsaved
            .push((key, self.title_text.clone(), self.stored_body()));
    }

    fn stored_body(&self) -> String {
        ambient_cli::join_note_tags(&self.body.text(), &self.tags)
    }

    fn load_sealed(&mut self, sealed: &str) {
        let (prose, tags) = ambient_cli::split_note_tags(sealed);
        self.tags = tags;
        self.tag_draft.clear();
        self.body.load(&prose);
    }

    fn editor_dirty(&self) -> bool {
        self.title_text != self.disk_title || self.stored_body() != self.disk_body
    }

    fn add_tag(&mut self) {
        let Some(tag) = clean_tag(&self.tag_draft) else {
            return;
        };
        if self.tags.iter().any(|have| have.eq_ignore_ascii_case(&tag)) {
            self.tag_draft.clear();
            return;
        }
        self.tags.push(tag);
        self.tag_draft.clear();
        self.arm_pause();
    }

    fn remove_tag(&mut self, tag: &str) {
        let before = self.tags.len();
        self.tags.retain(|have| !have.eq_ignore_ascii_case(tag));
        if self.tags.len() != before {
            self.arm_pause();
        }
    }

    fn tag_chips(&self) -> Element<'_, Message> {
        let mut chips = row![].spacing(6).align_y(iced::Alignment::Center);
        for tag in &self.tags {
            let label = tag.clone();
            chips = chips.push(
                button(text(format!("{label}  ×")).size(12))
                    .padding([4, 8])
                    .on_press(Message::RemoveTag(label))
                    .style(ghost_style),
            );
        }
        chips = chips.push(
            text_input("Add a tag", &self.tag_draft)
                .id(iced::widget::Id::new("note-tag"))
                .on_input(Message::TagDraft)
                .on_submit(Message::AddTag)
                .padding([4, 8])
                .size(13)
                .width(140)
                .style(field_style),
        );
        chips.into()
    }

    fn post(&self, job: lane::Job) {
        let _ = self.lane.send(job);
    }

    fn post_side(&self) {
        self.post(lane::Job::Side(lane::OpenSnap {
            selected: self.selected.clone(),
            title: self.title_text.clone(),
            stored: self.stored_body(),
            unsaved: self.unsaved.clone(),
        }));
    }

    fn sync_watch(&self) {
        let id = if self.screen == Screen::Chat {
            self.chat_id.clone()
        } else {
            None
        };
        self.post(lane::Job::Watch(id));
    }

    /// The list already knows the groups. This only points the open note at one.
    fn touch_group(&mut self) {
        self.track_book();
        self.use_cached_chapters();
        self.post_side();
    }

    fn use_cached_chapters(&mut self) {
        self.cached_chapters = self
            .group_views
            .iter()
            .find(|group| self.book_id.as_ref() == Some(&group.id))
            .map(|group| {
                group
                    .chapters
                    .iter()
                    .map(|piece| Chapter {
                        name: piece.name.clone(),
                        target: piece.target.clone(),
                        id: piece.id.clone(),
                        words: piece.words,
                        body: piece.body.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();
    }

    fn apply_side(&mut self, side: lane::SideDone) {
        self.groups = side
            .groups
            .iter()
            .map(|group| (group.id.clone(), group.targets.clone()))
            .collect();
        self.pieces = side.pieces;
        self.group_views = side.groups;
        self.cached_backlinks = side.backlinks;
        self.known_links = Some(Arc::new(side.known));
        markdown::set_known_links(self.known_links.clone());
        self.track_book();
        self.use_cached_chapters();
    }

    fn install_latest(&mut self) -> bool {
        let Some((generation, book)) = lane::take_latest(self.book_gen) else {
            return false;
        };
        self.book_gen = generation;
        self.devices = book.devices_held().unwrap_or_default();
        if let Ok(notes) = book.list_held() {
            self.catalog = notes;
            self.refresh_shown();
        }
        self.notebook = Some(book);
        true
    }

    fn apply_inbox(&mut self, rows: &[lane::ChatRow]) {
        self.chats = rows
            .iter()
            .map(|row| ambient_cli::ConversationSummary {
                id: row.id.clone(),
                label: row.label.clone(),
                other: row.other.clone(),
            })
            .collect();
        let known = self
            .chat_id
            .as_ref()
            .is_some_and(|id| self.chats.iter().any(|chat| &chat.id == id));
        if !known {
            self.chat_id = self.chats.first().map(|chat| chat.id.clone());
            if self.screen == Screen::Chat && self.chat_id.is_some() {
                self.post(lane::Job::Pull);
            }
        }
        self.chat_last.clear();
        let mut learned = false;
        for row in rows {
            self.chat_count.insert(row.id.clone(), row.count);
            if let std::collections::btree_map::Entry::Vacant(slot) =
                self.seen_count.entry(row.id.clone())
            {
                slot.insert(row.count);
                learned = true;
            }
            if row.count > 0 {
                self.chat_last
                    .insert(row.id.clone(), (row.preview.clone(), row.time.clone()));
            }
        }
        if learned {
            self.persist_seen();
        }
        if self.reading_chat() {
            self.mark_open_chat();
        }
    }

    fn apply_lane(&mut self, done: lane::Done) {
        match done {
            lane::Done::Side(side) => self.apply_side(side),
            lane::Done::Chat { id, messages } => {
                if self.chat_id.as_deref() == Some(id.as_str()) {
                    self.chat = bubbles(messages);
                    if self.reading_chat() {
                        self.mark_open_chat();
                    }
                }
            }
            lane::Done::Inbox(rows) => self.apply_inbox(&rows),
            lane::Done::Found {
                query,
                notes,
                chats,
            } => {
                if self.query == query {
                    self.search_lines = notes
                        .iter()
                        .map(|hit| (hit.note.id.clone(), hit.line.clone()))
                        .collect();
                    self.search_chats = chats;
                    self.search_hits = Some(notes.into_iter().map(|hit| hit.note).collect());
                    self.refresh_shown();
                }
            }
            lane::Done::Proof {
                text,
                warning,
                words,
            } => self.apply_proof(&text, warning, words),
            lane::Done::Version {
                id,
                title,
                body,
                current,
            } => self.apply_version(id, title, body, current),
            lane::Done::SendFailed { text, message } => {
                self.message = message;
                if self.draft_editor.text().trim().is_empty() {
                    self.draft_editor = text_editor::Content::with_text(&text);
                }
            }
            lane::Done::Notice(text) => self.message = text,
            lane::Done::Synced {
                detail,
                failed,
                transport,
                pending,
                writes,
                notes,
                side,
            } => {
                self.syncing = false;
                if failed {
                    self.pending = true;
                    if transport {
                        self.net_up = false;
                        if self.net_mode == ambient_cli::NetMode::ThisComputer
                            && detail.contains("unreachable")
                            && !self.relay_ours
                        {
                            self.relay_ready = false;
                        }
                        "No relay answered.".clone_into(&mut self.sync_detail);
                    } else {
                        self.sync_detail = detail;
                    }
                    return;
                }
                self.net_up = true;
                self.sync_detail = detail.clone();
                self.pending = pending || self.writes != writes;
                let installed = self.install_latest();
                if content_arrived(&detail) {
                    if !installed {
                        self.catalog = notes;
                        self.refresh_shown();
                    }
                    if self.screen == Screen::Tasks {
                        self.rebuild_tasks();
                    }
                    self.apply_side(side);
                    self.refresh_open_note();
                }
            }
        }
    }

    fn apply_proof(&mut self, text: &str, warning: Option<String>, words: BTreeSet<String>) {
        if self.body.text() != text {
            return;
        }
        self.proof_warning = warning.unwrap_or_default();
        self.misspellings = Arc::new(words);
        markdown::set_misspellings(Arc::clone(&self.misspellings));
    }

    fn apply_version(&mut self, id: String, title: String, body: String, current: bool) {
        if self.version_wait.as_deref() != Some(id.as_str()) {
            return;
        }
        self.version_wait = None;
        if current {
            self.older = None;
            self.show_note(&id);
            return;
        }
        self.older = Some(OlderNote { id, title, body });
        self.screen = Screen::Notes;
        self.sheet = Sheet::None;
        self.detail = true;
        self.reading = false;
    }

    fn target_is(target: &str, note: &NoteSummary) -> bool {
        target == note.id || (!note.title.is_empty() && target.eq_ignore_ascii_case(&note.title))
    }

    fn names_piece(&self, target: &str, title: &str) -> bool {
        if title.is_empty() {
            return false;
        }
        if target.eq_ignore_ascii_case(title) {
            return true;
        }
        self.catalog
            .iter()
            .any(|note| note.id == target && note.title.eq_ignore_ascii_case(title))
    }

    fn note_named(&self, title: &str) -> Option<(String, String)> {
        let title = title.trim();
        if title.is_empty() || markdown::is_note_id(title) {
            return None;
        }
        self.catalog.iter().find_map(|note| {
            note.title
                .eq_ignore_ascii_case(title)
                .then(|| (note.id.clone(), note.title.clone()))
        })
    }

    fn complete_link(&mut self, id: &str, version: Option<&str>) -> Task<Message> {
        let Some(title) = self
            .catalog
            .iter()
            .find(|note| note.id == id)
            .map(|note| note.title.clone())
        else {
            return Task::none();
        };
        let text = self.body.text();
        let caret = self.body.caret_byte();
        let Some((start, _)) = markdown::open_wiki(&text, caret) else {
            return Task::none();
        };
        let label = title.replace(['|', '[', ']'], "");
        let insert = match version {
            Some(version) => format!("[[{id}@{version}|{label}]]"),
            None => format!("[[{id}|{label}]]"),
        };
        let mut next = String::new();
        next.push_str(text.get(..start).unwrap_or(""));
        next.push_str(&insert);
        next.push_str(text.get(caret..).unwrap_or(""));
        let at = start.saturating_add(insert.len());
        self.revise(Burst::Break, |app| {
            app.body.load(&next);
            app.body.place_byte(at);
            true
        });
        self.note_follow(false)
    }

    fn maybe_proof(&mut self) {
        if self.reading || self.older.is_some() {
            return;
        }
        let text = self.body.text();
        if text == self.proof_body {
            return;
        }
        self.proof_body.clone_from(&text);
        self.post(lane::Job::Proof(text));
    }

    fn reset_proof(&mut self) {
        self.proof_body.clear();
        self.proof_warning.clear();
        self.misspellings = Arc::new(BTreeSet::new());
        markdown::set_misspellings(Arc::clone(&self.misspellings));
    }

    fn close_version(&mut self) {
        let id = self.older.as_ref().map(|note| note.id.clone());
        self.older = None;
        if let Some(id) = id {
            self.show_note(&id);
        }
    }

    fn older_page(&self, older: &OlderNote) -> Element<'_, Message> {
        let ink = self.ink();
        let title = if older.title.is_empty() {
            "Untitled".to_owned()
        } else {
            older.title.clone()
        };
        let page = column![
            text("This is an older version.").size(13).color(ink.muted),
            ghost_label("Current note", Message::CloseVersion),
            text(title)
                .font(serif())
                .size(if self.compact() { 32.0 } else { 36.0 }),
            vscroll(text(older.body.clone()).size(16)).height(Fill),
        ]
        .spacing(12)
        .width(Fill)
        .height(Fill);
        self.writing_frame(page)
    }

    fn open_note_link(&mut self, name: &str, version: Option<&str>) {
        let id = if markdown::is_note_id(name) {
            self.catalog
                .iter()
                .find(|note| note.id == name)
                .map(|note| note.id.clone())
        } else {
            self.catalog
                .iter()
                .find(|note| !note.title.is_empty() && note.title.eq_ignore_ascii_case(name))
                .map(|note| note.id.clone())
        };
        let Some(id) = id else {
            if markdown::is_note_id(name) {
                self.say("That note is gone.");
            } else {
                self.say("No note with that name.");
            }
            return;
        };
        let Some(version) = version else {
            self.show_note(&id);
            return;
        };
        if self
            .catalog
            .iter()
            .any(|note| note.id == id && note.version == version)
        {
            self.show_note(&id);
            return;
        }
        self.post(lane::Job::Version {
            id: id.clone(),
            version: version.to_owned(),
        });
        self.version_wait = Some(id);
    }

    fn pick_cite(&mut self, id: &str, task: bool) {
        let Some(note) = self.catalog.iter().find(|note| note.id == id) else {
            return;
        };
        let chosen = (note.id.clone(), note.title.clone());
        if task {
            self.task_cite = Some(chosen);
            self.task_cite_open = false;
        } else {
            self.chat_cite = Some(chosen);
            self.cite_open = false;
        }
    }

    fn cite_menu(&self, task: bool) -> Element<'_, Message> {
        let ink = self.ink();
        let mut notes: Vec<_> = self
            .catalog
            .iter()
            .filter(|note| !note.title.is_empty())
            .collect();
        notes.sort_by(|left, right| left.title.cmp(&right.title));
        notes.truncate(8);
        if notes.is_empty() {
            return text("No notes to cite.").size(12).color(ink.muted).into();
        }
        let mut list = column![].spacing(2).width(Fill);
        for note in notes {
            let id = note.id.clone();
            list = list.push(
                button(text(note.title.clone()).size(14).width(Fill))
                    .width(Fill)
                    .padding([4, 8])
                    .on_press(if task {
                        Message::PickTaskCite(id)
                    } else {
                        Message::PickChatCite(id)
                    })
                    .style(list_row(false)),
            );
        }
        container(list)
            .padding(6)
            .width(Fill)
            .style(card_style)
            .into()
    }

    fn link_menu(&self, query: &str) -> Element<'_, Message> {
        let ink = self.ink();
        let needle = query.trim().to_lowercase();
        let mut matches: Vec<&NoteSummary> = self
            .catalog
            .iter()
            .filter(|note| {
                !note.title.is_empty()
                    && (needle.is_empty() || note.title.to_lowercase().contains(&needle))
                    && self.selected.as_deref() != Some(note.id.as_str())
            })
            .take(6)
            .collect();
        matches.sort_by(|left, right| {
            let left_rank = left.title.to_lowercase().starts_with(&needle);
            let right_rank = right.title.to_lowercase().starts_with(&needle);
            right_rank
                .cmp(&left_rank)
                .then_with(|| left.title.cmp(&right.title))
        });
        if matches.is_empty() {
            return text("No note with that name yet. Close the link and it stays unresolved.")
                .size(12)
                .color(ink.muted)
                .into();
        }
        let mut list = column![].spacing(2).width(Fill);
        for note in matches {
            let id = note.id.clone();
            let version = note.version.clone();
            let mut line = row![
                button(text(note.title.clone()).size(14).width(Fill))
                    .width(Fill)
                    .padding([4, 8])
                    .on_press(Message::CompleteLink(id.clone()))
                    .style(list_row(false))
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center);
            if !note.conflict && markdown::is_note_id(&version) {
                line = line.push(
                    button(text("version").size(12))
                        .padding([4, 8])
                        .on_press(Message::CompleteVersion { id, version })
                        .style(ghost_style),
                );
            }
            list = list.push(line);
        }
        container(list)
            .padding(6)
            .width(280)
            .style(card_style)
            .into()
    }

    fn ink(&self) -> Ink {
        Ink::from_dark(self.is_dark())
    }

    fn is_dark(&self) -> bool {
        match self.appearance {
            profile::Appearance::Light => false,
            profile::Appearance::Dark => true,
            profile::Appearance::System => self.system_dark,
        }
    }

    fn arm_pause(&mut self) {
        self.edited_at = Some(Instant::now());
        self.pause_token = self.pause_token.wrapping_add(1);
        self.pause_armed = true;
    }

    fn disarm_pause(&mut self) {
        self.edited_at = None;
        self.pause_armed = false;
        self.pause_token = self.pause_token.wrapping_add(1);
    }

    fn arm_search(&mut self) {
        self.search_token = self.search_token.wrapping_add(1);
        self.search_armed = true;
    }

    fn disarm_search(&mut self) {
        self.search_armed = false;
        self.search_token = self.search_token.wrapping_add(1);
    }

    fn start_save(&mut self, force: bool) -> Task<Message> {
        if self.notebook.is_none() || !self.editor_dirty() {
            return Task::none();
        }
        if self.saving {
            self.save_again = true;
            return Task::none();
        }
        if !force
            && self
                .edited_at
                .is_none_or(|at| at.elapsed() < Duration::from_millis(700))
        {
            return Task::none();
        }
        self.maybe_proof();
        self.post_side();
        self.saving = true;
        self.save_again = false;
        let token = self.save_token;
        save_task(
            self.dir.clone(),
            self.selected.clone(),
            self.title_text.clone(),
            self.stored_body(),
            token,
        )
    }

    fn begin_sync(&mut self) -> Task<Message> {
        if self.notebook.is_none()
            || self.syncing
            || (self.net_mode == ambient_cli::NetMode::ThisComputer && !self.relay_ready)
        {
            return Task::none();
        }
        self.syncing = true;
        self.post(lane::Job::Sync {
            writes: self.writes,
        });
        Task::none()
    }

    fn refresh_open_note(&mut self) {
        if self.editor_dirty() {
            return;
        }
        let Some(id) = self.selected.clone() else {
            return;
        };
        let Some(book) = self.notebook.as_ref() else {
            return;
        };
        let Ok(note) = book.show_held(&id) else {
            return;
        };
        let draft = Draft::from_note(&note);
        let writer = note
            .heads
            .first()
            .map(|head| head.writer.clone())
            .unwrap_or_default();
        if draft.title == self.disk_title
            && draft.body == self.disk_body
            && draft.conflicts == self.conflicts
        {
            self.note_writer = writer;
            return;
        }
        self.note_writer = writer;
        self.conflicts = draft.conflicts;
        self.disk_title.clone_from(&draft.title);
        self.disk_body.clone_from(&draft.body);
        self.title_text.clone_from(&draft.title);
        self.load_sealed(&draft.body);
        self.disarm_pause();
        self.touch_group();
    }

    fn compact(&self) -> bool {
        self.size.width < 720.0
    }

    fn side_width(&self) -> f32 {
        if self.size.width < 1040.0 {
            210.0
        } else {
            280.0
        }
    }

    /// Width of the middle pane, inside its padding. The list and the group panel take their share first.
    fn pane_inner(&self) -> f32 {
        if self.compact() {
            return (self.size.width - self.gutter_x() * 2.0).max(160.0);
        }
        let mut chrome = self.gutter_x() * 2.0 + 28.0;
        if self.show_list {
            chrome += self.side_width() + 1.0;
        }
        if self.context_room() {
            chrome += 28.0;
            if self.show_context {
                chrome += 241.0;
            }
        }
        (self.size.width - chrome).max(160.0)
    }

    /// The writing column. It grows with the middle pane, and stops at a readable measure.
    fn writing_column(&self) -> f32 {
        self.pane_inner().min(720.0)
    }

    /// Where a paragraph wraps. A little inside the column, so the scrollbar is not the margin.
    fn writing_width(&self) -> f32 {
        (self.writing_column() - 8.0).max(120.0)
    }

    fn gutter(&self) -> f32 {
        if self.compact() { 12.0 } else { 22.0 }
    }

    fn gutter_x(&self) -> f32 {
        if self.compact() {
            12.0
        } else if self.size.width < 1000.0 {
            20.0
        } else {
            32.0
        }
    }

    /// Writing with nothing else in view. Only in the wide layout, and only for a note being
    /// written, not a book being read or an older version.
    fn focused(&self) -> bool {
        self.focus
            && self.editing_note()
            && !self.compact()
            && !self.reading
            && self.older.is_none()
    }

    fn editing_note(&self) -> bool {
        self.sheet == Sheet::None
            && self.screen == Screen::Notes
            && (!self.compact() || self.detail)
    }

    fn chat_inner(&self) -> f32 {
        let chrome = if self.compact() {
            self.gutter_x() * 2.0
        } else {
            let side = if self.show_list {
                self.side_width() + 1.0
            } else {
                0.0
            };
            side + self.gutter_x() * 2.0
        };
        (self.size.width - chrome).max(120.0)
    }

    fn bubble_width(&self) -> f32 {
        let inner = self.chat_inner();
        (inner - 36.0).clamp(96.0, 460.0)
    }

    /// The formatting bar. In focus it slides down from the top edge of the page, or away.
    fn format_slot(&self) -> Element<'static, Message> {
        if !self.focused() {
            return Self::format_bar();
        }
        let shown = self.slide_bar.shown();
        if shown <= 0.0 {
            return space().height(0).into();
        }
        if shown >= 1.0 {
            return Self::format_bar();
        }
        // The bar is kept against the bottom of a box that grows, so it comes down into view.
        // It is clipped, and it is only in the tree while it is partly or fully shown.
        container(Self::format_bar())
            .width(Fill)
            .height(FORMAT_BAR * shown)
            .align_y(iced::alignment::Vertical::Bottom)
            .clip(true)
            .into()
    }

    fn format_bar() -> Element<'static, Message> {
        hscroll(
            row![
                format_caption("Word"),
                format_button("B", "Bold", FormatKind::Bold),
                format_button("I", "Italic", FormatKind::Italic),
                format_button("S", "Strike", FormatKind::Strike),
                format_button("==", "Highlight", FormatKind::Highlight),
                format_button("</>", "Code", FormatKind::Code),
                space().width(14),
                format_caption("Line"),
                format_button("H", "Heading", FormatKind::Heading),
                format_button("List", "List", FormatKind::List),
                format_button("Task", "Task", FormatKind::Task),
                format_button("Quote", "Quote", FormatKind::Quote),
                space().width(14),
                format_caption("Insert"),
                format_button("Link", "Link", FormatKind::Link),
                format_button("[[]]", "Link a note", FormatKind::Wiki),
                format_button("Image", "Image", FormatKind::Image),
                format_button("Table", "Table", FormatKind::Table),
                format_button("—", "Rule", FormatKind::Rule),
                format_button("Callout", "Callout", FormatKind::Callout),
                format_button("```", "Code block", FormatKind::CodeBlock),
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center)
            .padding([2, 0]),
        )
        .width(Fill)
        .into()
    }

    fn bottom_bar(&self) -> Element<'_, Message> {
        let mut bar = column![].width(Fill);
        if self.editing_note() {
            bar = bar.push(Self::format_bar()).push(hairline());
        }
        let tasks_name = self.tasks_tab();
        let chat_name = self.chat_tab();
        let mut nav = row![
            segment(
                "Notes",
                self.screen == Screen::Notes && self.sheet == Sheet::None,
                Message::ShowNotes,
            ),
            segment(
                &tasks_name,
                self.screen == Screen::Tasks && self.sheet == Sheet::None,
                Message::ShowTasks,
            ),
            segment(
                &chat_name,
                self.screen == Screen::Chat && self.sheet == Sheet::None,
                Message::ShowChat,
            ),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center)
        .width(Fill);
        if self.notebook.is_some() && !self.editor_dirty() {
            nav = nav.push(text("✓").size(18).color(Color::from_rgb8(46, 140, 96)));
        }
        nav = nav.push(icon_button(
            gear_handle(self.is_dark()),
            Message::ShowSettings,
        ));
        nav = nav.push(icon_button(
            mode_handle(self.is_dark()),
            Message::ToggleTheme,
        ));
        nav = nav.push(anchored(
            icon_button(globe_handle(self.globe_color()), Message::ToggleSyncPop),
            self.sync_open.then(|| self.sync_card()),
        ));
        bar = bar.push(container(nav).padding([6, 8]).width(Fill));
        bar.into()
    }

    fn compact_title(&self) -> Element<'_, Message> {
        let ink = self.ink();
        row![
            text("Aerodash").font(serif()).size(20),
            text(self.message.as_str())
                .size(12)
                .color(ink.muted)
                .width(Fill),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .padding([8, 12])
        .into()
    }

    fn compact_body(&self) -> Element<'_, Message> {
        let body = match self.sheet {
            Sheet::Invite => self.invite_sheet(),
            Sheet::Settings => self.settings_sheet(),
            Sheet::None if self.screen == Screen::Tasks => self.tasks_pane(),
            Sheet::None if self.screen == Screen::Chat && !self.detail => self.chat_browser(),
            Sheet::None if self.screen == Screen::Chat => self.chat_pane(),
            Sheet::None if !self.detail => self.note_browser(),
            Sheet::None => self.notes_pane(),
        };
        container(body)
            .padding([self.gutter(), self.gutter_x()])
            .width(Fill)
            .height(Fill)
            .into()
    }

    fn note_browser(&self) -> Element<'_, Message> {
        self.note_index()
    }

    fn chat_browser(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let mut side = column![action_fill("Invite", Message::ShowInvite)]
            .spacing(8)
            .width(Fill);
        if self.chats.is_empty() {
            side = side.push(text("No conversations yet.").size(13).color(ink.muted));
        }
        for chat in &self.chats {
            let selected = self.chat_id.as_deref() == Some(chat.id.as_str());
            let (preview, time) = self.chat_last.get(&chat.id).cloned().unwrap_or_default();
            side = side.push(chat_row(
                &self.chat_name(chat),
                &preview,
                &time,
                &chat.id,
                selected,
                self.chat_is_new(&chat.id),
            ));
        }
        side.into()
    }

    fn note_media(&self) -> Element<'_, Message> {
        let found = files::note_media(&self.dir, &self.body.text());
        if found.is_empty() {
            return space().height(0).into();
        }
        let mut row = row![].spacing(8);
        for item in found {
            match item {
                files::NoteMedia::Picture(path) => {
                    row = row.push(
                        iced_image::Image::new(iced_image::Handle::from_path(path))
                            .content_fit(iced::ContentFit::Contain)
                            .height(140)
                            .width(180),
                    );
                }
                files::NoteMedia::Video(path, name) => {
                    row = row.push(ghost_label_owned(
                        format!("Video · {name}"),
                        Message::OpenPath(path),
                    ));
                }
                files::NoteMedia::File(path, name) => {
                    row = row.push(ghost_label_owned(name, Message::OpenPath(path)));
                }
            }
        }
        hscroll(row).height(156).into()
    }

    fn chat_bubble(&self, line: &ChatBubble, width: f32) -> Element<'static, Message> {
        let ink = self.ink();
        let copy_text = line.text.clone();
        let words = line.text.clone();
        let when = line.time.clone();
        let mine = line.mine;
        let conv = self.chat_id.clone().unwrap_or_default();
        let parts = files::pieces(&self.dir, &conv, &words);
        let mut body = column![].spacing(6);
        if parts.is_empty() {
            body = body.push(chat_words(words));
        }
        for part in parts {
            match part {
                files::Piece::Text(words) => {
                    body = body.push(chat_words(words));
                }
                files::Piece::Picture { name, path } => {
                    if let Some(path) = path {
                        body = body.push(
                            iced_image::Image::new(iced_image::Handle::from_path(path))
                                .content_fit(iced::ContentFit::Contain)
                                .width((width - 28.0).max(64.0))
                                .height(180),
                        );
                    } else {
                        body = body.push(
                            text(format!("{name} · not on this computer"))
                                .size(13)
                                .color(if mine { ink.on_accent } else { ink.text }),
                        );
                    }
                }
                files::Piece::Video { name, path } => {
                    body = body.push(file_chip(&name, path, true, mine));
                }
                files::Piece::File { name, path } => {
                    body = body.push(file_chip(&name, path, false, mine));
                }
            }
        }
        let time_color = if mine {
            Color {
                a: 0.72,
                ..ink.on_accent
            }
        } else {
            ink.muted
        };
        let mut meta = row![].spacing(8).align_y(iced::Alignment::Center);
        if !when.is_empty() {
            meta = meta.push(text(when).size(11).color(time_color));
        }
        meta = meta.push(
            button(text("Copy").size(11))
                .padding([0, 2])
                .on_press(Message::CopyChat(copy_text))
                .style(move |theme, status| copy_style(theme, status, mine)),
        );
        if let Some(cite) = line.cite.clone() {
            let label = self
                .catalog
                .iter()
                .find(|note| note.id == cite)
                .map(|note| note.title.clone())
                .filter(|title| !title.is_empty())
                .unwrap_or_else(|| "Note".to_owned());
            meta = meta.push(
                button(text(label).size(11))
                    .padding([0, 2])
                    .on_press(Message::OpenCited(cite))
                    .style(move |theme, status| copy_style(theme, status, mine)),
            );
        }
        body = body.push(meta);
        let bubble = container(body)
            .padding([8, 12])
            .max_width(width)
            .clip(true)
            .style(if mine { mine_style } else { theirs_style });
        container(bubble)
            .width(Fill)
            .align_x(if mine {
                iced::alignment::Horizontal::Right
            } else {
                iced::alignment::Horizontal::Left
            })
            .into()
    }

    fn stage_file(&mut self, path: PathBuf) {
        if !path.is_file() {
            self.say("Choose a file, not a folder.");
            return;
        }
        if self.screen == Screen::Chat {
            if self.chat_id.is_none() {
                self.say("Invite someone before adding a file.");
                return;
            }
            self.pending_files.push(path);
            self.detail = true;
            self.sheet = Sheet::None;
            return;
        }
        match files::store_for_note(&self.dir, &path) {
            Ok(markdown) => {
                let insert = if self.body.text().is_empty() || self.body.text().ends_with('\n') {
                    markdown
                } else {
                    format!("\n{markdown}")
                };
                self.revise(Burst::Break, |app| {
                    let before = app.body.text();
                    app.body
                        .perform(&text_editor::Action::Edit(text_editor::Edit::Paste(
                            Arc::new(insert),
                        )));
                    app.body.text() != before
                });
                self.screen = Screen::Notes;
                self.sheet = Sheet::None;
                self.detail = true;
                self.say("Added to the note on this computer.");
            }
            Err(err) => self.message = err,
        }
    }

    fn pick_file(&mut self) -> Task<Message> {
        if self.screen == Screen::Chat && self.chat_id.is_none() {
            self.say("Invite someone before adding a file.");
            return Task::none();
        }
        Task::perform(
            async {
                let (tx, rx) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    let _ = tx.send(files::choose());
                });
                rx.recv()
                    .unwrap_or_else(|_| Err("No file chosen.".to_owned()))
            },
            Message::FilePicked,
        )
    }

    fn insert_mark(&mut self, kind: FormatKind) {
        let change = markdown::format_change(&self.body.text(), self.body.cursor(), kind);
        if kind == FormatKind::Wiki
            && let Some(inner) = change
                .insert
                .strip_prefix("[[")
                .and_then(|rest| rest.strip_suffix("]]"))
            && let Some((id, title)) = self.note_named(inner)
        {
            let change =
                markdown::note_link_change(&self.body.text(), &self.body.cursor(), &id, &title);
            self.apply_change(change);
            return;
        }
        self.apply_change(change);
    }

    fn apply_change(&mut self, change: markdown::Change) {
        self.revise(Burst::Break, |app| {
            let before = app.body.text();
            if change.anchor == change.end {
                app.body.place(text_editor::Cursor {
                    position: change.anchor,
                    selection: None,
                });
            } else {
                app.body.place(text_editor::Cursor {
                    position: change.end,
                    selection: Some(change.anchor),
                });
            }
            app.body
                .perform(&text_editor::Action::Edit(text_editor::Edit::Paste(
                    Arc::new(change.insert),
                )));
            app.body.place(change.after);
            app.body.text() != before
        });
    }

    fn revise(&mut self, burst: Burst, change: impl FnOnce(&mut Self) -> bool) {
        let now = Instant::now();
        let gate = if self.history.coalesced(burst, now) {
            self.history.begin("", "", self.body.cursor(), burst, now)
        } else {
            let cursor = self.body.cursor();
            let title = self.title_text.clone();
            let body = self.body.text();
            self.history.begin(&title, &body, cursor, burst, now)
        };
        if change(self) {
            self.arm_pause();
        } else {
            self.history.abort(gate);
        }
    }

    fn snap(&self) -> EditSnap {
        EditSnap {
            title: self.title_text.clone(),
            body: self.body.text(),
            cursor: self.body.cursor(),
        }
    }

    fn restore_snap(&mut self, snap: EditSnap) {
        self.title_text = snap.title;
        self.body.load(&snap.body);
        self.body.place(snap.cursor);
        self.drag_from = None;
        self.last_press = None;
        if self.editor_dirty() {
            self.arm_pause();
        } else {
            self.disarm_pause();
        }
    }

    fn undo_note(&mut self) {
        let Some(previous) = self.history.undo(self.snap()) else {
            return;
        };
        self.restore_snap(previous);
    }

    fn redo_note(&mut self) {
        let Some(next) = self.history.redo(self.snap()) else {
            return;
        };
        self.restore_snap(next);
    }

    fn clear_history(&mut self) {
        self.history.clear();
    }

    /// Keeps the steps with the note that just left. Memory only, and not the log.
    fn swap_history(&mut self, next: &str) {
        if self.history_key == next {
            return;
        }
        let leaving = std::mem::take(&mut self.history);
        let key = self.history_key.clone();
        if leaving.undo.is_empty() && leaving.redo.is_empty() {
            self.histories.remove(&key);
            self.history_order.retain(|id| id != &key);
        } else {
            self.histories.insert(key.clone(), leaving);
            self.history_order.retain(|id| id != &key);
            self.history_order.push(key);
        }
        while self.history_order.len() > 8 {
            let oldest = self.history_order.remove(0);
            self.histories.remove(&oldest);
        }
        self.history = self.histories.remove(next).unwrap_or_default();
        self.history_order.retain(|id| id != next);
        next.clone_into(&mut self.history_key);
    }

    fn try_open_at(&mut self, byte: usize) -> bool {
        let Some(link) = markdown::opened_at(&self.body.text(), byte) else {
            return false;
        };
        match link {
            markdown::OpenLink::Url(url) => {
                if let Err(err) = files::open_url(&url) {
                    self.message = err;
                }
            }
            markdown::OpenLink::Note(target) => {
                self.open_note_link(&target.name, target.version.as_deref());
            }
        }
        true
    }

    fn find_bar(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let hits = markdown::find_bytes(&self.body.text(), &self.find_query);
        let place = if self.find_query.trim().is_empty() {
            String::new()
        } else if hits.is_empty() {
            "None".to_owned()
        } else if self.find_at == usize::MAX || self.find_at >= hits.len() {
            format!("{} found", hits.len())
        } else {
            format!("{} of {}", self.find_at.saturating_add(1), hits.len())
        };
        let mut bar = row![
            text_input("Find in this note", &self.find_query)
                .on_input(Message::FindQuery)
                .on_submit(Message::FindNext)
                .padding(8)
                .width(Fill)
                .style(field_style),
            ghost_label("Previous", Message::FindPrev),
            ghost_label("Next", Message::FindNext),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);
        if !place.is_empty() {
            bar = bar.push(text(place).size(13).color(ink.muted));
        }
        bar.into()
    }

    fn seek_find(&mut self, forward: bool) {
        let query = self.find_query.trim().to_owned();
        if query.is_empty() {
            self.say("Type what to find.");
            return;
        }
        let hits = markdown::find_bytes(&self.body.text(), &query);
        if hits.is_empty() {
            self.find_at = usize::MAX;
            self.say("Not in this note.");
            return;
        }
        let next = if self.find_at == usize::MAX || self.find_at >= hits.len() {
            if forward {
                0
            } else {
                hits.len().saturating_sub(1)
            }
        } else if forward {
            self.find_at.saturating_add(1) % hits.len()
        } else if self.find_at == 0 {
            hits.len().saturating_sub(1)
        } else {
            self.find_at.saturating_sub(1)
        };
        self.find_at = next;
        let Some(&start) = hits.get(next) else {
            self.find_at = usize::MAX;
            return;
        };
        let end = start.saturating_add(query.len());
        self.body.select_between(start, end);
        self.body.end_select();
        self.message.clear();
    }

    fn note_reveal(&mut self) -> Task<Message> {
        let y = (self.body.reveal(self.writing_width()) - 72.0).max(0.0);
        Task::batch([
            iced::widget::operation::focus(iced::widget::Id::new("note-body")),
            iced::widget::operation::scroll_to(
                iced::widget::Id::new("note-scroll"),
                iced::widget::operation::AbsoluteOffset { x: 0.0, y },
            ),
        ])
    }

    fn arm_press(&mut self, byte: usize) {
        self.pointer_down = true;
        self.kept_click = false;
        let text = self.body.text();
        let byte = markdown::snap_caret(&text, byte);
        if self.shift_down {
            let anchor = self
                .body
                .anchor_byte()
                .unwrap_or_else(|| self.body.caret_byte());
            self.drag_from = Some(anchor);
            self.kept_click = true;
            self.body.select_between(anchor, byte);
            self.last_press = None;
            return;
        }
        let repeat = self.last_press.is_some_and(|(when, previous, _)| {
            when.elapsed() < Duration::from_millis(400)
                && markdown::same_line(&text, previous, byte)
        });
        let count = if repeat {
            self.last_press
                .map_or(1, |(_, _, count)| count.saturating_add(1))
        } else {
            1
        };
        if count >= 3 {
            let (from, to) = markdown::line_bounds(&text, byte);
            self.body.select_between(from, to);
            self.drag_from = Some(from);
            self.kept_click = true;
            self.last_press = None;
            return;
        }
        if count == 2
            && self
                .last_press
                .is_some_and(|(_, previous, _)| markdown::same_word(&text, previous, byte))
        {
            let (from, to) = markdown::word_bounds(&text, byte);
            self.body.select_between(from, to);
            self.drag_from = Some(from);
            self.kept_click = true;
            self.last_press = Some((Instant::now(), byte, 2));
            return;
        }
        self.last_press = Some((Instant::now(), byte, 1));
        self.drag_from = Some(byte);
        if markdown::line_index(&text, byte) == self.body.open_line() {
            self.body.place_byte(byte);
        } else {
            // Opening that line now would reflow the page under the pointer.
            self.body.place_byte_soft(byte);
        }
    }

    fn note_column(&self) -> Element<'_, Message> {
        if let Some(older) = &self.older {
            return self.older_page(older);
        }
        let chapters = &self.cached_chapters;
        let mut page = column![].spacing(14).width(Fill).height(Fill);
        if self.reading && !chapters.is_empty() {
            page = page.push(
                text(self.book_title())
                    .font(serif())
                    .size(if self.compact() { 32.0 } else { 36.0 }),
            );
            page = page.push(self.read_pages(chapters));
        } else {
            let focused = self.focused();
            if focused {
                // The title is small, at the top right of the note, so the writing starts a
                // little below it. This keeps a line from running under it.
                page = page.push(space().height(FOCUS_TOP));
            } else {
                page = page.push(
                    text_input("Title", &self.title_text)
                        .id(iced::widget::Id::new("note-title"))
                        .on_input(Message::TitleChanged)
                        .on_submit(Message::TitleSubmit)
                        .font(serif())
                        .size(if self.compact() { 32.0 } else { 36.0 })
                        .padding([2.0, 0.0])
                        .style(title_style),
                );
                page = page.push(self.tag_chips());
            }
            if self.find_open {
                page = page.push(self.find_bar());
            }
            if !self.compact() {
                page = page.push(self.format_slot());
            }
            if let Some((_, query)) = markdown::open_wiki(&self.body.text(), self.body.caret_byte())
            {
                page = page.push(self.link_menu(&query));
            }
            if !self.proof_warning.is_empty() && !focused {
                page = page.push(
                    text(self.proof_warning.clone())
                        .size(12)
                        .color(self.ink().muted),
                );
            }
            page = page.push(
                vscroll(self.editor_page())
                    .id(iced::widget::Id::new("note-scroll"))
                    .height(Fill),
            );
            page = page.push(self.note_media());
        }
        if self.compact()
            && self.show_context
            && let Some(rail) = self.chapter_rail(chapters)
        {
            return column![rail, self.writing_frame(page)]
                .spacing(12)
                .width(Fill)
                .height(Fill)
                .into();
        }
        self.writing_frame(page)
    }

    fn writing_frame<'a>(&self, page: iced::widget::Column<'a, Message>) -> Element<'a, Message> {
        container(page.max_width(self.writing_column()).clip(true))
            .width(Fill)
            .height(Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .into()
    }

    fn editor_page(&self) -> Element<'_, Message> {
        markdown::set_known_links(self.known_links.clone());
        markdown::set_misspellings(Arc::clone(&self.misspellings));
        let dragging = self.pointer_down && self.drag_from.is_some();
        let page = markdown::view(&self.body, self.is_dark(), dragging, self.writing_width());
        let Some((y, height)) = (!dragging)
            .then(|| self.body.selection_origin(self.writing_width()))
            .flatten()
        else {
            return page;
        };
        let lift = if y >= 38.0 {
            y - 34.0
        } else {
            y + height + 4.0
        };
        stack![
            page,
            container(Self::format_pill()).padding(iced::Padding {
                top: lift,
                right: 8.0,
                bottom: 0.0,
                left: 0.0,
            })
        ]
        .into()
    }

    fn format_pill() -> Element<'static, Message> {
        container(
            row![
                pill_button("B", "Bold", Message::Format(FormatKind::Bold)),
                pill_button("I", "Italic", Message::Format(FormatKind::Italic)),
                pill_button("S", "Strike", Message::Format(FormatKind::Strike)),
                pill_button("==", "Highlight", Message::Format(FormatKind::Highlight)),
                pill_button("Link", "Link", Message::Format(FormatKind::Link)),
                pill_button("Copy", "Copy", Message::CopyNote),
                pill_button("Cut", "Cut", Message::CutNote),
                pill_button("Paste", "Paste", Message::PasteNote),
            ]
            .spacing(2)
            .align_y(iced::Alignment::Center),
        )
        .padding([4, 6])
        .style(pill_style)
        .into()
    }

    fn chapter_rail(&self, chapters: &[Chapter]) -> Option<Element<'_, Message>> {
        if chapters.len() < 2 {
            return None;
        }
        let ink = self.ink();
        let kind = self
            .group_views
            .iter()
            .find(|group| self.book_id.as_ref() == Some(&group.id))
            .map(|group| group.kind.clone())
            .unwrap_or_default();
        let mut head = column![self.group_heading()].spacing(4).width(Fill);
        if !kind.is_empty() {
            head = head.push(text(kind).size(12).color(ink.muted));
        }
        if !self.on_group_note() && !self.title_text.trim().is_empty() {
            head = head.push(text(self.title_text.clone()).size(14));
        }
        let total: usize = chapters.iter().map(|chapter| chapter.words).sum();
        head = head.push(
            row![
                text(words_label(total)).size(12).color(ink.muted),
                space().width(Fill),
                ghost_label(
                    if self.reading { "Write" } else { "Read" },
                    Message::ToggleRead,
                ),
            ]
            .align_y(iced::Alignment::Center),
        );
        let mut list = column![].spacing(4).width(Fill);
        for chapter in chapters {
            let here = self.selected.as_deref() == chapter.id.as_deref()
                || self.selected.as_deref() == Some(chapter.target.as_str())
                || (!self.on_group_note()
                    && !self.title_text.is_empty()
                    && self.title_text.eq_ignore_ascii_case(&chapter.name));
            let count = if chapter.id.is_some() {
                chapter.words.to_string()
            } else if markdown::is_note_id(&chapter.target) {
                "missing".to_owned()
            } else {
                "new".to_owned()
            };
            list = list.push(
                button(
                    text(format!("{} · {count}", chapter.name))
                        .size(14)
                        .width(Fill),
                )
                .width(Fill)
                .padding([6.0, 8.0])
                .on_press(Message::OpenChapter(chapter.target.clone()))
                .style(list_row(here)),
            );
        }
        let scroll = vscroll(list).width(Fill);
        let body = if self.compact() {
            column![head, scroll.height(180)].spacing(8).width(Fill)
        } else {
            column![head, scroll.height(Fill)]
                .spacing(8)
                .width(Fill)
                .height(Fill)
        };
        Some(body.into())
    }

    fn on_group_note(&self) -> bool {
        match (&self.book_id, &self.selected) {
            (Some(book), Some(id)) => book == id,
            (Some(book), None) => book.is_empty(),
            _ => false,
        }
    }

    fn group_heading(&self) -> Element<'_, Message> {
        let title = self.book_title();
        if self.on_group_note() {
            return text(title).font(serif()).size(18).into();
        }
        let Some(id) = self.book_id.clone().filter(|id| !id.is_empty()) else {
            return text(title).font(serif()).size(18).into();
        };
        button(text(title).font(serif()).size(18))
            .padding(0)
            .on_press(Message::Select(id))
            .style(ghost_style)
            .into()
    }

    fn context_room(&self) -> bool {
        self.sheet == Sheet::None && self.screen == Screen::Notes
    }

    fn context_pane(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let chapters = &self.cached_chapters;
        let mut block = column![].spacing(10).padding(16).width(Fill).height(Fill);
        block = block.push(
            row![
                text(if self.note_panel {
                    "This note"
                } else {
                    "Links"
                })
                .size(12)
                .color(ink.muted),
                space().width(Fill),
                mini_button(
                    mini_handle(Mini::Gear, self.is_dark()),
                    if self.note_panel {
                        "Links and backlinks"
                    } else {
                        "Note settings"
                    },
                    Message::ToggleNoteSettings,
                ),
            ]
            .align_y(iced::Alignment::Center),
        );
        if self.note_panel {
            return container(self.note_settings(block))
                .width(240)
                .height(Fill)
                .style(surface_pane)
                .into();
        }
        if let Some(rail) = self.chapter_rail(chapters) {
            block = block.push(rail);
        } else if self.cached_backlinks.is_empty() {
            block = block.push(
                text("Link two notes and they gather here, under this note's name. A first line Kind: issues names the pieces. Tags are added under the title.")
                    .size(13)
                    .color(ink.muted),
            );
        }
        if !self.cached_backlinks.is_empty() {
            block = block.push(text("Linked from").size(12).color(ink.muted));
            for (id, title) in &self.cached_backlinks {
                let id = id.clone();
                block = block.push(
                    button(text(title.clone()).size(14).width(Fill))
                        .width(Fill)
                        .padding([4, 8])
                        .on_press(Message::Select(id))
                        .style(list_row(false)),
                );
            }
        }
        container(block)
            .width(240)
            .height(Fill)
            .style(surface_pane)
            .into()
    }

    /// What belongs to this one note and not to the page: where it was written, and what can be
    /// done with it as a whole.
    fn note_settings<'a>(
        &'a self,
        mut block: iced::widget::Column<'a, Message>,
    ) -> iced::widget::Column<'a, Message> {
        let ink = self.ink();
        let saved = self.selected.is_some();
        block = block.push(text("Home device").size(12).color(ink.muted));
        let home = if !saved {
            "Not saved yet.".to_owned()
        } else if self.note_writer.is_empty() {
            "This computer.".to_owned()
        } else {
            self.note_writer.clone()
        };
        block = block.push(text(home).size(14));
        let has_words = saved || !self.body.text().is_empty() || !self.title_text.is_empty();
        if has_words {
            block = block.push(space().height(6));
            block = block.push(action_fill("Send to a chat", Message::SendNoteToChat));
        }
        if saved || self.editor_dirty() {
            block = block.push(ghost_label(
                if saved { "Delete note" } else { "Discard" },
                Message::AskDelete,
            ));
        }
        block
    }

    fn edge_toggle(&self, list: bool) -> Element<'_, Message> {
        let open = if list {
            self.show_list
        } else {
            self.show_context
        };
        let mark = match (list, open) {
            (true, true) | (false, false) => "‹",
            _ => "›",
        };
        let hint = match (list, open) {
            (true, true) => "Hide the list",
            (true, false) => "Show the list",
            (false, true) => "Hide the group",
            (false, false) => "Show the group",
        };
        let message = if list {
            Message::ToggleList
        } else {
            Message::ToggleContext
        };
        let face = button(
            container(text(mark).size(18))
                .width(Fill)
                .height(Fill)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center),
        )
        .width(28)
        .height(Fill)
        .padding(0)
        .on_press(message)
        .style(icon_style);
        hover_label(face, hint)
    }

    fn read_pages(&self, chapters: &[Chapter]) -> Element<'_, Message> {
        markdown::set_known_links(self.known_links.clone());
        markdown::set_misspellings(Arc::clone(&self.misspellings));
        let ink = self.ink();
        let mut pages = column![].spacing(0).width(Fill);
        for (index, chapter) in chapters.iter().enumerate() {
            if index > 0 {
                pages = pages.push(space().height(36));
                pages = pages.push(hairline());
                pages = pages.push(space().height(36));
            }
            let mut part = column![text(chapter.name.clone()).font(serif()).size(24)]
                .spacing(16)
                .width(Fill);
            if chapter.body.trim().is_empty() {
                part = part.push(text("Not written yet.").size(14).color(ink.muted));
            } else {
                part = part.push(markdown::reading(&chapter.body, self.is_dark()));
            }
            pages = pages.push(part);
        }
        vscroll(pages).height(Fill).into()
    }

    fn book_title(&self) -> String {
        let Some(id) = &self.book_id else {
            return self.title_text.clone();
        };
        if id.is_empty() || self.selected.as_deref() == Some(id.as_str()) {
            return self.title_text.clone();
        }
        self.catalog
            .iter()
            .find(|note| note.id == *id)
            .map(|note| note.title.clone())
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| "Untitled".to_owned())
    }

    fn track_book(&mut self) {
        let names = markdown::chapter_names(&self.body.text());
        if names.len() >= 2 && !self.context_shut {
            self.show_context = true;
        }
        if names.len() >= 2 {
            self.group_cache = None;
            if let Some(id) = self.selected.clone() {
                if self.book_id.as_ref().is_some_and(|book| book != &id)
                    && self.linked_from_book(&self.title_text)
                {
                    return;
                }
                self.book_id = Some(id);
            } else {
                let keep = self.book_id.as_ref().is_some_and(|book| !book.is_empty());
                if keep && self.linked_from_book(&self.title_text) {
                    return;
                }
                self.book_id = Some(String::new());
            }
            return;
        }
        if self.on_group_note() || self.linked_from_book(&self.title_text) {
            return;
        }
        self.remember_group();
    }

    /// A piece opened from the list still belongs to the note that links it.
    fn remember_group(&mut self) {
        if self
            .group_cache
            .as_ref()
            .is_some_and(|(title, _)| title == &self.title_text)
        {
            if let Some((_, Some(id))) = &self.group_cache {
                let id = id.clone();
                if !self.context_shut {
                    self.show_context = true;
                }
                self.book_id = Some(id);
            } else {
                self.book_id = None;
                self.reading = false;
            }
            return;
        }
        let found = self.group_for(&self.title_text);
        self.group_cache = Some((self.title_text.clone(), found.clone()));
        if let Some(id) = found {
            if !self.context_shut {
                self.show_context = true;
            }
            self.book_id = Some(id);
        } else if !self.on_group_note() {
            self.book_id = None;
            self.reading = false;
        }
    }

    fn group_for(&self, title: &str) -> Option<String> {
        let title = title.trim();
        if title.is_empty() {
            return None;
        }
        if let Some(id) = &self.book_id
            && self
                .groups
                .iter()
                .any(|(group, targets)| group == id && self.names_in(targets, title))
        {
            return Some(id.clone());
        }
        self.groups
            .iter()
            .find_map(|(id, targets)| self.names_in(targets, title).then(|| id.clone()))
    }

    fn names_in(&self, targets: &[String], title: &str) -> bool {
        targets.iter().any(|target| self.names_piece(target, title))
    }

    fn linked_from_book(&self, title: &str) -> bool {
        if title.is_empty() {
            return false;
        }
        let Some(id) = &self.book_id else {
            return false;
        };
        self.groups
            .iter()
            .any(|(group, targets)| group == id && self.names_in(targets, title))
    }

    fn find_title(&self, name: &str) -> Option<String> {
        if name.is_empty() {
            return None;
        }
        if self.title_text.eq_ignore_ascii_case(name)
            && let Some(id) = &self.selected
        {
            return Some(id.clone());
        }
        for (key, title, _) in &self.unsaved {
            if title.eq_ignore_ascii_case(name)
                && let Some(id) = key
            {
                return Some(id.clone());
            }
        }
        self.catalog
            .iter()
            .find(|note| !note.title.is_empty() && note.title.eq_ignore_ascii_case(name))
            .map(|note| note.id.clone())
    }

    fn open_chapter(&mut self, name: &str) -> Task<Message> {
        let name = name.trim().to_owned();
        if name.is_empty() {
            return Task::none();
        }
        if markdown::is_note_id(&name) {
            if self.catalog.iter().any(|note| note.id == name) {
                self.reading = false;
                self.show_note(&name);
            } else {
                self.say("That note is gone.");
            }
            return Task::none();
        }
        if let Some(id) = self.find_title(&name) {
            self.reading = false;
            self.show_note(&id);
            return Task::none();
        }
        if self.saving {
            self.pending_chapter = Some(name);
            return Task::none();
        }
        self.saving = true;
        let dir = self.dir.clone();
        let current = if self.editor_dirty() {
            Some((
                self.selected.clone(),
                self.title_text.clone(),
                self.stored_body(),
            ))
        } else {
            None
        };
        Task::perform(
            async move {
                let (sender, receiver) = iced::futures::channel::oneshot::channel();
                std::thread::spawn(move || {
                    let result = write_chapter(&dir, current.as_ref(), &name);
                    let _ = sender.send(result);
                });
                receiver
                    .await
                    .unwrap_or_else(|_| Err("Could not add that chapter.".to_owned()))
            },
            Message::ChapterMade,
        )
    }

    fn finish_chapter(&mut self, result: Result<MadeChapter, String>) -> Task<Message> {
        self.saving = false;
        let made = match result {
            Ok(made) => made,
            Err(err) => {
                self.message = err;
                return self.take_pending_chapter();
            }
        };
        if let Some(saved) = made.saved {
            if self.title_text == saved.title && self.stored_body() == saved.body {
                self.disk_title = saved.title;
                self.disk_body = saved.body;
                self.disarm_pause();
            }
            if self.selected.is_none() {
                if self.history_key.is_empty() {
                    self.history_key.clone_from(&saved.id);
                }
                self.selected = Some(saved.id.clone());
            }
            if self.book_id.as_deref() == Some("") {
                self.book_id = Some(saved.id);
            }
        }
        if self.install_latest() {
            self.post_side();
        } else {
            self.take_notes(made.notes);
        }
        self.reading = false;
        self.show_note(&made.id);
        self.take_pending_chapter()
    }

    fn take_pending_chapter(&mut self) -> Task<Message> {
        let Some(name) = self.pending_chapter.take() else {
            return Task::none();
        };
        self.open_chapter(&name)
    }

    fn note_follow(&mut self, scroll: bool) -> Task<Message> {
        let jump = if scroll {
            self.body.follow(self.writing_width())
        } else {
            None
        };
        let focus = iced::widget::operation::focus(iced::widget::Id::new("note-body"));
        if let Some(top) = jump {
            let y = (top - 72.0).max(0.0);
            return Task::batch([
                focus,
                iced::widget::operation::scroll_to(
                    iced::widget::Id::new("note-scroll"),
                    iced::widget::operation::AbsoluteOffset { x: 0.0, y },
                ),
            ]);
        }
        focus
    }

    /// Copy the notebook folder to the place typed in settings.
    ///
    /// A note that was typed in the last moment is saved first, so it is in the copy. The copy
    /// runs on its own thread and holds the notes lock and the chat lock, so no save or sync lands
    /// in the middle of it.
    fn start_backup(&mut self) -> Task<Message> {
        if self.backup_busy {
            return Task::none();
        }
        if self.notebook.is_none() {
            self.say("Folder is not open.");
            return Task::none();
        }
        if self.saving || self.editor_dirty() {
            self.say("Saving your note first. Press Back up again in a moment.");
            return self.start_save(true);
        }
        let typed = self.backup_to.trim();
        let dest = if typed.is_empty() {
            profile::default_backup_dir()
        } else {
            profile::expand_home(typed)
        };
        self.backup_to = dest.display().to_string();
        self.backup_busy = true;
        self.say("Backing up.");
        let dir = self.dir.clone();
        let name = format!(
            "aerodash-{}",
            clock::stamp(clock::now_ms(), &clock::load_tz())
        );
        Task::perform(
            async move {
                let (sender, receiver) = iced::futures::channel::oneshot::channel();
                std::thread::spawn(move || {
                    let result = {
                        let _notes = notes_lock();
                        let _chat = chat_lock();
                        profile::backup_folder(&dir, &dest, &name)
                    };
                    let _ = sender.send(result);
                });
                receiver
                    .await
                    .unwrap_or_else(|_| Err("The copy stopped before it finished.".to_owned()))
            },
            Message::BackedUp,
        )
    }

    fn finish_backup(&mut self, result: Result<profile::BackupReport, String>) {
        self.backup_busy = false;
        match result {
            Ok(report) => {
                self.backup_at = clock::now_ms();
                self.persist_profile();
                self.say(&format!(
                    "Backed up {} files, {}, to {}.",
                    report.files,
                    size_label(report.bytes),
                    report.path.display()
                ));
            }
            Err(err) => self.message = err,
        }
    }

    /// Type a key that the editor ignored. Only while the writing page is the thing on screen and
    /// nothing else, such as a popup, wants keys. A field that has focus takes its own keys, so
    /// those never get here.
    fn type_ignored(&mut self, typed: &Typed) -> Task<Message> {
        let writing = self.notebook.is_some()
            && self.welcome.is_none()
            && !self.breeze_on()
            && self.editing_note()
            && !self.reading
            && self.older.is_none()
            && !self.confirm_delete
            && !self.send_open
            && self.conflicts.is_empty();
        if !writing {
            return Task::none();
        }
        let edit = |edit: text_editor::Edit| Message::BodyEdited(text_editor::Action::Edit(edit));
        let messages: Vec<Message> = match typed {
            Typed::Text(text) => text
                .chars()
                .map(|ch| edit(text_editor::Edit::Insert(ch)))
                .collect(),
            Typed::Enter => vec![edit(text_editor::Edit::Enter)],
            Typed::Backspace => vec![edit(text_editor::Edit::Backspace)],
            Typed::Delete => vec![edit(text_editor::Edit::Delete)],
        };
        Task::batch(messages.into_iter().map(|message| self.update(message)))
    }

    fn share_words(&self) {
        let words: Vec<String> = self.my_words.iter().cloned().collect();
        markdown::set_personal_words(&words);
    }

    /// Add the word at the caret to the dictionary, and check the page again so its underline goes.
    fn add_word(&mut self) {
        if !self.editing_note() {
            return;
        }
        let Some(word) = markdown::word_at(&self.body.text(), self.body.caret_byte()) else {
            self.say("Put the cursor in a word first.");
            return;
        };
        if !self.my_words.insert(word.clone()) {
            self.say("That word is already in your dictionary.");
            return;
        }
        let words: Vec<String> = self.my_words.iter().cloned().collect();
        if let Err(err) = profile::store_words(&self.dir, &words) {
            self.message = err;
            return;
        }
        self.share_words();
        self.proof_body.clear();
        self.maybe_proof();
        self.say(&format!("Added {word} to your dictionary."));
    }

    fn say(&mut self, text: &str) {
        text.clone_into(&mut self.message);
    }

    fn start_relay(&mut self) {
        match ambient_cli::start_loopback_relay() {
            Ok(()) => {
                self.relay_ready = true;
                self.relay_ours = true;
                "Relay is listening on 127.0.0.1:8787.".clone_into(&mut self.sync_detail);
            }
            Err(err) => {
                if err.to_string().contains("already running") {
                    self.relay_ready = true;
                    self.relay_ours = false;
                    self.sync_detail = err.to_string();
                } else {
                    self.relay_ready = false;
                    self.relay_ours = false;
                    self.sync_detail = err.to_string();
                }
            }
        }
    }

    fn stop_relay(&mut self) {
        match ambient_cli::stop_loopback_relay() {
            Ok(()) => {
                self.relay_ready = false;
                self.relay_ours = false;
                self.net_up = false;
                "Relay stopped.".clone_into(&mut self.sync_detail);
            }
            Err(err) => self.sync_detail = err.to_string(),
        }
    }

    fn use_this_computer(&mut self) {
        let choice = ambient_cli::net_setting(&self.dir)
            .map(|setting| setting.choice)
            .unwrap_or_default();
        let setting = ambient_cli::NetSetting {
            mode: ambient_cli::NetMode::ThisComputer,
            pin: self.pin_draft.clone(),
            backup: self.backup_draft.clone(),
            choice,
        };
        if let Err(err) = ambient_cli::set_net_setting(&self.dir, &setting) {
            self.sync_detail = err.to_string();
            return;
        }
        self.net_mode = ambient_cli::NetMode::ThisComputer;
        self.net_up = false;
        self.start_relay();
    }

    fn save_pinned(&mut self) {
        let choice = ambient_cli::net_setting(&self.dir)
            .map(|setting| setting.choice)
            .unwrap_or_default();
        let setting = ambient_cli::NetSetting {
            mode: ambient_cli::NetMode::Pin,
            pin: self.pin_draft.trim().to_owned(),
            backup: self.backup_draft.trim().to_owned(),
            choice,
        };
        match ambient_cli::set_net_setting(&self.dir, &setting) {
            Ok(()) => {
                self.net_mode = ambient_cli::NetMode::Pin;
                self.net_up = false;
                "Pinned relay saved.".clone_into(&mut self.sync_detail);
            }
            Err(err) => self.sync_detail = err.to_string(),
        }
    }

    fn mark_pending(&mut self) {
        self.pending = true;
        self.writes = self.writes.saturating_add(1);
    }

    fn open_paired(&mut self) {
        if !self.notebook.as_ref().is_some_and(Notebook::holds_root) {
            self.say("This window is the paired device. It does not hold the root.");
            return;
        }
        let path = sibling(&self.dir, "-device");
        if path.join("key").exists() {
            self.spawn_at(
                &path,
                "Opened the paired window. Close a duplicate if one was already open.",
            );
            return;
        }
        let offer = match ambient_cli::device_offer(&path, "other-computer") {
            Ok(offer) => offer,
            Err(err) => {
                self.message = err.to_string();
                return;
            }
        };
        match self
            .notebook
            .as_ref()
            .map(|book| book.pair_device(&offer, "other-computer"))
        {
            Some(Err(err)) => {
                let _ = std::fs::remove_dir_all(&path);
                self.message = err.to_string();
                return;
            }
            None => return,
            Some(Ok(())) => {}
        }
        self.spawn_at(
            &path,
            "Paired. That window does not hold the root. Both windows sync while the relay is on.",
        );
    }

    fn open_person(&mut self) {
        if !self.notebook.as_ref().is_some_and(Notebook::holds_root) {
            self.say("Open the other person from the window that holds the root.");
            return;
        }
        let path = sibling(&self.dir, "-person");
        if !path.join("key").exists()
            && let Err(err) = Notebook::create(&path)
        {
            self.message = err.to_string();
            return;
        }
        self.spawn_at(
            &path,
            "Opened the other person. In that window open Invite and copy the link. Paste it here. Only this window presses Start conversation.",
        );
    }

    fn copy_card(&mut self) -> Task<Message> {
        match ambient_cli::contact_card(&self.dir) {
            Ok(card) => {
                self.my_card.clone_from(&card);
                self.say("Card copied. The other window wants the invite link, not this card.");
                clipboard::write(card)
            }
            Err(err) => {
                self.message = err.to_string();
                Task::none()
            }
        }
    }

    fn start_chat(&mut self) {
        let card = self.their_card.trim();
        if card.is_empty() {
            self.say("Paste their link first. In their window open Invite and copy the link.");
            return;
        }
        match ambient_cli::start_conversation(&self.dir, card) {
            Ok(id) => {
                let call = self.call_draft.trim().to_owned();
                if !call.is_empty() {
                    self.aliases.insert(id.clone(), call);
                    self.persist_names();
                }
                self.call_draft.clear();
                self.chat_id = Some(id);
                self.screen = Screen::Chat;
                self.sheet = Sheet::None;
                self.their_card.clear();
                self.say("Conversation saved here. Both windows sync while the relay is on.");
                self.sync_watch();
                self.post(lane::Job::Inbox);
                self.mark_pending();
            }
            Err(err) => self.message = err.to_string(),
        }
    }

    fn send_chat(&mut self) {
        let text = self.draft_editor.text().trim().to_owned();
        let Some(id) = self.chat_id.clone() else {
            self.say("Invite someone before sending.");
            return;
        };
        let files = std::mem::take(&mut self.pending_files);
        if text.is_empty() && files.is_empty() {
            self.say("Write a message, or add a file.");
            return;
        }
        self.draft_editor = text_editor::Content::new();
        self.screen = Screen::Chat;
        self.detail = true;
        self.sync_watch();
        let cite = self.chat_cite.take().map(|(id, _)| id);
        self.cite_open = false;
        self.post(lane::Job::Send {
            id,
            text,
            files,
            cite,
        });
    }

    fn spawn_at(&mut self, path: &Path, ok_message: &str) {
        let exe = match std::env::current_exe() {
            Ok(exe) => exe,
            Err(err) => {
                self.message = format!("could not open another window: {err}");
                return;
            }
        };
        match std::process::Command::new(exe).arg(path).spawn() {
            Ok(_) => ok_message.clone_into(&mut self.message),
            Err(err) => self.message = format!("could not open another window: {err}"),
        }
    }

    fn show_phrase(&mut self) {
        match ambient_cli::recovery_phrase(&self.dir) {
            Ok(phrase) => {
                self.recovery_phrase = phrase;
                if self.welcome.is_none() {
                    self.say("Recovery phrase shown. Copy it and store it offline.");
                }
            }
            Err(err) => self.message = err.to_string(),
        }
    }

    fn copy_phrase(&mut self) -> Task<Message> {
        if self.recovery_phrase.is_empty() {
            self.say("Show the phrase first.");
            return Task::none();
        }
        self.say("Phrase copied.");
        clipboard::write(self.recovery_phrase.clone())
    }

    fn start_restore(&mut self) {
        let phrase = self.restore_phrase.trim();
        if phrase.is_empty() {
            self.say("Paste the recovery phrase first.");
            return;
        }
        let path = sibling(&self.dir, "-restored");
        if path.join("key").exists() {
            self.say("Restored folder already exists. Open it separately or delete it first.");
            return;
        }
        match ambient_cli::restore_folder(&path, phrase, ambient_cli::LOOPBACK_URL) {
            Ok(()) => {
                self.spawn_at(
                    &path,
                    "Restored. That window holds the root and enrolled this computer. Old devices stay until you revoke them.",
                );
            }
            Err(err) => self.message = err.to_string(),
        }
    }

    fn copy_link(&mut self) -> Task<Message> {
        if self.contact_link.is_empty() {
            self.say("No invite link yet.");
            return Task::none();
        }
        self.say("Invite link copied. Send it by text.");
        clipboard::write(self.contact_link.clone())
    }

    fn toggle_qr(&mut self) {
        self.show_qr = !self.show_qr;
        if self.show_qr && self.qr_bytes.is_none() {
            match generate_qr(&self.contact_link) {
                Ok(bytes) => self.qr_bytes = Some(bytes),
                Err(err) => {
                    self.message = err;
                    self.show_qr = false;
                }
            }
        }
    }

    fn revoke(&mut self, id: &str) {
        let Some(book) = self.notebook.as_ref() else {
            return;
        };
        match book.revoke_device(id) {
            Ok(()) => {
                self.devices = book.devices().unwrap_or_default();
                self.say("Revoked. New events are no longer sealed to that device. Notes it already wrote stay.");
                self.mark_pending();
            }
            Err(err) => self.message = err.to_string(),
        }
    }

    fn send_note_to_chat(&mut self) {
        if self.title_text.trim().is_empty() && self.body.text().trim().is_empty() {
            self.say("Write a note first.");
            return;
        }
        if self.chats.is_empty() {
            self.say("Start a conversation before sending a note.");
            return;
        }
        if self
            .send_choice
            .as_ref()
            .is_none_or(|id| !self.chats.iter().any(|chat| &chat.id == id))
        {
            self.send_choice = self
                .chat_id
                .clone()
                .filter(|id| self.chats.iter().any(|chat| &chat.id == id))
                .or_else(|| self.chats.first().map(|chat| chat.id.clone()));
        }
        self.send_open = true;
        self.screen = Screen::Notes;
        self.sheet = Sheet::None;
        self.detail = true;
    }

    fn confirm_send(&mut self) {
        let Some(id) = self.send_choice.clone() else {
            self.say("Choose a chat.");
            return;
        };
        if self.title_text.trim().is_empty() && self.body.text().trim().is_empty() {
            self.say("Write a note first.");
            return;
        }
        let mut text = String::new();
        if !self.title_text.is_empty() {
            text.push_str(&self.title_text);
            text.push_str("\n\n");
        }
        text.push_str(&self.body.text());
        let label = self
            .chats
            .iter()
            .find(|chat| chat.id == id)
            .map_or_else(|| "that chat".to_owned(), |chat| self.chat_name(chat));
        self.send_open = false;
        self.chat_id = Some(id.clone());
        self.say(&format!(
            "Sent to {label}. The words sync while a relay is on."
        ));
        self.mark_pending();
        self.post(lane::Job::Send {
            id,
            text,
            files: Vec::new(),
            cite: self.selected.clone(),
        });
    }

    fn add_loose_task(&mut self) -> Task<Message> {
        let mut label = self.task_draft.trim().replace(['\n', '\r'], " ");
        if label.is_empty() {
            self.say("Write the task first.");
            return Task::none();
        }
        if let Some((id, title)) = self.task_cite.clone() {
            let title = title.replace(['|', '[', ']'], "");
            label = format!("{label} [[{id}|{title}]]");
        }
        if self.notebook.is_none() {
            self.say("Folder is not open.");
            return Task::none();
        }
        let line = format!("- [ ] {label}");
        let existing = self
            .catalog
            .iter()
            .find(|note| note.title == "Tasks" && !note.conflict)
            .map(|note| note.id.clone());
        let Some(id) = existing else {
            self.task_draft.clear();
            self.task_cite = None;
            self.task_cite_open = false;
            self.say("Added to Tasks.");
            return self.apply_note(None, "Tasks".to_owned(), format!("{line}\n"));
        };
        let Some((body, conflict, title)) = self.task_source(&id) else {
            self.say("That note is not here.");
            return Task::none();
        };
        if conflict {
            self.show_note(&id);
            self.say("Two versions. Write the one you want to keep, then save.");
            return Task::none();
        }
        let mut next = body;
        if !next.is_empty() && !next.ends_with('\n') {
            next.push('\n');
        }
        next.push_str(&line);
        next.push('\n');
        let follow = self.write_note(&id, &title, &next);
        if self.message.is_empty() {
            self.task_draft.clear();
            self.task_cite = None;
            self.task_cite_open = false;
            self.say("Added to Tasks.");
        }
        follow
    }

    fn delete_open_note(&mut self) -> Task<Message> {
        self.confirm_delete = false;
        if self.saving {
            self.say("Still saving. Try delete again in a moment.");
            return Task::none();
        }
        let Some(id) = self.selected.clone() else {
            self.discard_draft();
            self.say("Discarded.");
            return Task::none();
        };
        if self.notebook.is_none() {
            self.say("Folder is not open.");
            return Task::none();
        }
        self.saving = true;
        self.save_token = self.save_token.wrapping_add(1);
        self.unsaved
            .retain(|(key, _, _)| key.as_deref() != Some(id.as_str()));
        self.selected = None;
        self.title_text.clear();
        self.tags.clear();
        self.tag_draft.clear();
        self.body.load("");
        self.disk_title.clear();
        self.disk_body.clear();
        self.conflicts.clear();
        self.note_writer.clear();
        self.disarm_pause();
        self.histories.remove(&id);
        self.history_order.retain(|key| key != &id);
        if self.book_id.as_deref() == Some(id.as_str()) {
            self.book_id = None;
            self.reading = false;
        }
        if self.history_key == id {
            self.history.clear();
        }
        self.swap_history("");
        self.catalog.retain(|note| note.id != id);
        self.refresh_shown();
        self.post_side();
        self.say("Deleted. It has left the list.");
        let dir = self.dir.clone();
        Task::perform(
            async move {
                let (sender, receiver) = iced::futures::channel::oneshot::channel();
                std::thread::spawn(move || {
                    let result = write_snapshot(&dir, Some(&id), "", ambient_cli::DELETED_BODY)
                        .map(|(_id, notes)| notes);
                    let _ = sender.send(result);
                });
                receiver
                    .await
                    .unwrap_or_else(|_| Err("Delete stopped before it finished.".to_owned()))
            },
            Message::Deleted,
        )
    }

    fn finish_delete(&mut self, result: Result<Vec<NoteSummary>, String>) -> Task<Message> {
        self.saving = false;
        match result {
            Ok(notes) => {
                self.writes = self.writes.saturating_add(1);
                self.pending = true;
                if !self.install_latest() {
                    self.take_notes(notes);
                }
            }
            Err(err) => self.message = err,
        }
        if self.save_again && self.editor_dirty() {
            self.save_again = false;
            return self.start_save(true);
        }
        self.save_again = false;
        Task::none()
    }

    fn discard_draft(&mut self) {
        self.unsaved.retain(|(key, _, _)| key.is_some());
        if self.book_id.as_deref() == Some("") {
            self.book_id = None;
            self.reading = false;
        }
        self.title_text.clear();
        self.tags.clear();
        self.tag_draft.clear();
        self.body.load("");
        self.disk_title.clear();
        self.disk_body.clear();
        self.disarm_pause();
        self.confirm_delete = false;
        self.save_token = self.save_token.wrapping_add(1);
        self.histories.remove("");
        self.history_order.retain(|key| !key.is_empty());
        if self.history_key.is_empty() {
            self.clear_history();
        }
    }

    fn write_note(&mut self, id: &str, title: &str, body: &str) -> Task<Message> {
        if self.selected.as_deref() == Some(id) {
            return self.put_note_text(id, title, body);
        }
        if let Some(slot) = self
            .unsaved
            .iter_mut()
            .find(|(key, _, _)| key.as_deref() == Some(id))
        {
            title.clone_into(&mut slot.1);
            body.clone_into(&mut slot.2);
        }
        self.apply_note(Some(id.to_owned()), title.to_owned(), body.to_owned())
    }

    fn breeze_on(&self) -> bool {
        !self.loaded || self.boot_at.elapsed() < Duration::from_secs(3)
    }

    fn breeze_screen(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let phase = self.boot_at.elapsed().as_secs_f32();
        canvas(Breeze {
            phase,
            bg: ink.bg,
            ink: ink.muted,
            second: ink.accent,
            word: ink.text,
        })
        .width(Fill)
        .height(Fill)
        .into()
    }

    fn submit_unlock(&mut self) -> Task<Message> {
        let passphrase = self.unlock_input.trim().to_owned();
        if self.open_error == "this folder is locked" {
            match ambient_cli::unlock_folder(&self.dir, &passphrase) {
                Ok(()) => {
                    self.unlock_input.clear();
                    self.open_error.clear();
                    match Notebook::open(&self.dir) {
                        Ok(book) => {
                            self.notebook = Some(book);
                            self.reload();
                            self.post(lane::Job::Hello(self.dir.clone()));
                            self.post(lane::Job::Inbox);
                            self.sync_watch();
                            if let Ok(card) = ambient_cli::contact_card(&self.dir) {
                                self.my_card = card;
                            }
                            if let Ok(link) = ambient_cli::contact_link(&self.dir) {
                                self.contact_link = link;
                            }
                            self.say("Unlocked for this run. The key file on disk stays wrapped.");
                            self.pending = false;
                            self.start_relay();
                            return self.begin_sync();
                        }
                        Err(err) => self.open_error = err.to_string(),
                    }
                }
                Err(err) => self.message = err.to_string(),
            }
            return Task::none();
        }
        match ambient_cli::set_folder_unlock(&self.dir, &passphrase) {
            Ok(()) => {
                self.unlock_input.clear();
                self.say(
                    "Key file wrapped. The passphrase is not the root and not the recovery phrase.",
                );
            }
            Err(err) => self.message = err.to_string(),
        }
        Task::none()
    }

    fn finish_open(&mut self, result: Result<(), String>) -> Task<Message> {
        match result {
            Ok(()) => {
                let Some(book) = take_notebook() else {
                    "could not open this folder".clone_into(&mut self.open_error);
                    self.loaded = true;
                    return Task::none();
                };
                self.adopt_open_book(book);
                self.loaded = true;
                if self.open_error != "this folder is locked"
                    && self.net_mode == ambient_cli::NetMode::ThisComputer
                {
                    self.start_relay();
                }
                self.begin_sync()
            }
            Err(err) => {
                self.open_error = err;
                self.loaded = true;
                Task::none()
            }
        }
    }

    fn adopt_open_book(&mut self, book: Notebook) {
        let paired = !book.holds_root();
        self.notebook = Some(book);
        self.reload();
        if let Some(side) = lane::take_side() {
            self.apply_side(side);
        }
        if let Some(rows) = lane::take_inbox() {
            self.apply_inbox(&rows);
        }
        self.post(lane::Job::Hello(self.dir.clone()));
        self.sync_watch();
        if let Ok(card) = ambient_cli::contact_card(&self.dir) {
            self.my_card = card;
        }
        if let Ok(link) = ambient_cli::contact_link(&self.dir) {
            self.contact_link = link;
        }
        self.devices = self
            .notebook
            .as_ref()
            .and_then(|book| book.devices_held().ok())
            .unwrap_or_default();
        if self.device_draft.is_empty()
            && let Some(label) = self
                .devices
                .iter()
                .find(|device| device.current)
                .map(|device| device.label.clone())
        {
            self.device_draft = label;
            self.persist_profile();
        }
        if let Ok(setting) = ambient_cli::net_setting(&self.dir) {
            self.net_mode = setting.mode;
            self.pin_draft = setting.pin;
            self.backup_draft = setting.backup;
        }
        self.pending = ambient_cli::has_unsent(&self.dir).unwrap_or(false);
        if paired && self.notes.is_empty() && self.seen {
            self.say(
                "Paired device. It does not hold the root. It syncs on its own while the relay is on.",
            );
        }
        if !self.seen {
            self.welcome = Some(WelcomeStep::Phrase);
            self.show_phrase();
        }
    }

    fn finish_save(
        &mut self,
        token: u64,
        result: Result<(String, Vec<NoteSummary>), String>,
        title: String,
        body: String,
    ) -> Task<Message> {
        self.saving = false;
        let current = token == self.save_token;
        match result {
            Ok((id, notes)) => {
                if current && self.selected.is_none() {
                    if self.history_key.is_empty() {
                        self.history_key.clone_from(&id);
                    }
                    if self.book_id.as_deref() == Some("") {
                        self.book_id = Some(id.clone());
                    }
                    self.selected = Some(id);
                }
                if current && self.title_text == title && self.stored_body() == body {
                    self.disk_title = title;
                    self.disk_body = body;
                    self.disarm_pause();
                }
                self.writes = self.writes.saturating_add(1);
                self.pending = true;
                if self.install_latest() {
                    self.post_side();
                    if self.screen == Screen::Tasks {
                        self.rebuild_tasks();
                    }
                } else {
                    self.take_notes(notes);
                }
                if current
                    && self.editor_dirty()
                    && (self.save_again
                        || self
                            .edited_at
                            .is_some_and(|at| at.elapsed() >= Duration::from_millis(700)))
                {
                    self.save_again = false;
                    return self.start_save(true);
                }
                self.save_again = false;
            }
            Err(err) => {
                self.message = err;
                if current && self.save_again && self.editor_dirty() {
                    self.save_again = false;
                    return self.start_save(true);
                }
                self.save_again = false;
            }
        }
        self.take_pending_chapter()
    }

    fn finish_applied(&mut self, result: Result<Vec<NoteSummary>, String>) -> Task<Message> {
        match result {
            Ok(notes) => {
                self.writes = self.writes.saturating_add(1);
                self.pending = true;
                if self.install_latest() {
                    self.post_side();
                    // The tasks come from the notebook that was just installed. Without this the
                    // tab kept showing the old list until it was left and opened again.
                    if self.screen == Screen::Tasks {
                        self.rebuild_tasks();
                    }
                } else {
                    self.take_notes(notes);
                }
            }
            Err(err) => self.message = err,
        }
        Task::none()
    }

    fn take_notes(&mut self, notes: Vec<NoteSummary>) {
        if self.tag.as_ref().is_some_and(|tag| {
            !notes
                .iter()
                .any(|note| note.tags.iter().any(|item| item == tag))
        }) {
            self.tag = None;
        }
        self.catalog = notes;
        self.refresh_shown();
        self.post_side();
        if self.screen == Screen::Tasks {
            self.rebuild_tasks();
        }
    }

    fn apply_note(&self, id: Option<String>, title: String, body: String) -> Task<Message> {
        #[cfg(test)]
        tests_support::record_write(id.as_deref(), &title, &body);
        let dir = self.dir.clone();
        Task::perform(
            async move {
                let (sender, receiver) = iced::futures::channel::oneshot::channel();
                std::thread::spawn(move || {
                    let result = write_snapshot(&dir, id.as_deref(), &title, &body)
                        .map(|(_id, notes)| notes);
                    let _ = sender.send(result);
                });
                receiver
                    .await
                    .unwrap_or_else(|_| Err("Save stopped before it finished.".to_owned()))
            },
            Message::Applied,
        )
    }

    fn persist_profile(&mut self) {
        let profile = profile::Profile {
            device: self.device_draft.clone(),
            name: self.your_name.clone(),
            appearance: self.appearance,
            seen: self.seen,
            backup_to: self.backup_to.clone(),
            backup_at: self.backup_at,
        };
        if let Err(err) = profile::store_profile(&self.dir, &profile) {
            self.message = err;
        }
    }

    fn persist_names(&mut self) {
        let names: Vec<(String, String)> = self
            .aliases
            .iter()
            .map(|(id, name)| (id.clone(), name.clone()))
            .collect();
        if let Err(err) = profile::store_names(&self.dir, &names) {
            self.message = err;
        }
    }

    fn finish_onboarding(&mut self) -> Task<Message> {
        let device = self.device_draft.trim();
        if device.is_empty() {
            self.say("Name this computer.");
            return Task::none();
        }
        if device.len() > 32 {
            self.say("Use a shorter name for this computer. 32 bytes is the limit.");
            return Task::none();
        }
        if self.dir.join("key").exists() {
            self.say("This folder already has a notebook.");
            return Task::none();
        }
        match Notebook::create_with_label(&self.dir, device) {
            Ok(book) => {
                self.device_draft = device.to_owned();
                self.your_name = self.name_draft.trim().to_owned();
                self.name_draft.clone_from(&self.your_name);
                self.seen = false;
                self.persist_profile();
                self.open_error.clear();
                self.adopt_open_book(book);
                self.loaded = true;
                if self.net_mode == ambient_cli::NetMode::ThisComputer {
                    self.start_relay();
                }
                self.begin_sync()
            }
            Err(err) => {
                self.message = err.to_string();
                Task::none()
            }
        }
    }

    fn export_notes(&mut self) -> Task<Message> {
        let Some(book) = self.notebook.as_ref() else {
            self.say("Nothing to export yet.");
            return Task::none();
        };
        let mut notes = Vec::new();
        if let Err(err) = book.visit_held(|id, title, body, _conflict| {
            notes.push((id.to_owned(), title.to_owned(), body.to_owned()));
        }) {
            self.message = err.to_string();
            return Task::none();
        }
        if self.editor_dirty() {
            if let Some(id) = &self.selected {
                if let Some(slot) = notes.iter_mut().find(|(note_id, _, _)| note_id == id) {
                    slot.1.clone_from(&self.title_text);
                    slot.2 = self.stored_body();
                }
            } else if !self.title_text.is_empty() || !self.stored_body().is_empty() {
                notes.push((
                    "unsaved".to_owned(),
                    self.title_text.clone(),
                    self.stored_body(),
                ));
            }
        }
        match profile::export_notes(&self.dir, &notes) {
            Ok(path) => self.say(&format!("Exported to {}.", path.display())),
            Err(err) => self.message = err,
        }
        Task::none()
    }

    fn wipe_notebook(&mut self) -> Task<Message> {
        if self.wipe_draft.trim() != "delete" {
            self.say("Type delete to erase this notebook.");
            return Task::none();
        }
        if self.saving {
            self.say("Still saving. Try again in a moment.");
            return Task::none();
        }
        self.notebook = None;
        let wiped = {
            let _notes = notes_lock();
            let _chat = chat_lock();
            profile::wipe_folder(&self.dir)
        };
        if let Err(err) = wiped {
            self.message = err;
            return Task::none();
        }
        self.reset_after_wipe();
        Task::none()
    }

    fn reset_after_wipe(&mut self) {
        // The words in the personal dictionary are from the notes that were just erased. They
        // must not be written back into the next notebook's file.
        self.my_words.clear();
        self.share_words();
        self.focus = false;
        self.slide_bar.jump(true);
        self.note_panel = false;
        self.catalog.clear();
        self.notes.clear();
        self.tasks.clear();
        self.query.clear();
        self.tag = None;
        self.selected = None;
        self.title_text.clear();
        self.tags.clear();
        self.tag_draft.clear();
        self.body.load("");
        self.disk_title.clear();
        self.disk_body.clear();
        self.conflicts.clear();
        self.note_writer.clear();
        self.chats.clear();
        self.chat.clear();
        self.chat_id = None;
        self.devices.clear();
        self.draft_editor = text_editor::Content::new();
        self.my_card.clear();
        self.their_card.clear();
        self.unsaved.clear();
        self.recovery_phrase.clear();
        self.restore_phrase.clear();
        self.contact_link.clear();
        self.show_qr = false;
        self.qr_bytes = None;
        self.pending_files.clear();
        self.task_draft.clear();
        self.task_query.clear();
        self.task_rename = None;
        self.task_rename_text.clear();
        self.chat_last.clear();
        self.chat_count.clear();
        self.seen_count.clear();
        self.chat_query.clear();
        self.hide_done = false;
        self.confirm_clear = false;
        self.task_due_text.clear();
        self.find_open = false;
        self.find_query.clear();
        self.find_at = usize::MAX;
        self.command_down = false;
        self.shift_down = false;
        self.pointer_down = false;
        self.drag_from = None;
        self.last_press = None;
        self.note_search = false;
        self.task_search = false;
        self.chat_search = false;
        self.invite_pick = None;
        self.call_draft.clear();
        self.wipe_draft.clear();
        self.aliases.clear();
        self.your_name.clear();
        self.name_draft.clear();
        self.device_draft.clear();
        self.appearance = profile::Appearance::Dark;
        self.seen = true;
        self.welcome = None;
        self.clear_history();
        self.histories.clear();
        self.history_order.clear();
        self.history_key.clear();
        self.open_error.clear();
        self.message.clear();
        self.disarm_pause();
        self.saving = false;
        self.save_again = false;
        self.pending = false;
        self.screen = Screen::Notes;
        self.sheet = Sheet::None;
        self.detail = false;
        self.confirm_delete = false;
        self.send_open = false;
        self.save_token = self.save_token.wrapping_add(1);
        self.loaded = true;
        self.boot_at = Instant::now();
        self.breeze_frame = 0;
    }

    fn onboarding_screen(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let width = (self.size.width - 64.0).clamp(280.0, 440.0);
        container(
            column![
                text("Welcome").font(serif()).size(40),
                text("Name this computer, then say what to call you.")
                    .size(16)
                    .color(ink.muted),
                column![
                    text("This computer").size(13).color(ink.muted),
                    text_input("A short name, like laptop", &self.device_draft)
                        .on_input(Message::DeviceDraft)
                        .padding(12)
                        .style(field_style),
                    text("Saved with this computer. It is not sent as your name.")
                        .size(13)
                        .color(ink.muted),
                ]
                .spacing(8),
                column![
                    text("Your name").size(13).color(ink.muted),
                    text_input("What should this notebook call you?", &self.name_draft)
                        .on_input(Message::NameDraft)
                        .padding(12)
                        .style(field_style),
                    text("This stays on this computer. An invite still has no name on it.")
                        .size(13)
                        .color(ink.muted),
                ]
                .spacing(8),
                column![
                    text("Look").size(13).color(ink.muted),
                    self.appearance_row(),
                ]
                .spacing(8),
                action_label("Continue", Message::FinishOnboarding),
                text(self.message.as_str()).size(13).color(ink.muted),
            ]
            .spacing(22)
            .width(width),
        )
        .center(Fill)
        .padding(32)
        .into()
    }

    fn welcome_screen(&self) -> Element<'_, Message> {
        let page = match self.welcome {
            Some(WelcomeStep::Guide) => self.guide_screen(),
            _ => self.phrase_screen(),
        };
        container(vscroll(page).width(Fill).height(Fill))
            .padding(32)
            .width(Fill)
            .height(Fill)
            .into()
    }

    fn phrase_screen(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let width = (self.size.width - 64.0).clamp(280.0, 520.0);
        let mut body = column![
            text("Recovery phrase").font(serif()).size(40),
            text("These words rebuild the notebook on a new computer. They are not a passphrase. A passphrase is optional. It only locks the key file on this computer, and you can set it later in Settings.")
                .size(16)
                .color(ink.muted),
        ]
        .spacing(16)
        .width(width);
        if self.recovery_phrase.is_empty() {
            body = body.push(text(self.message.as_str()).size(14).color(ink.muted));
            body = body.push(action_label("Show recovery phrase", Message::ShowPhrase));
        } else {
            body = body.push(
                container(text(&self.recovery_phrase).size(16))
                    .padding(14)
                    .width(Fill)
                    .style(field_well),
            );
            body = body.push(
                row![
                    ghost_label("Copy phrase", Message::CopyPhrase),
                    action_label("I've stored it", Message::WelcomeStored),
                ]
                .spacing(8),
            );
            body = body.push(
                text("Store it offline. Settings can show it again.")
                    .size(13)
                    .color(ink.muted),
            );
        }
        container(body)
            .width(Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .into()
    }

    fn guide_screen(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let width = (self.size.width - 64.0).clamp(280.0, 560.0);
        container(
            column![
                text("Before the notes").font(serif()).size(40),
                text("Settings holds the rest. Nothing on this page is sent.")
                    .size(16)
                    .color(ink.muted),
                guide_line(
                    "Passphrase",
                    "Optional. Settings, then Devices. It locks the key file on this computer. It is not the recovery phrase, and this welcome does not ask for one.",
                ),
                guide_line(
                    "Devices",
                    "This computer already has a name. Another computer can pair later. Revoke stops new notes from a computer you no longer use. Notes it already wrote stay.",
                ),
                guide_line(
                    "Recovery",
                    "The phrase is in Settings if you need to copy it again. Restoring on a new computer does not bring back old device keys.",
                ),
                guide_line(
                    "Your notes",
                    "Export a copy into this folder, or erase this computer's notebook. Erasing does not reach another computer.",
                ),
                guide_line(
                    "People",
                    "What you call someone stays on this computer. An invite still has no name on it.",
                ),
                guide_line(
                    "Chat",
                    "Either you paste their link and start, or you send yours and wait. The words can sync. A file stays on this computer.",
                ),
                action_label("Open the notebook", Message::WelcomeDone),
            ]
            .spacing(18)
            .width(width),
        )
        .width(Fill)
        .align_x(iced::alignment::Horizontal::Center)
        .into()
    }

    fn settings_sheet(&self) -> Element<'_, Message> {
        let width = (self.size.width - self.gutter_x() * 2.0 - 24.0).clamp(280.0, 640.0);
        container(
            vscroll(
                container(
                    column![
                        Self::sheet_title("Settings"),
                        self.you_section(),
                        self.appearance_section(),
                        self.devices_sheet(),
                        self.recovery_sheet(),
                        self.people_section(),
                        self.backup_section(),
                        self.data_section(),
                        Self::keys_section(),
                    ]
                    .spacing(16)
                    .width(width)
                    .padding(iced::Padding {
                        top: 4.0,
                        right: 0.0,
                        bottom: 36.0,
                        left: 0.0,
                    }),
                )
                .width(Fill)
                .align_x(iced::alignment::Horizontal::Center),
            )
            .height(Fill),
        )
        .width(Fill)
        .height(Fill)
        .into()
    }

    /// The keys, since most of them have no button to find them by.
    fn keys_section() -> Element<'static, Message> {
        const KEYS: [(&str, &str); 17] = [
            ("Ctrl or Cmd + S", "Save the note now"),
            ("Ctrl or Cmd + N", "A new note"),
            ("Ctrl or Cmd + F", "Find in the note"),
            ("Ctrl or Cmd + .", "Write without distractions, or leave it"),
            ("Ctrl or Cmd + Z", "Undo"),
            ("Ctrl or Cmd + Shift + Z", "Redo"),
            ("Ctrl or Cmd + B", "Bold"),
            ("Ctrl or Cmd + I", "Italic"),
            ("Ctrl or Cmd + K", "Link"),
            ("Ctrl or Cmd + E", "Code"),
            ("Ctrl or Cmd + Shift + X", "Strike"),
            ("Ctrl or Cmd + Shift + H", "Highlight"),
            ("Tab", "Nest a list item, or every one you have selected"),
            ("Shift + Tab", "Bring a list item out one level"),
            (
                "Enter on an empty item",
                "End the list, or step a nested item out",
            ),
            (
                "Ctrl or Cmd + Shift + D",
                "Add the underlined word to your dictionary",
            ),
            ("Enter in the title", "Start writing"),
        ];
        let mut rows = column![].spacing(8);
        for (keys, does) in KEYS {
            rows = rows.push(
                row![
                    text(keys).size(14).width(210),
                    text(does).size(14).width(Fill),
                ]
                .spacing(12),
            );
        }
        settings_card("Keys", rows.into())
    }

    fn you_section(&self) -> Element<'_, Message> {
        let ink = self.ink();
        settings_card(
            "You",
            column![
                text("Your name stays on this computer. It is not sent to anyone.")
                    .size(14)
                    .color(ink.muted),
                text_input("Your name", &self.name_draft)
                    .on_input(Message::NameDraft)
                    .padding(12)
                    .style(field_style),
                text("This computer").size(13).color(ink.muted),
                text(self.this_device_label()).size(16),
                text("Set when this folder was created.")
                    .size(13)
                    .color(ink.muted),
            ]
            .spacing(10)
            .into(),
        )
    }

    fn appearance_section(&self) -> Element<'_, Message> {
        let ink = self.ink();
        settings_card(
            "Appearance",
            column![
                text("Light, dark, or the same as this computer.")
                    .size(14)
                    .color(ink.muted),
                self.appearance_row(),
            ]
            .spacing(12)
            .into(),
        )
    }

    fn appearance_row(&self) -> Element<'_, Message> {
        row![
            choice_button(
                "Light",
                self.appearance == profile::Appearance::Light,
                Message::PickAppearance(profile::Appearance::Light),
            ),
            choice_button(
                "Dark",
                self.appearance == profile::Appearance::Dark,
                Message::PickAppearance(profile::Appearance::Dark),
            ),
            choice_button(
                "System",
                self.appearance == profile::Appearance::System,
                Message::PickAppearance(profile::Appearance::System),
            ),
        ]
        .spacing(8)
        .width(Fill)
        .into()
    }

    fn people_section(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let mut people = column![].spacing(16);
        if self.chats.is_empty() {
            people = people.push(
                text("No one yet. Invite someone from Chat.")
                    .size(14)
                    .color(ink.muted),
            );
        }
        for chat in &self.chats {
            let tail = if chat.other.is_empty() {
                "Key unknown".to_owned()
            } else {
                format!("Key {}", chat.other)
            };
            let draft = self.aliases.get(&chat.id).cloned().unwrap_or_default();
            let id = chat.id.clone();
            people = people.push(
                column![
                    text(tail).size(12).color(ink.muted),
                    text_input("What do you call them?", &draft)
                        .on_input(move |text| Message::AliasDraft(id.clone(), text))
                        .padding(12)
                        .style(field_style),
                ]
                .spacing(6),
            );
        }
        settings_card(
            "People",
            column![
                text("Names you type here stay on this computer. The short key stays under the name, because an invite still cannot carry one.")
                    .size(14)
                    .color(ink.muted),
                people,
            ]
            .spacing(14)
            .into(),
        )
    }

    /// Where the notes go if this computer's disk does not survive.
    fn backup_section(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let warn = Color::from_rgb8(184, 78, 62);
        let wrapped = ambient_cli::folder_key_is_wrapped(&self.dir);
        let last = if self.backup_at == 0 {
            text("No backup yet.").size(14).color(warn)
        } else {
            let when = clock::when(Some(self.backup_at), &clock::load_tz(), clock::now_ms());
            text(format!("Last backup: {}, {}.", when.day, when.time))
                .size(14)
                .color(ink.muted)
        };
        let key = if wrapped {
            text("The key file is locked with your passphrase, so the copy is locked too.")
                .size(14)
                .color(ink.muted)
        } else {
            text("The key file is not locked with a passphrase. Anyone who gets the copy can read your notes. Lock the key file, above, first.")
                .size(14)
                .color(warn)
        };
        let default = profile::default_backup_dir().display().to_string();
        settings_card(
            "Back up",
            column![
                text("Your notes are only on this computer until you copy them somewhere else, such as a USB drive or a folder that syncs. The copy has everything: the sealed notes, your chats, and the key file. It leaves out the readable Markdown in the export folder, which has no lock.")
                    .size(14)
                    .color(ink.muted),
                key,
                text_input(&default, &self.backup_to)
                    .on_input(Message::BackupTo)
                    .on_submit(Message::BackUp)
                    .padding(12)
                    .style(field_style),
                action_label(
                    if self.backup_busy { "Backing up" } else { "Back up now" },
                    Message::BackUp,
                ),
                last,
                text(format!("To restore, close the app, put the copy where this notebook is, {}, and open it again.", self.dir.display()))
                    .size(13)
                    .color(ink.muted),
            ]
            .spacing(12)
            .into(),
        )
    }

    fn data_section(&self) -> Element<'_, Message> {
        let ink = self.ink();
        settings_card(
            "Your notes",
            column![
                text("Save a copy of your notes into this folder, or erase this computer's copy. Erasing does not reach another computer.")
                    .size(14)
                    .color(ink.muted),
                action_label("Export notes", Message::ExportNotes),
                text("Type delete, then erase. This cannot be undone.")
                    .size(13)
                    .color(ink.muted),
                text_input("delete", &self.wipe_draft)
                    .on_input(Message::WipeDraft)
                    .padding(12)
                    .style(field_style),
                danger_label("Erase this notebook", Message::ConfirmWipe),
            ]
            .spacing(12)
            .into(),
        )
    }

    fn chat_name(&self, chat: &ambient_cli::ConversationSummary) -> String {
        self.aliases
            .get(&chat.id)
            .filter(|name| !name.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| chat.label.clone())
    }

    fn this_device_label(&self) -> String {
        self.devices
            .iter()
            .find(|device| device.current)
            .map(|device| device.label.clone())
            .filter(|label| !label.is_empty())
            .or_else(|| {
                let draft = self.device_draft.trim();
                if draft.is_empty() {
                    None
                } else {
                    Some(draft.to_owned())
                }
            })
            .unwrap_or_else(|| "this computer".to_owned())
    }
}

#[derive(Clone, Copy)]
struct Ink {
    bg: Color,
    text: Color,
    muted: Color,
    accent: Color,
    on_accent: Color,
    surface: Color,
    hairline: Color,
    field: Color,
}

impl Ink {
    fn from_dark(dark: bool) -> Self {
        if dark {
            Self {
                bg: Color::from_rgb8(0x1C, 0x19, 0x17),
                text: Color::from_rgb8(0xED, 0xEA, 0xDE),
                muted: Color::from_rgb8(0xA8, 0xA2, 0x9E),
                accent: Color::from_rgb8(0x66, 0xA5, 0xE1),
                on_accent: Color::from_rgb8(0x1C, 0x19, 0x17),
                surface: Color::from_rgb8(0x26, 0x22, 0x20),
                hairline: Color::from_rgb8(0x3A, 0x34, 0x2E),
                field: Color::from_rgb8(0x14, 0x12, 0x10),
            }
        } else {
            Self {
                bg: Color::from_rgb8(0xED, 0xEA, 0xDE),
                text: Color::from_rgb8(0x1C, 0x19, 0x17),
                muted: Color::from_rgb8(0x6B, 0x65, 0x60),
                accent: Color::from_rgb8(0x16, 0x44, 0x6F),
                on_accent: Color::from_rgb8(0xED, 0xEA, 0xDE),
                surface: Color::from_rgb8(0xE4, 0xE0, 0xD2),
                hairline: Color::from_rgb8(0xD0, 0xCB, 0xBC),
                field: Color::from_rgb8(0xF7, 0xF5, 0xEE),
            }
        }
    }
}

fn content_arrived(detail: &str) -> bool {
    if detail == "Nothing new." {
        return false;
    }
    let mut saw = false;
    let mut arrived = false;
    for part in detail.split(',') {
        let part = part.trim();
        if let Some(count) = part.strip_suffix(" notes in") {
            saw = true;
            if count.trim() != "0" {
                arrived = true;
            }
        } else if let Some(count) = part.strip_suffix(" messages in") {
            saw = true;
            if count.trim() != "0" {
                arrived = true;
            }
        }
    }
    !saw || arrived
}

fn ink_of(theme: &Theme) -> Ink {
    Ink::from_dark(theme.palette().background.r < 0.5)
}

fn mix(from: Color, to: Color, t: f32) -> Color {
    Color {
        r: from.r + (to.r - from.r) * t,
        g: from.g + (to.g - from.g) * t,
        b: from.b + (to.b - from.b) * t,
        a: from.a + (to.a - from.a) * t,
    }
}

#[derive(Clone, Debug)]
struct SyncOutcome {
    detail: String,
    pending: bool,
    failed: bool,
    transport: bool,
}

/// Notes, the search index, and snapshots. Never take the chat lock first.
fn notes_lock() -> std::sync::MutexGuard<'static, ()> {
    static NOTES: Mutex<()> = Mutex::new(());
    NOTES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Conversation logs. Never take this and then the notes lock.
fn chat_lock() -> std::sync::MutexGuard<'static, ()> {
    static CHATS: Mutex<()> = Mutex::new(());
    CHATS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn bubbles(lines: Vec<ambient_cli::ChatMessage>) -> Vec<ChatBubble> {
    let tz = clock::load_tz();
    let now = clock::now_ms();
    lines
        .into_iter()
        .map(|line| {
            let when = clock::when(line.unix_ms, &tz, now);
            ChatBubble {
                text: line.text,
                time: when.time,
                day: when.day,
                mine: line.mine,
                cite: line.cite,
            }
        })
        .collect()
}

fn lane_messages() -> impl iced::futures::Stream<Item = Message> + Send {
    use iced::futures::StreamExt;
    lane::listen().map(Message::Lane)
}

fn wake_stream(wake: &Wake) -> iced::futures::stream::BoxStream<'static, Message> {
    use iced::futures::stream::StreamExt;
    let delay = match *wake {
        Wake::Pause(_) => Duration::from_millis(700),
        Wake::Search(_) => Duration::from_millis(200),
        Wake::Scroll(_) => SCROLL_SHOWN + Duration::from_millis(200),
    };
    let fired = *wake;
    iced::futures::stream::once(async move {
        tokio::time::sleep(delay).await;
        Message::Woke(fired)
    })
    .boxed()
}

fn sync_outcome(dir: &Path) -> SyncOutcome {
    let skip = lane::watched();
    match ambient_cli::sync_notebook_except(dir, skip.as_deref()) {
        Ok(detail) => SyncOutcome {
            pending: ambient_cli::has_unsent(dir).unwrap_or(true),
            detail,
            failed: false,
            transport: false,
        },
        Err(err) => {
            let detail = err.to_string();
            let transport = matches!(
                detail.as_str(),
                "no relay answered"
                    | "relay unreachable"
                    | "relay write failed"
                    | "relay read failed"
                    | "relay reply missing"
                    | "relay reply rejected"
                    | "unavailable"
                    | "too_large"
                    | "bad_request"
            );
            SyncOutcome {
                detail,
                pending: true,
                failed: true,
                transport,
            }
        }
    }
}

fn action_style(theme: &Theme, status: button::Status) -> button::Style {
    let ink = ink_of(theme);
    let background = match status {
        button::Status::Hovered => mix(ink.accent, Color::WHITE, 0.12),
        button::Status::Pressed => mix(ink.accent, Color::BLACK, 0.16),
        button::Status::Disabled => mix(ink.accent, ink.bg, 0.45),
        button::Status::Active => ink.accent,
    };
    button::Style {
        background: Some(iced::Background::Color(background)),
        text_color: ink.on_accent,
        border: iced::Border {
            radius: 8.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..Default::default()
    }
}

fn quiet_style(theme: &Theme, status: button::Status) -> button::Style {
    let ink = ink_of(theme);
    let background = match status {
        button::Status::Hovered => Some(iced::Background::Color(ink.surface)),
        button::Status::Pressed => Some(iced::Background::Color(mix(ink.accent, ink.bg, 0.85))),
        _ => None,
    };
    button::Style {
        background,
        text_color: ink.text,
        border: iced::Border {
            radius: 8.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..Default::default()
    }
}

fn ghost_style(theme: &Theme, status: button::Status) -> button::Style {
    let ink = ink_of(theme);
    let background = match status {
        button::Status::Hovered => Some(iced::Background::Color(ink.surface)),
        button::Status::Pressed => Some(iced::Background::Color(mix(ink.accent, ink.bg, 0.85))),
        _ => None,
    };
    button::Style {
        background,
        text_color: ink.text,
        border: iced::Border {
            radius: 8.0.into(),
            width: 1.0,
            color: ink.hairline,
        },
        ..Default::default()
    }
}

fn icon_style(theme: &Theme, status: button::Status) -> button::Style {
    let ink = ink_of(theme);
    let background = match status {
        button::Status::Hovered | button::Status::Pressed => {
            Some(iced::Background::Color(ink.surface))
        }
        _ => None,
    };
    button::Style {
        background,
        text_color: ink.text,
        border: iced::Border {
            radius: 8.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..Default::default()
    }
}

fn list_row(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let ink = ink_of(theme);
        let background = if selected {
            Some(iced::Background::Color(mix(ink.bg, ink.accent, 0.18)))
        } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
            Some(iced::Background::Color(mix(ink.surface, ink.bg, 0.35)))
        } else {
            None
        };
        button::Style {
            background,
            text_color: ink.text,
            border: iced::Border {
                radius: 8.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            ..Default::default()
        }
    }
}

fn segment_style(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        if active {
            action_style(theme, status)
        } else {
            let ink = ink_of(theme);
            let background = match status {
                button::Status::Hovered | button::Status::Pressed => {
                    Some(iced::Background::Color(mix(ink.bg, ink.surface, 0.6)))
                }
                _ => None,
            };
            button::Style {
                background,
                text_color: ink.muted,
                border: iced::Border {
                    radius: 8.0.into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                ..Default::default()
            }
        }
    }
}

fn field_style(theme: &Theme, _status: text_input::Status) -> text_input::Style {
    let ink = ink_of(theme);
    text_input::Style {
        background: iced::Background::Color(ink.field),
        border: iced::Border {
            radius: 8.0.into(),
            width: 1.0,
            color: ink.hairline,
        },
        icon: ink.muted,
        placeholder: ink.muted,
        value: ink.text,
        selection: mix(ink.accent, ink.bg, 0.35),
    }
}

fn title_style(theme: &Theme, _status: text_input::Status) -> text_input::Style {
    let ink = ink_of(theme);
    text_input::Style {
        background: iced::Background::Color(Color::TRANSPARENT),
        border: iced::Border {
            radius: 0.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        icon: ink.muted,
        placeholder: ink.muted,
        value: ink.text,
        selection: mix(ink.accent, ink.bg, 0.35),
    }
}

fn editor_style(theme: &Theme, _status: text_editor::Status) -> text_editor::Style {
    let ink = ink_of(theme);
    text_editor::Style {
        background: iced::Background::Color(Color::TRANSPARENT),
        border: iced::Border {
            radius: 0.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        placeholder: ink.muted,
        value: ink.text,
        selection: mix(ink.accent, ink.bg, 0.35),
    }
}

fn field_editor_style(theme: &Theme, _status: text_editor::Status) -> text_editor::Style {
    let ink = ink_of(theme);
    text_editor::Style {
        background: iced::Background::Color(ink.field),
        border: iced::Border {
            radius: 8.0.into(),
            width: 1.0,
            color: ink.hairline,
        },
        placeholder: ink.muted,
        value: ink.text,
        selection: mix(ink.accent, ink.bg, 0.35),
    }
}

fn surface_pane(theme: &Theme) -> container::Style {
    let ink = ink_of(theme);
    container::Style {
        background: Some(iced::Background::Color(ink.surface)),
        text_color: Some(ink.text),
        ..Default::default()
    }
}

fn is_tasks_note(note: &NoteSummary) -> bool {
    note.title == "Tasks" && !note.conflict
}

fn card_style(theme: &Theme) -> container::Style {
    let ink = ink_of(theme);
    container::Style {
        background: Some(iced::Background::Color(ink.surface)),
        text_color: Some(ink.text),
        border: iced::Border {
            radius: 12.0.into(),
            width: 1.0,
            color: ink.hairline,
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.18),
            offset: iced::Vector::new(0.0, 8.0),
            blur_radius: 18.0,
        },
        ..Default::default()
    }
}

fn section_card(theme: &Theme) -> container::Style {
    let ink = ink_of(theme);
    container::Style {
        background: Some(iced::Background::Color(ink.surface)),
        text_color: Some(ink.text),
        border: iced::Border {
            radius: 14.0.into(),
            width: 1.0,
            color: ink.hairline,
        },
        ..Default::default()
    }
}

fn field_well(theme: &Theme) -> container::Style {
    let ink = ink_of(theme);
    container::Style {
        background: Some(iced::Background::Color(ink.field)),
        text_color: Some(ink.text),
        border: iced::Border {
            radius: 10.0.into(),
            width: 1.0,
            color: ink.hairline,
        },
        ..Default::default()
    }
}

fn settings_card<'a>(title: &'static str, body: Element<'a, Message>) -> Element<'a, Message> {
    container(
        column![text(title).font(serif()).size(22), body]
            .spacing(14)
            .width(Fill),
    )
    .padding(20)
    .width(Fill)
    .style(section_card)
    .into()
}

fn bubble_style(theme: &Theme) -> container::Style {
    let ink = ink_of(theme);
    container::Style {
        background: Some(iced::Background::Color(ink.surface)),
        text_color: Some(ink.text),
        border: iced::Border {
            radius: 10.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..Default::default()
    }
}

fn mine_style(theme: &Theme) -> container::Style {
    let ink = ink_of(theme);
    container::Style {
        background: Some(iced::Background::Color(ink.accent)),
        text_color: Some(ink.on_accent),
        border: iced::Border {
            radius: 14.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..Default::default()
    }
}

fn theirs_style(theme: &Theme) -> container::Style {
    let mut style = bubble_style(theme);
    style.border.radius = 14.0.into();
    style
}

struct ChatBubble {
    text: String,
    time: String,
    day: String,
    mine: bool,
    cite: Option<String>,
}

fn file_chip(
    name: &str,
    path: Option<PathBuf>,
    video: bool,
    mine: bool,
) -> Element<'static, Message> {
    let label = if video {
        format!("Video · {name}")
    } else {
        name.to_owned()
    };
    if let Some(path) = path {
        button(text(format!("Open {label}")).size(14))
            .padding([8.0, 14.0])
            .on_press(Message::OpenPath(path))
            .style(move |theme, status| file_chip_style(theme, status, mine))
            .into()
    } else {
        text(format!("{label} · not on this computer"))
            .size(13)
            .style(move |theme: &Theme| {
                let ink = ink_of(theme);
                iced::widget::text::Style {
                    color: Some(if mine { ink.on_accent } else { ink.text }),
                }
            })
            .into()
    }
}

fn file_chip_style(theme: &Theme, status: button::Status, mine: bool) -> button::Style {
    let ink = ink_of(theme);
    if !mine {
        return ghost_style(theme, status);
    }
    let background = match status {
        button::Status::Hovered | button::Status::Pressed => Some(iced::Background::Color(Color {
            a: 0.16,
            ..ink.on_accent
        })),
        _ => Some(iced::Background::Color(Color {
            a: 0.08,
            ..ink.on_accent
        })),
    };
    button::Style {
        background,
        text_color: ink.on_accent,
        border: iced::Border {
            radius: 8.0.into(),
            width: 1.0,
            color: Color {
                a: 0.45,
                ..ink.on_accent
            },
        },
        ..Default::default()
    }
}

fn format_button(
    label: &'static str,
    hint_text: &'static str,
    kind: FormatKind,
) -> Element<'static, Message> {
    let face = button(text(label).size(13))
        .padding([6.0, 10.0])
        .on_press(Message::Format(kind))
        .style(ghost_style);
    hover_label(face, hint_text)
}

fn pill_button(
    label: &'static str,
    hint_text: &'static str,
    message: Message,
) -> Element<'static, Message> {
    let face = button(text(label).size(13))
        .padding([4.0, 8.0])
        .on_press(message)
        .style(ghost_style);
    tooltip(
        face,
        container(text(hint_text).size(12))
            .padding([4, 8])
            .style(card_style),
        tooltip::Position::Bottom,
    )
    .gap(6)
    .into()
}

fn pill_style(theme: &Theme) -> container::Style {
    let ink = ink_of(theme);
    container::Style {
        text_color: Some(ink.text),
        background: Some(iced::Background::Color(ink.surface)),
        border: iced::Border {
            radius: 16.0.into(),
            width: 1.0,
            color: ink.hairline,
        },
        ..Default::default()
    }
}

fn clean_tag(raw: &str) -> Option<String> {
    let tag = raw.trim().trim_start_matches('#').trim();
    if tag.is_empty() || tag.len() > 40 || tag.contains(['%', ',', '\n', '|']) {
        return None;
    }
    Some(tag.to_owned())
}

fn words_label(count: usize) -> String {
    if count == 1 {
        "1 word".to_owned()
    } else {
        format!("{count} words")
    }
}

fn format_caption(label: &'static str) -> Element<'static, Message> {
    text(label).size(11).into()
}

fn chat_words(words: String) -> Element<'static, Message> {
    text(words)
        .size(15)
        .wrapping(iced::advanced::text::Wrapping::WordOrGlyph)
        .into()
}

fn push_tasks(rows: &mut Vec<TaskRow>, id: &str, title: &str, body: &str) {
    for task in markdown::task_lines(body) {
        rows.push(TaskRow {
            note_id: id.to_owned(),
            note_title: title.to_owned(),
            line: task.line,
            done: task.done,
            text: task.text,
        });
    }
}

fn circle_button(
    glyph: &'static str,
    hint_text: &'static str,
    message: Message,
) -> Element<'static, Message> {
    let face = button(canvas(Glyph { content: glyph }).width(Fill).height(Fill))
        .width(40)
        .height(40)
        .padding(0)
        .on_press(message)
        .style(circle_style);
    hover_label(face, hint_text)
}

fn hover_label(
    face: impl Into<Element<'static, Message>>,
    label: &'static str,
) -> Element<'static, Message> {
    tooltip(
        face,
        container(text(label).size(12))
            .padding([4, 8])
            .style(card_style),
        tooltip::Position::Top,
    )
    .gap(6)
    .into()
}

struct Glyph {
    content: &'static str,
}

impl canvas::Program<Message> for Glyph {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        theme: &Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let ink = ink_of(theme);
        frame.fill_text(canvas::Text {
            content: self.content.to_owned(),
            position: iced::Point::new(bounds.width / 2.0, bounds.height / 2.0),
            color: ink.text,
            size: iced::Pixels(22.0),
            font: sans(),
            align_x: iced::alignment::Horizontal::Center.into(),
            align_y: iced::alignment::Vertical::Center,
            ..canvas::Text::default()
        });
        vec![frame.into_geometry()]
    }
}

fn circle_style(theme: &Theme, status: button::Status) -> button::Style {
    let ink = ink_of(theme);
    let background = match status {
        button::Status::Hovered => mix(ink.surface, ink.accent, 0.18),
        button::Status::Pressed => mix(ink.surface, ink.accent, 0.32),
        _ => ink.surface,
    };
    button::Style {
        background: Some(iced::Background::Color(background)),
        text_color: ink.text,
        border: iced::Border {
            radius: 20.0.into(),
            width: 1.0,
            color: ink.hairline,
        },
        ..Default::default()
    }
}

fn slim_bar() -> scrollable::Scrollbar {
    scrollable::Scrollbar::new()
        .width(4.0)
        .scroller_width(3.0)
        .margin(2.0)
}

fn vscroll<'a>(content: impl Into<Element<'a, Message>>) -> scrollable::Scrollable<'a, Message> {
    scrollable(content)
        .direction(scrollable::Direction::Vertical(slim_bar()))
        .style(scroll_style)
}

fn hscroll<'a>(content: impl Into<Element<'a, Message>>) -> scrollable::Scrollable<'a, Message> {
    scrollable(content)
        .direction(scrollable::Direction::Horizontal(slim_bar()))
        .style(scroll_style)
}

/// How long a scrollbar stays visible after the last scroll.
const SCROLL_SHOWN: Duration = Duration::from_millis(700);

/// Milliseconds since the start of the process at the last scroll, or `u64::MAX` for never.
static LAST_SCROLL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(u64::MAX);

fn since_start() -> Duration {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed()
}

/// Whether a wake to hide the scrollbar is already waiting. A burst of wheel ticks asks for one.
static SCROLL_WAKE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// A wheel tick. It costs one atomic store, so scrolling never goes through `update` or rebuilds
/// the view. Only the first tick of a burst sends a message, to start the wake that hides the bar.
fn scroll_seen() -> Option<Message> {
    note_scroll();
    if SCROLL_WAKE.swap(true, std::sync::atomic::Ordering::Relaxed) {
        None
    } else {
        Some(Message::Scrolled)
    }
}

fn note_scroll() {
    let at = u64::try_from(since_start().as_millis()).unwrap_or(u64::MAX - 1);
    LAST_SCROLL.store(at, std::sync::atomic::Ordering::Relaxed);
}

fn scrolled_recently() -> bool {
    let last = LAST_SCROLL.load(std::sync::atomic::Ordering::Relaxed);
    let now = u64::try_from(since_start().as_millis()).unwrap_or(u64::MAX - 1);
    last != u64::MAX
        && now.saturating_sub(last) < u64::try_from(SCROLL_SHOWN.as_millis()).unwrap_or(0)
}

/// A scrollbar is not drawn. It shows faintly for a moment after the pane under the pointer is
/// scrolled, and clearly while the pointer is on the bar itself, so it can still be dragged.
fn scroll_style(theme: &Theme, status: scrollable::Status) -> scrollable::Style {
    let mut style = scrollable::default(theme, status);
    let ink = ink_of(theme);
    let thumb = Color {
        a: thumb_alpha(status, scrolled_recently()),
        ..ink.muted
    };
    let rail = scrollable::Rail {
        background: None,
        border: iced::Border {
            radius: 2.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        scroller: scrollable::Scroller {
            background: iced::Background::Color(thumb),
            border: iced::Border {
                radius: 2.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
        },
    };
    style.vertical_rail = rail;
    style.horizontal_rail = rail;
    style.gap = None;
    style
}

/// Opacity of the thumb. `recent` is whether the last scroll was a moment ago.
fn thumb_alpha(status: scrollable::Status, recent: bool) -> f32 {
    if scroll_hot(status) {
        0.62
    } else if recent
        && matches!(
            status,
            scrollable::Status::Hovered { .. } | scrollable::Status::Dragged { .. }
        )
    {
        0.32
    } else {
        0.0
    }
}

fn scroll_hot(status: scrollable::Status) -> bool {
    matches!(
        status,
        scrollable::Status::Hovered {
            is_horizontal_scrollbar_hovered: true,
            ..
        } | scrollable::Status::Hovered {
            is_vertical_scrollbar_hovered: true,
            ..
        } | scrollable::Status::Dragged {
            is_horizontal_scrollbar_dragged: true,
            ..
        } | scrollable::Status::Dragged {
            is_vertical_scrollbar_dragged: true,
            ..
        }
    )
}

fn hairline() -> Element<'static, Message> {
    container(column![])
        .height(1)
        .width(Fill)
        .style(|theme| {
            let ink = ink_of(theme);
            container::Style {
                background: Some(iced::Background::Color(ink.hairline)),
                ..Default::default()
            }
        })
        .into()
}

fn hairline_v() -> Element<'static, Message> {
    container(column![])
        .width(1)
        .height(Fill)
        .style(|theme| {
            let ink = ink_of(theme);
            container::Style {
                background: Some(iced::Background::Color(ink.hairline)),
                ..Default::default()
            }
        })
        .into()
}

fn action_label(label: &str, message: Message) -> Element<'_, Message> {
    button(text(label).size(14))
        .padding([10.0, 16.0])
        .on_press(message)
        .style(action_style)
        .into()
}

fn danger_label(label: &'static str, message: Message) -> Element<'static, Message> {
    button(text(label).size(14))
        .padding([10.0, 16.0])
        .on_press(message)
        .style(danger_style)
        .into()
}

fn danger_style(theme: &Theme, status: button::Status) -> button::Style {
    let ink = ink_of(theme);
    let danger = Color::from_rgb8(184, 78, 62);
    let background = match status {
        button::Status::Hovered => mix(danger, Color::WHITE, 0.08),
        button::Status::Pressed => mix(danger, Color::BLACK, 0.16),
        button::Status::Disabled => mix(danger, ink.bg, 0.45),
        button::Status::Active => danger,
    };
    button::Style {
        background: Some(iced::Background::Color(background)),
        text_color: Color::from_rgb8(0xED, 0xEA, 0xDE),
        border: iced::Border {
            radius: 8.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..Default::default()
    }
}

fn action_fill(label: &str, message: Message) -> Element<'_, Message> {
    button(text(label).size(14).width(Fill))
        .width(Fill)
        .padding([8.0, 14.0])
        .on_press(message)
        .style(action_style)
        .into()
}

fn ghost_label(label: &str, message: Message) -> Element<'_, Message> {
    button(text(label).size(14))
        .padding([8.0, 14.0])
        .on_press(message)
        .style(ghost_style)
        .into()
}

fn ghost_label_owned(label: String, message: Message) -> Element<'static, Message> {
    button(text(label).size(14))
        .padding([8.0, 14.0])
        .on_press(message)
        .style(ghost_style)
        .into()
}

fn segment(label: &str, active: bool, message: Message) -> Element<'static, Message> {
    button(text(label.to_owned()).size(14).width(Fill))
        .width(Fill)
        .padding([8.0, 10.0])
        .on_press(message)
        .style(segment_style(active))
        .into()
}

fn chat_row(
    label: &str,
    preview: &str,
    time: &str,
    id: &str,
    selected: bool,
    fresh: bool,
) -> Element<'static, Message> {
    let label = label.to_owned();
    let preview = preview.to_owned();
    let time = time.to_owned();
    let id = id.to_owned();
    let mut head = row![text(label).size(15).width(Fill)].align_y(iced::Alignment::Center);
    if fresh {
        head = head.push(text("New").size(12).style(fresh_style));
    }
    if !time.is_empty() {
        head = head.push(text(time).size(12).style(muted_style));
    }
    let mut lines = column![head].spacing(2).width(Fill);
    if !preview.is_empty() {
        lines = lines.push(text(preview).size(12).style(muted_style));
    }
    button(lines)
        .width(Fill)
        .padding([8.0, 10.0])
        .on_press(Message::SelectChat(id))
        .style(list_row(selected))
        .into()
}

fn guide_line(title: &'static str, body: &'static str) -> Element<'static, Message> {
    column![
        text(title).font(serif()).size(22),
        text(body).size(14).style(muted_style),
    ]
    .spacing(4)
    .into()
}

fn chat_preview(text: &str) -> String {
    let line = text
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("")
        .trim();
    let mut out: String = line.chars().take(72).collect();
    if line.chars().count() > 72 {
        out.push_str("...");
    }
    out
}

fn edit_burst(action: &text_editor::Action) -> Option<Burst> {
    match action {
        text_editor::Action::Edit(text_editor::Edit::Insert(_)) => Some(Burst::Type),
        text_editor::Action::Edit(text_editor::Edit::Backspace | text_editor::Edit::Delete) => {
            Some(Burst::Erase)
        }
        text_editor::Action::Edit(
            text_editor::Edit::Paste(_)
            | text_editor::Edit::Enter
            | text_editor::Edit::Indent
            | text_editor::Edit::Unindent,
        ) => Some(Burst::Break),
        _ => None,
    }
}

fn fresh_style(theme: &Theme) -> iced::widget::text::Style {
    let ink = ink_of(theme);
    iced::widget::text::Style {
        color: Some(ink.accent),
    }
}

fn muted_style(theme: &Theme) -> iced::widget::text::Style {
    let ink = ink_of(theme);
    iced::widget::text::Style {
        color: Some(ink.muted),
    }
}

fn copy_style(theme: &Theme, status: button::Status, mine: bool) -> button::Style {
    let ink = ink_of(theme);
    let text_color = if mine {
        Color {
            a: 0.85,
            ..ink.on_accent
        }
    } else {
        ink.muted
    };
    let background = match status {
        button::Status::Hovered | button::Status::Pressed => Some(iced::Background::Color(Color {
            a: 0.12,
            ..if mine { ink.on_accent } else { ink.text }
        })),
        _ => None,
    };
    button::Style {
        background,
        text_color,
        border: iced::Border {
            radius: 6.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..Default::default()
    }
}

fn icon_button(handle: iced_image::Handle, message: Message) -> Element<'static, Message> {
    button(iced_image::Image::new(handle).width(22).height(22))
        .padding(6)
        .on_press(message)
        .style(icon_style)
        .into()
}

/// The height of the formatting bar, for sliding it. Close enough that it does not jump.
const FORMAT_BAR: f32 = 32.0;

/// Room kept above the writing in focus, for the small title and the buttons beside it.
const FOCUS_TOP: f32 = 34.0;

/// How long a panel takes to slide in or out.
const SLIDE: Duration = Duration::from_millis(220);

/// Something that slides between shown and hidden. It only moves for a moment after it is set,
/// and the window only redraws on a timer while it does.
struct Slide {
    open: bool,
    since: Instant,
}

impl Slide {
    /// Already in place, with nothing moving.
    fn settled(open: bool) -> Self {
        Self {
            open,
            since: Instant::now()
                .checked_sub(SLIDE.saturating_mul(2))
                .unwrap_or_else(Instant::now),
        }
    }

    fn set(&mut self, open: bool) {
        if self.open != open {
            self.open = open;
            self.since = Instant::now();
        }
    }

    /// Put it in place at once, with no slide.
    fn jump(&mut self, open: bool) {
        *self = Self::settled(open);
    }

    fn moving(&self) -> bool {
        self.since.elapsed() < SLIDE
    }

    /// 0 when hidden and 1 when shown, easing out.
    fn shown(&self) -> f32 {
        let t = (self.since.elapsed().as_secs_f32() / SLIDE.as_secs_f32()).clamp(0.0, 1.0);
        let eased = 1.0 - (1.0 - t).powi(3);
        if self.open { eased } else { 1.0 - eased }
    }
}

/// A size a person can read: `812 bytes`, `340 KB`, `3.4 MB`, `1.2 GB`.
fn size_label(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    let tenths = |unit: u64| bytes.saturating_mul(10) / unit;
    if bytes < KB {
        format!("{bytes} bytes")
    } else if bytes < MB {
        format!("{} KB", bytes / KB)
    } else if bytes < GB {
        format!("{}.{} MB", tenths(MB) / 10, tenths(MB) % 10)
    } else {
        format!("{}.{} GB", tenths(GB) / 10, tenths(GB) % 10)
    }
}

/// A key the editor did not take. It is put into the page here, so it is not lost.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Typed {
    Text(String),
    Enter,
    Backspace,
    Delete,
}

/// What a key that no widget handled should type, if anything.
///
/// The live editor is a widget whose focus lives in its place in the tree. When it moves, as it
/// does after Enter or when a space wraps a word onto another row, the new one starts without
/// focus, and the app asks for focus again a moment later. A key pressed in that moment is
/// ignored by the editor and is gone. The window sees every key an editor ignored, so it can
/// type them. A key the editor did take is captured, and is not seen here, so nothing is typed
/// twice. A key with Ctrl, Cmd, Alt, or the Super key is a shortcut and is left alone.
fn ignored_key(
    key: &Key,
    text: Option<&str>,
    modifiers: iced::keyboard::Modifiers,
) -> Option<Typed> {
    use iced::keyboard::key::Named;
    if modifiers.control() || modifiers.command() || modifiers.alt() || modifiers.logo() {
        return None;
    }
    match key.as_ref() {
        Key::Named(Named::Enter) => Some(Typed::Enter),
        Key::Named(Named::Backspace) => Some(Typed::Backspace),
        Key::Named(Named::Delete) => Some(Typed::Delete),
        Key::Named(Named::Space) => Some(Typed::Text(" ".to_owned())),
        _ => {
            let typed: String = text?.chars().filter(|ch| !ch.is_control()).collect();
            (!typed.is_empty()).then_some(Typed::Text(typed))
        }
    }
}

fn focus_named(name: &'static str) -> Task<Message> {
    iced::widget::operation::focus(iced::widget::Id::new(name))
}

#[derive(Clone, Copy)]
enum Mini {
    Search,
    Pencil,
    Trash,
    Up,
    Down,
    Focus,
    Gear,
}

fn mini_button(
    handle: iced_image::Handle,
    hint_text: &'static str,
    message: Message,
) -> Element<'static, Message> {
    let face = button(iced_image::Image::new(handle).width(16).height(16))
        .padding(6)
        .on_press(message)
        .style(icon_style);
    hover_label(face, hint_text)
}

fn mini_handle(kind: Mini, dark: bool) -> iced_image::Handle {
    use std::sync::OnceLock;
    static HANDLES: OnceLock<[iced_image::Handle; 14]> = OnceLock::new();
    let handles = HANDLES.get_or_init(|| {
        let ink = [0xED, 0xEA, 0xDE];
        let day = [0x1C, 0x19, 0x17];
        [
            iced_image::Handle::from_bytes(icon_png(ink, search_ink)),
            iced_image::Handle::from_bytes(icon_png(day, search_ink)),
            iced_image::Handle::from_bytes(icon_png(ink, pencil_ink)),
            iced_image::Handle::from_bytes(icon_png(day, pencil_ink)),
            iced_image::Handle::from_bytes(icon_png(ink, trash_ink)),
            iced_image::Handle::from_bytes(icon_png(day, trash_ink)),
            iced_image::Handle::from_bytes(icon_png(ink, up_ink)),
            iced_image::Handle::from_bytes(icon_png(day, up_ink)),
            iced_image::Handle::from_bytes(icon_png(ink, down_ink)),
            iced_image::Handle::from_bytes(icon_png(day, down_ink)),
            iced_image::Handle::from_bytes(icon_png(ink, focus_ink)),
            iced_image::Handle::from_bytes(icon_png(day, focus_ink)),
            iced_image::Handle::from_bytes(icon_png(ink, gear_ink)),
            iced_image::Handle::from_bytes(icon_png(day, gear_ink)),
        ]
    });
    let index = match kind {
        Mini::Search => 0,
        Mini::Pencil => 2,
        Mini::Trash => 4,
        Mini::Up => 6,
        Mini::Down => 8,
        Mini::Focus => 10,
        Mini::Gear => 12,
    } + usize::from(!dark);
    handles
        .get(index)
        .cloned()
        .unwrap_or_else(|| iced_image::Handle::from_bytes(Vec::new()))
}

fn icon_png(color: [u8; 3], ink: fn(f64, f64) -> bool) -> Vec<u8> {
    let size = 64_u32;
    let mut pixels = vec![0_u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let dx = f64::from(x) - 31.5;
            let dy = f64::from(y) - 31.5;
            if !ink(dx, dy) {
                continue;
            }
            let index = ((y * size + x) * 4) as usize;
            if let Some(pixel) = pixels.get_mut(index..index.saturating_add(4)) {
                pixel.copy_from_slice(&[color[0], color[1], color[2], 255]);
            }
        }
    }
    encode_png(&pixels, size)
}

fn search_ink(dx: f64, dy: f64) -> bool {
    let ox = dx + 6.0;
    let oy = dy + 6.0;
    let radius = ox.hypot(oy);
    if (11.0..15.0).contains(&radius) {
        return true;
    }
    dx > 4.0 && dy > 4.0 && dx < 20.0 && dy < 20.0 && (dx - dy).abs() < 2.8 && radius > 14.0
}

fn pencil_ink(dx: f64, dy: f64) -> bool {
    let u = (dx + dy) * std::f64::consts::FRAC_1_SQRT_2;
    let v = (dy - dx) * std::f64::consts::FRAC_1_SQRT_2;
    if (-16.0..8.0).contains(&u) && v.abs() < 3.0 {
        return true;
    }
    (8.0..16.0).contains(&u) && v.abs() < (16.0 - u) * 0.55
}

fn trash_ink(dx: f64, dy: f64) -> bool {
    let x = dx.abs();
    let handle = (-21.0..-16.5).contains(&dy) && x < 5.0 && (dy > -18.2 || x > 3.2);
    let lid = (-16.5..-13.0).contains(&dy) && x < 16.0;
    let side = (-12.0..15.0).contains(&dy) && (x - 11.0).abs() < 1.7;
    let bottom = (13.0..16.5).contains(&dy) && x < 12.0;
    handle || lid || side || bottom
}

/// Four corner brackets around a dot: a viewfinder, for writing with nothing else in view.
fn focus_ink(dx: f64, dy: f64) -> bool {
    let x = dx.abs();
    let y = dy.abs();
    let upright = (16.0..20.5).contains(&x) && (8.0..=20.5).contains(&y);
    let across = (16.0..20.5).contains(&y) && (8.0..=20.5).contains(&x);
    upright || across || x.hypot(y) < 4.5
}

/// A ring with eight teeth and a hole: settings.
fn gear_ink(dx: f64, dy: f64) -> bool {
    let radius = dx.hypot(dy);
    if !(6.5..=19.5).contains(&radius) {
        return false;
    }
    let tooth = (dy.atan2(dx) * 8.0).cos() > -0.1;
    radius <= 14.5 || (tooth && radius <= 19.5)
}

fn up_ink(dx: f64, dy: f64) -> bool {
    chevron_ink(true, dx, dy)
}

fn down_ink(dx: f64, dy: f64) -> bool {
    chevron_ink(false, dx, dy)
}

fn chevron_ink(up: bool, dx: f64, dy: f64) -> bool {
    let y = if up { dy + 2.0 } else { -dy + 2.0 };
    let x = dx.abs();
    let target = (y + 12.0) * (14.0 / 18.0);
    (-12.0..8.0).contains(&y) && (x - target).abs() < 2.4 && x < 16.0
}

fn gear_handle(dark: bool) -> iced_image::Handle {
    use std::sync::OnceLock;
    static HANDLES: OnceLock<[iced_image::Handle; 2]> = OnceLock::new();
    let handles = HANDLES.get_or_init(|| {
        [
            iced_image::Handle::from_bytes(gear_png([0x66, 0xA5, 0xE1])),
            iced_image::Handle::from_bytes(gear_png([0x16, 0x44, 0x6F])),
        ]
    });
    if dark {
        handles[0].clone()
    } else {
        handles[1].clone()
    }
}

fn gear_png(color: [u8; 3]) -> Vec<u8> {
    let Ok(icon) = image::load_from_memory(include_bytes!("../icons/settings.png")) else {
        return Vec::new();
    };
    let icon = icon.to_rgba8();
    let (width, height) = icon.dimensions();
    let mut pixels = icon.into_raw();
    for chunk in pixels.chunks_exact_mut(4) {
        let Ok(pixel) = <&mut [u8; 4]>::try_from(chunk) else {
            continue;
        };
        if pixel[3] == 0 {
            continue;
        }
        pixel[0] = color[0];
        pixel[1] = color[1];
        pixel[2] = color[2];
    }
    let mut buf = Vec::new();
    let encoder = ::image::codecs::png::PngEncoder::new(Cursor::new(&mut buf));
    let _ = encoder.write_image(&pixels, width, height, ::image::ExtendedColorType::Rgba8);
    buf
}

fn mode_handle(dark: bool) -> iced_image::Handle {
    use std::sync::OnceLock;
    static HANDLES: OnceLock<[iced_image::Handle; 2]> = OnceLock::new();
    let handles = HANDLES.get_or_init(|| {
        [
            iced_image::Handle::from_bytes(mode_png(true, [0x66, 0xA5, 0xE1])),
            iced_image::Handle::from_bytes(mode_png(false, [0x16, 0x44, 0x6F])),
        ]
    });
    if dark {
        handles[0].clone()
    } else {
        handles[1].clone()
    }
}

fn mode_png(sun: bool, color: [u8; 3]) -> Vec<u8> {
    let size = 64_u32;
    let mut pixels = vec![0_u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let dx = f64::from(x) - 31.5;
            let dy = f64::from(y) - 31.5;
            if !mode_ink(sun, dx, dy) {
                continue;
            }
            let index = ((y * size + x) * 4) as usize;
            if let Some(pixel) = pixels.get_mut(index..index.saturating_add(4)) {
                pixel.copy_from_slice(&[color[0], color[1], color[2], 255]);
            }
        }
    }
    encode_png(&pixels, size)
}

fn mode_ink(sun: bool, dx: f64, dy: f64) -> bool {
    let radius = dx.hypot(dy);
    if sun {
        if radius < 9.5 {
            return true;
        }
        if !(13.5..22.0).contains(&radius) {
            return false;
        }
        let angle = dy.atan2(dx);
        let step = std::f64::consts::FRAC_PI_4;
        let nearest = (angle / step).round() * step;
        (angle - nearest).abs() < 0.18
    } else {
        let bite = (dx - 9.0).hypot(dy - 1.0);
        radius < 16.0 && bite > 13.5
    }
}

fn encode_png(pixels: &[u8], size: u32) -> Vec<u8> {
    let mut buf = Vec::new();
    let encoder = ::image::codecs::png::PngEncoder::new(Cursor::new(&mut buf));
    let _ = encoder.write_image(pixels, size, size, ::image::ExtendedColorType::Rgba8);
    buf
}

fn globe_handle(color: [u8; 3]) -> iced_image::Handle {
    use std::sync::OnceLock;
    static HANDLES: OnceLock<[iced_image::Handle; 3]> = OnceLock::new();
    let handles = HANDLES.get_or_init(|| {
        [
            iced_image::Handle::from_bytes(globe_png([140, 133, 122])),
            iced_image::Handle::from_bytes(globe_png([184, 78, 62])),
            iced_image::Handle::from_bytes(globe_png([46, 140, 96])),
        ]
    });
    if color == [184, 78, 62] {
        handles[1].clone()
    } else if color == [46, 140, 96] {
        handles[2].clone()
    } else {
        handles[0].clone()
    }
}

fn globe_png(color: [u8; 3]) -> Vec<u8> {
    let size = 64_u32;
    let mut pixels = vec![0_u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let dx = f64::from(x) - 31.5;
            let dy = f64::from(y) - 31.5;
            let radius = dx.hypot(dy);
            if radius > 28.0 {
                continue;
            }
            let ring = radius > 25.5;
            let equator = dy.abs() < 1.3;
            let meridian = dx.abs() < 1.3;
            let ellipse = ((dx / 16.0).powi(2) + (dy / 28.0).powi(2) - 1.0).abs() < 0.12;
            let ink = ring || equator || meridian || ellipse;
            let alpha = if ink { 255 } else { 48 };
            let index = ((y * size + x) * 4) as usize;
            if let Some(pixel) = pixels.get_mut(index..index.saturating_add(4)) {
                pixel.copy_from_slice(&[color[0], color[1], color[2], alpha]);
            }
        }
    }
    let mut buf = Vec::new();
    let encoder = ::image::codecs::png::PngEncoder::new(Cursor::new(&mut buf));
    let _ = encoder.write_image(&pixels, size, size, ::image::ExtendedColorType::Rgba8);
    buf
}

fn sibling(dir: &Path, suffix: &str) -> PathBuf {
    let name = dir
        .file_name()
        .and_then(|os| os.to_str())
        .unwrap_or("notebook");
    match dir.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(format!("{name}{suffix}")),
        _ => PathBuf::from(format!("{name}{suffix}")),
    }
}

fn open_task(dir: PathBuf) -> Task<Message> {
    Task::perform(
        async move {
            let (sender, receiver) = iced::futures::channel::oneshot::channel();
            std::thread::spawn(move || {
                let result = (|| {
                    let _notes = notes_lock();
                    let _chat = chat_lock();
                    let book = Notebook::open(&dir).map_err(|err| err.to_string())?;
                    let files = lane::files_of(&book);
                    lane::park_side(lane::compute(&files, &lane::OpenSnap::default()));
                    lane::park_inbox(lane::inbox_of(&dir));
                    park_notebook(book);
                    Ok(())
                })();
                let _ = sender.send(result);
            });
            receiver
                .await
                .unwrap_or_else(|_| Err("could not open this folder".to_owned()))
        },
        Message::Opened,
    )
}

fn parked_notebook() -> &'static Mutex<Option<Notebook>> {
    static PARK: Mutex<Option<Notebook>> = Mutex::new(None);
    &PARK
}

fn park_notebook(book: Notebook) {
    *parked_notebook()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(book);
}

fn take_notebook() -> Option<Notebook> {
    parked_notebook()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
}

fn save_task(
    dir: PathBuf,
    id: Option<String>,
    title: String,
    body: String,
    token: u64,
) -> Task<Message> {
    Task::perform(
        async move {
            let (sender, receiver) = iced::futures::channel::oneshot::channel();
            std::thread::spawn(move || {
                let result = write_snapshot(&dir, id.as_deref(), &title, &body);
                let _ = sender.send((result, title, body));
            });
            let (result, title, body) = receiver.await.unwrap_or_else(|_| {
                (
                    Err("Save stopped before it finished.".to_owned()),
                    String::new(),
                    String::new(),
                )
            });
            (token, result, title, body)
        },
        |(token, result, title, body)| Message::Saved(token, result, title, body),
    )
}

fn write_snapshot(
    dir: &Path,
    id: Option<&str>,
    title: &str,
    body: &str,
) -> Result<(String, Vec<NoteSummary>), String> {
    let _notes = notes_lock();
    let book = Notebook::open(dir).map_err(|err| err.to_string())?;
    let saved = match id {
        Some(id) => {
            book.edit(id, title, body).map_err(|err| err.to_string())?;
            id.to_owned()
        }
        None => book.add(title, body).map_err(|err| err.to_string())?,
    };
    let notes = book.list().map_err(|err| err.to_string())?;
    lane::publish_saved(book);
    Ok((saved, notes))
}

fn write_chapter(
    dir: &Path,
    current: Option<&(Option<String>, String, String)>,
    title: &str,
) -> Result<MadeChapter, String> {
    let _notes = notes_lock();
    let book = Notebook::open(dir).map_err(|err| err.to_string())?;
    let saved = if let Some((id, note_title, note_body)) = current {
        let id = match id {
            Some(id) => {
                book.edit(id, note_title, note_body)
                    .map_err(|err| err.to_string())?;
                id.clone()
            }
            None => book
                .add(note_title, note_body)
                .map_err(|err| err.to_string())?,
        };
        Some(SavedDraft {
            id,
            title: note_title.clone(),
            body: note_body.clone(),
        })
    } else {
        None
    };
    let id = book.add(title, "").map_err(|err| err.to_string())?;
    let notes = book.list().map_err(|err| err.to_string())?;
    lane::publish_saved(book);
    Ok(MadeChapter { notes, id, saved })
}

fn choice_button(label: &str, active: bool, message: Message) -> Element<'_, Message> {
    button(
        text(label)
            .size(14)
            .width(Fill)
            .align_x(iced::alignment::Horizontal::Center),
    )
    .width(Fill)
    .padding([10.0, 12.0])
    .on_press(message)
    .style(segment_style(active))
    .into()
}

fn binding(press: text_editor::KeyPress) -> Option<text_editor::Binding<Message>> {
    let character = match press.key.as_ref() {
        Key::Character(key) => Some(key.to_owned()),
        _ => None,
    };
    let command = press.modifiers.control() || press.modifiers.command();
    let shift = press.modifiers.shift();
    if !command
        && !press.modifiers.alt()
        && matches!(
            press.key.as_ref(),
            Key::Named(iced::keyboard::key::Named::Tab)
        )
    {
        return Some(text_editor::Binding::Custom(Message::ShiftItems {
            back: shift,
        }));
    }
    match (character.as_deref(), command, shift) {
        (Some("z" | "Z"), true, true) | (Some("y" | "Y"), true, _) => {
            Some(text_editor::Binding::Custom(Message::Redo))
        }
        // Formatting, on the same marks the buttons use. Shift keeps the ones a browser or a
        // word processor already gives to something else out of the way.
        (Some("b" | "B"), true, false) => Some(format_binding(FormatKind::Bold)),
        (Some("i" | "I"), true, false) => Some(format_binding(FormatKind::Italic)),
        (Some("k" | "K"), true, false) => Some(format_binding(FormatKind::Link)),
        (Some("e" | "E"), true, false) => Some(format_binding(FormatKind::Code)),
        (Some("x" | "X"), true, true) => Some(format_binding(FormatKind::Strike)),
        (Some("h" | "H"), true, true) => Some(format_binding(FormatKind::Highlight)),
        (Some("d" | "D"), true, true) => Some(text_editor::Binding::Custom(Message::AddWord)),
        (Some("z" | "Z"), true, false) => Some(text_editor::Binding::Custom(Message::Undo)),
        (Some("f" | "F"), true, _) => Some(text_editor::Binding::Custom(Message::FindOpen)),
        (Some("s"), true, _) => Some(text_editor::Binding::Custom(Message::Save)),
        (Some("n"), true, _) => Some(text_editor::Binding::Custom(Message::NewNote)),
        (Some("c"), true, _) => Some(text_editor::Binding::Custom(Message::CopyNote)),
        (Some("x"), true, _) => Some(text_editor::Binding::Custom(Message::CutNote)),
        _ => text_editor::Binding::from_key_press(press),
    }
}

fn format_binding(kind: FormatKind) -> text_editor::Binding<Message> {
    text_editor::Binding::Custom(Message::Format(kind))
}

fn composer_binding(press: text_editor::KeyPress) -> Option<text_editor::Binding<Message>> {
    let enter = matches!(
        press.key.as_ref(),
        Key::Named(iced::keyboard::key::Named::Enter)
    );
    if enter && press.modifiers.shift() {
        return Some(text_editor::Binding::Enter);
    }
    if enter && !press.modifiers.command() && !press.modifiers.control() && !press.modifiers.alt() {
        return Some(text_editor::Binding::Custom(Message::SendChat));
    }
    text_editor::Binding::from_key_press(press)
}

fn generate_qr(data: &str) -> Result<Vec<u8>, String> {
    if data.is_empty() {
        return Err("no data for QR".to_owned());
    }
    let code = QrCode::new(data.as_bytes()).map_err(|_| "could not make QR")?;
    let img = code
        .render::<::image::Luma<u8>>()
        .min_dimensions(200, 200)
        .build();
    let mut buf = Vec::new();
    let mut cursor = Cursor::new(&mut buf);
    let encoder = ::image::codecs::png::PngEncoder::new(&mut cursor);
    encoder
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            ::image::ExtendedColorType::L8,
        )
        .map_err(|_| "could not encode QR")?;
    Ok(buf)
}

fn anchored<'a>(
    anchor: impl Into<Element<'a, Message>>,
    card: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    Anchored {
        anchor: anchor.into(),
        card,
    }
    .into()
}

struct Anchored<'a> {
    anchor: Element<'a, Message>,
    card: Option<Element<'a, Message>>,
}

impl iced::advanced::widget::Widget<Message, Theme, iced::Renderer> for Anchored<'_> {
    fn size(&self) -> iced::Size<iced::Length> {
        self.anchor.as_widget().size()
    }

    fn size_hint(&self) -> iced::Size<iced::Length> {
        self.anchor.as_widget().size_hint()
    }

    fn children(&self) -> Vec<iced::advanced::widget::Tree> {
        let mut children = vec![iced::advanced::widget::Tree::new(&self.anchor)];
        if let Some(card) = &self.card {
            children.push(iced::advanced::widget::Tree::new(card));
        }
        children
    }

    fn diff(&self, tree: &mut iced::advanced::widget::Tree) {
        match &self.card {
            Some(card) => tree.diff_children(&[self.anchor.as_widget(), card.as_widget()]),
            None => tree.diff_children(&[self.anchor.as_widget()]),
        }
    }

    fn layout(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        renderer: &iced::Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        let Some(child) = tree.children.first_mut() else {
            return iced::advanced::layout::Node::new(iced::Size::ZERO);
        };
        self.anchor.as_widget_mut().layout(child, renderer, limits)
    }

    fn update(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
        viewport: &iced::Rectangle,
    ) {
        let Some(child) = tree.children.first_mut() else {
            return;
        };
        self.anchor.as_widget_mut().update(
            child, event, layout, cursor, renderer, clipboard, shell, viewport,
        );
    }

    fn draw(
        &self,
        tree: &iced::advanced::widget::Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
    ) {
        let Some(child) = tree.children.first() else {
            return;
        };
        self.anchor
            .as_widget()
            .draw(child, renderer, theme, style, layout, cursor, viewport);
    }

    fn mouse_interaction(
        &self,
        tree: &iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
        renderer: &iced::Renderer,
    ) -> iced::advanced::mouse::Interaction {
        let Some(child) = tree.children.first() else {
            return iced::advanced::mouse::Interaction::None;
        };
        self.anchor
            .as_widget()
            .mouse_interaction(child, layout, cursor, viewport, renderer)
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &iced::Rectangle,
        translation: iced::Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, Message, Theme, iced::Renderer>> {
        let (anchor_tree, card_trees) = tree.children.split_first_mut()?;
        let anchor_overlay = self.anchor.as_widget_mut().overlay(
            anchor_tree,
            layout,
            renderer,
            viewport,
            translation,
        );
        let Some(card) = self.card.as_mut() else {
            return anchor_overlay;
        };
        let Some(card_tree) = card_trees.first_mut() else {
            return anchor_overlay;
        };
        let mut anchor = layout.bounds();
        anchor.x += translation.x;
        anchor.y += translation.y;
        let card_overlay = iced::advanced::overlay::Element::new(Box::new(CardOverlay {
            anchor,
            viewport: *viewport,
            card,
            tree: card_tree,
        }));
        let mut layers = Vec::new();
        if let Some(layer) = anchor_overlay {
            layers.push(layer);
        }
        layers.push(card_overlay);
        Some(iced::advanced::overlay::Group::with_children(layers).overlay())
    }
}

impl<'a> From<Anchored<'a>> for Element<'a, Message> {
    fn from(anchored: Anchored<'a>) -> Self {
        Element::new(anchored)
    }
}

struct CardOverlay<'a, 'b> {
    anchor: iced::Rectangle,
    viewport: iced::Rectangle,
    card: &'b mut Element<'a, Message>,
    tree: &'b mut iced::advanced::widget::Tree,
}

impl iced::advanced::Overlay<Message, Theme, iced::Renderer> for CardOverlay<'_, '_> {
    fn layout(
        &mut self,
        renderer: &iced::Renderer,
        bounds: iced::Size,
    ) -> iced::advanced::layout::Node {
        let node = self.card.as_widget_mut().layout(
            self.tree,
            renderer,
            &iced::advanced::layout::Limits::new(iced::Size::ZERO, bounds),
        );
        let size = node.size();
        let mut x = self.anchor.x + self.anchor.width - size.width;
        let mut y = self.anchor.y + self.anchor.height + 4.0;
        if x < 8.0 {
            x = 8.0;
        }
        let right = bounds.width - 8.0;
        if x + size.width > right {
            x = (right - size.width).max(8.0);
        }
        let bottom = bounds.height - 8.0;
        if y + size.height > bottom {
            let above = self.anchor.y - size.height - 4.0;
            y = if above >= 8.0 { above } else { 8.0 };
        }
        node.move_to(iced::Point::new(x, y))
    }

    fn update(
        &mut self,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
    ) {
        let press = matches!(
            event,
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(_))
        );
        let on_card = cursor.is_over(layout.bounds());
        let on_anchor = cursor.is_over(self.anchor);
        if press && !on_card && !on_anchor {
            shell.publish(Message::ToggleSyncPop);
            shell.capture_event();
            return;
        }
        self.card.as_widget_mut().update(
            self.tree,
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            &self.viewport,
        );
        if press && on_card {
            shell.capture_event();
        }
    }

    fn draw(
        &self,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
    ) {
        self.card.as_widget().draw(
            self.tree,
            renderer,
            theme,
            style,
            layout,
            cursor,
            &self.viewport,
        );
    }

    fn mouse_interaction(
        &self,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        renderer: &iced::Renderer,
    ) -> iced::advanced::mouse::Interaction {
        let interaction = self.card.as_widget().mouse_interaction(
            self.tree,
            layout,
            cursor,
            &self.viewport,
            renderer,
        );
        if cursor.is_over(layout.bounds())
            && interaction == iced::advanced::mouse::Interaction::None
        {
            iced::advanced::mouse::Interaction::Idle
        } else {
            interaction
        }
    }
}

struct Breeze {
    phase: f32,
    bg: Color,
    ink: Color,
    second: Color,
    word: Color,
}

impl canvas::Program<Message> for Breeze {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        frame.fill_rectangle(iced::Point::ORIGIN, bounds.size(), self.bg);
        stroke_current(&mut frame, self.phase, 0.50, self.ink, 1.0);
        stroke_current(&mut frame, self.phase + 0.8, 0.58, self.second, 0.85);
        paint_word(&mut frame, self.phase, self.word);
        vec![frame.into_geometry()]
    }
}

fn paint_word(frame: &mut canvas::Frame, phase: f32, color: Color) {
    let size = (frame.width() * 0.072).clamp(42.0, 84.0);
    let advance = size * 0.58;
    let count = 8.0;
    let total = advance * count;
    let mut x = (frame.width() - total) / 2.0 + advance / 2.0;
    let y = frame.height() * 0.34;
    let mut start = 0.0;
    for letter in "Aerodash".chars() {
        let t = ((phase - start) / 0.35).clamp(0.0, 1.0);
        let alpha = t * t * (3.0 - 2.0 * t);
        if alpha > 0.01 {
            frame.fill_text(canvas::Text {
                content: letter.to_string(),
                position: iced::Point::new(x, y),
                color: Color { a: alpha, ..color },
                size: iced::Pixels(size),
                font: serif(),
                align_x: iced::alignment::Horizontal::Center.into(),
                align_y: iced::alignment::Vertical::Center,
                ..canvas::Text::default()
            });
        }
        start += 0.28;
        x += advance;
    }
}

fn stroke_current(
    frame: &mut canvas::Frame,
    phase: f32,
    y_frac: f32,
    color: Color,
    width_mul: f32,
) {
    let width = frame.width();
    let height = frame.height();
    let span = width * 3.5;
    let amp = 7.0 + 20.0 * (phase * 0.85).sin().abs();
    let drift = phase * width * 0.7;
    let freq = std::f32::consts::TAU / (width * 1.25);
    let base_y = height * y_frac + (phase * 1.15).sin() * height * 0.035;
    let mut x = -span;
    let end = width + span * 0.35;
    let step = (width / 90.0).max(1.0);
    let mut first = true;
    let path = canvas::Path::new(|builder| {
        while x <= end {
            let along = x - drift;
            let y = base_y
                + (along * freq).sin() * amp
                + (along * freq * 0.43 + 0.6).sin() * amp * 0.32;
            let point = iced::Point::new(x, y);
            if first {
                builder.move_to(point);
                first = false;
            } else {
                builder.line_to(point);
            }
            x += step;
        }
    });
    let thickness = (1.15 + 0.7 * (phase * 1.6 + y_frac).sin().abs()) * width_mul;
    frame.stroke(
        &path,
        canvas::Stroke::default()
            .with_color(Color { a: 0.82, ..color })
            .with_width(thickness)
            .with_line_cap(canvas::LineCap::Round)
            .with_line_join(canvas::LineJoin::Round),
    );
}

#[cfg(test)]
mod history_tests {
    use std::time::{Duration, Instant};

    use iced::widget::text_editor;

    use super::{Burst, EditSnap, NoteHistory, chat_preview, edit_burst};

    fn snap(body: &str) -> EditSnap {
        EditSnap {
            title: String::new(),
            body: body.to_owned(),
            cursor: text_editor::Cursor {
                position: text_editor::Position { line: 0, column: 0 },
                selection: None,
            },
        }
    }

    #[test]
    fn typing_gathers_into_one_step() {
        let mut history = NoteHistory::new();
        let start = Instant::now();
        let empty = snap("");
        history.begin("", "", empty.cursor, Burst::Type, start);
        history.begin(
            "",
            "a",
            snap("a").cursor,
            Burst::Type,
            start + Duration::from_millis(120),
        );
        let back = history.undo(snap("ab")).expect("step");
        assert_eq!(back.body, "");
        let forward = history.redo(back).expect("forward");
        assert_eq!(forward.body, "ab");
    }

    #[test]
    fn a_pause_starts_another_step() {
        let mut history = NoteHistory::new();
        let start = Instant::now();
        history.begin("", "", snap("").cursor, Burst::Type, start);
        history.begin(
            "",
            "a",
            snap("a").cursor,
            Burst::Type,
            start + Duration::from_millis(900),
        );
        let back = history.undo(snap("ab")).expect("step");
        assert_eq!(back.body, "a");
        let earlier = history.undo(back).expect("earlier");
        assert_eq!(earlier.body, "");
    }

    #[test]
    fn a_format_does_not_merge_with_typing() {
        let mut history = NoteHistory::new();
        let start = Instant::now();
        history.begin("", "", snap("").cursor, Burst::Type, start);
        history.begin(
            "",
            "hi",
            snap("hi").cursor,
            Burst::Break,
            start + Duration::from_millis(40),
        );
        let back = history.undo(snap("**hi**")).expect("step");
        assert_eq!(back.body, "hi");
    }

    #[test]
    fn a_failed_edit_puts_the_step_back() {
        let mut history = NoteHistory::new();
        let gate = history.begin("", "a", snap("a").cursor, Burst::Erase, Instant::now());
        history.abort(gate);
        assert!(history.undo(snap("a")).is_none());
    }

    #[test]
    fn redo_is_dropped_when_typing_starts_again() {
        let mut history = NoteHistory::new();
        let start = Instant::now();
        history.begin("", "", snap("").cursor, Burst::Type, start);
        let back = history.undo(snap("a")).expect("step");
        assert_eq!(back.body, "");
        history.begin(
            "",
            "",
            snap("").cursor,
            Burst::Type,
            start + Duration::from_millis(40),
        );
        assert!(history.redo(snap("b")).is_none());
    }

    #[test]
    fn the_stack_keeps_eighty_steps() {
        let mut history = NoteHistory::new();
        let start = Instant::now();
        for index in 0..90 {
            let body = "x".repeat(index);
            let cursor = snap(&body).cursor;
            let at = start + Duration::from_millis(u64::try_from(index).unwrap_or(0));
            history.begin("", &body, cursor, Burst::Break, at);
        }
        assert_eq!(history.undo.len(), 80);
    }

    #[test]
    fn a_chat_preview_is_one_short_line() {
        assert_eq!(chat_preview("\nhello\nthere"), "hello");
        let long = "a".repeat(80);
        let preview = chat_preview(&long);
        assert!(preview.ends_with("..."));
        assert_eq!(preview.chars().count(), 75);
    }

    #[test]
    fn paste_is_its_own_step() {
        let pasted = text_editor::Action::Edit(text_editor::Edit::Paste(std::sync::Arc::new(
            "x".to_owned(),
        )));
        assert_eq!(edit_burst(&pasted), Some(Burst::Break));
        let typed = text_editor::Action::Edit(text_editor::Edit::Insert('a'));
        assert_eq!(edit_burst(&typed), Some(Burst::Type));
    }
}

#[cfg(test)]
mod scrollbar_tests {
    use iced::widget::scrollable::Status;

    use super::{SCROLL_WAKE, scroll_hot, scroll_seen, scrolled_recently, thumb_alpha};

    fn resting() -> Status {
        Status::Active {
            is_horizontal_scrollbar_disabled: false,
            is_vertical_scrollbar_disabled: false,
        }
    }

    fn over_pane() -> Status {
        Status::Hovered {
            is_horizontal_scrollbar_hovered: false,
            is_vertical_scrollbar_hovered: false,
            is_horizontal_scrollbar_disabled: false,
            is_vertical_scrollbar_disabled: false,
        }
    }

    fn over_bar() -> Status {
        Status::Hovered {
            is_horizontal_scrollbar_hovered: false,
            is_vertical_scrollbar_hovered: true,
            is_horizontal_scrollbar_disabled: false,
            is_vertical_scrollbar_disabled: false,
        }
    }

    fn dragging() -> Status {
        Status::Dragged {
            is_horizontal_scrollbar_dragged: false,
            is_vertical_scrollbar_dragged: true,
            is_horizontal_scrollbar_disabled: false,
            is_vertical_scrollbar_disabled: false,
        }
    }

    #[test]
    fn a_scrollbar_is_not_drawn_at_rest() {
        assert!(thumb_alpha(resting(), false) < f32::EPSILON);
        // Hovering a pane is not scrolling it.
        assert!(thumb_alpha(over_pane(), false) < f32::EPSILON);
        // A pane the pointer is not over stays bare even right after another one scrolled.
        assert!(thumb_alpha(resting(), true) < f32::EPSILON);
    }

    #[test]
    fn a_scrollbar_shows_for_a_moment_after_the_pane_under_the_pointer_scrolls() {
        let shown = thumb_alpha(over_pane(), true);
        assert!(
            shown > 0.0 && shown < 0.62,
            "faint, not the full thumb: {shown}"
        );
    }

    #[test]
    fn the_bar_can_still_be_found_and_dragged() {
        assert!(scroll_hot(over_bar()) && scroll_hot(dragging()));
        assert!(!scroll_hot(over_pane()) && !scroll_hot(resting()));
        assert!(thumb_alpha(over_bar(), false) > thumb_alpha(over_pane(), true));
        assert!(thumb_alpha(dragging(), false) > 0.5);
    }

    #[test]
    fn a_burst_of_wheel_ticks_sends_one_message() {
        // Scrolling must not go through `update`, which rebuilds the whole view. A tick only
        // records the time. The first tick of a burst asks for the wake that hides the bar.
        SCROLL_WAKE.store(false, std::sync::atomic::Ordering::Relaxed);
        assert!(scroll_seen().is_some(), "the first tick starts the wake");
        for _ in 0..500 {
            assert!(scroll_seen().is_none(), "a later tick sent a message");
        }
        assert!(scrolled_recently());
        // Once the wake has run, the next burst starts a new one.
        SCROLL_WAKE.store(false, std::sync::atomic::Ordering::Relaxed);
        assert!(scroll_seen().is_some());
    }
}

/// What a test needs from the app's background writes. They are recorded so a test can do them
/// itself, the way the worker thread would, and hand the result back.
#[cfg(test)]
mod tests_support {
    use std::cell::RefCell;

    type Write = (Option<String>, String, String);

    thread_local! {
        static WRITES: RefCell<Vec<Write>> = const { RefCell::new(Vec::new()) };
    }

    pub(super) fn record_write(id: Option<&str>, title: &str, body: &str) {
        WRITES.with(|writes| {
            writes
                .borrow_mut()
                .push((id.map(str::to_owned), title.to_owned(), body.to_owned()));
        });
    }

    pub(super) fn take_writes() -> Vec<Write> {
        WRITES.with(|writes| std::mem::take(&mut *writes.borrow_mut()))
    }

    /// The published notebook and the personal dictionary are each one per process, so the
    /// tests that touch them take turns.
    pub(crate) fn one_at_a_time() -> std::sync::MutexGuard<'static, ()> {
        static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
        TURN.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Tests that drive a real `App` on a real folder, through the same `update` the window uses.
#[cfg(test)]
mod app_tests {
    use super::tests_support::one_at_a_time;
    use super::{App, Message, Notebook, tests_support, write_snapshot};
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("aerodash-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn open_app(dir: &Path) -> App {
        Notebook::create(dir).expect("create");
        let (mut app, _) = App::at(dir.to_path_buf());
        app.loaded = true;
        // The splash is over, so the window takes keys.
        app.boot_at = Instant::now()
            .checked_sub(Duration::from_secs(10))
            .unwrap_or_else(Instant::now);
        app.adopt_open_book(Notebook::open(dir).expect("open"));
        let _ = tests_support::take_writes();
        app
    }

    /// Do the writes the app asked for, as its worker thread would, and hand each result back.
    fn settle(app: &mut App) {
        for (id, title, body) in tests_support::take_writes() {
            let result =
                write_snapshot(&app.dir, id.as_deref(), &title, &body).map(|(_id, notes)| notes);
            let _ = app.update(Message::Applied(result));
        }
    }

    fn listed(app: &App) -> Vec<String> {
        app.tasks.iter().map(|task| task.text.clone()).collect()
    }

    #[test]
    fn the_tasks_tab_updates_when_a_task_is_added_removed_or_checked() {
        let _turn = one_at_a_time();
        let dir = folder("tasks");
        let mut app = open_app(&dir);
        let _ = app.update(Message::ShowTasks);
        assert!(listed(&app).is_empty());

        // The first task creates the Tasks note.
        let _ = app.update(Message::TaskDraft("buy milk".to_owned()));
        let _ = app.update(Message::AddTask);
        settle(&mut app);
        assert_eq!(listed(&app), ["buy milk"], "the first task did not show");

        // The second goes into the note that is there now.
        let _ = app.update(Message::TaskDraft("call the clinic".to_owned()));
        let _ = app.update(Message::AddTask);
        settle(&mut app);
        assert_eq!(listed(&app), ["buy milk", "call the clinic"]);

        // Checking one moves it to done, without leaving the tab.
        let milk = app
            .tasks
            .iter()
            .find(|task| task.text == "buy milk")
            .expect("task");
        let (id, line) = (milk.note_id.clone(), milk.line);
        let _ = app.update(Message::SetTask {
            id: id.clone(),
            line,
            done: true,
        });
        settle(&mut app);
        let milk = app
            .tasks
            .iter()
            .find(|task| task.text == "buy milk")
            .expect("task");
        assert!(milk.done, "the check did not show");

        // Removing one takes it off the list.
        let _ = app.update(Message::RemoveTask { id, line });
        settle(&mut app);
        assert_eq!(
            listed(&app),
            ["call the clinic"],
            "the removal did not show"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_conversation_name_is_text_until_the_pencil_is_pressed() {
        let _turn = one_at_a_time();
        let dir = folder("alias");
        let mut app = open_app(&dir);
        app.chats = vec![ambient_cli::ConversationSummary {
            id: "abc".to_owned(),
            label: "Key 12ab".to_owned(),
            other: String::new(),
        }];
        app.chat_id = Some("abc".to_owned());
        assert!(app.alias_editing.is_none(), "the name starts as plain text");

        let _ = app.update(Message::EditAlias);
        assert_eq!(app.alias_editing.as_deref(), Some("abc"));
        let _ = app.update(Message::AliasDraft("abc".to_owned(), "Sam".to_owned()));
        let chat = app.chats[0].clone();
        assert_eq!(app.chat_name(&chat), "Sam", "typing names them at once");

        // Enter, or Done, puts the plain name back.
        let _ = app.update(Message::FinishAlias);
        assert!(app.alias_editing.is_none());
        assert_eq!(app.chat_name(&chat), "Sam");

        // The pencil again opens it, and pressing it once more closes it.
        let _ = app.update(Message::EditAlias);
        assert!(app.alias_editing.is_some());
        let _ = app.update(Message::EditAlias);
        assert!(app.alias_editing.is_none());

        // Leaving and coming back does not leave the field open.
        let _ = app.update(Message::EditAlias);
        let _ = app.update(Message::ShowNotes);
        let _ = app.update(Message::ShowChat);
        assert!(app.alias_editing.is_none());

        // Clearing the name falls back to the label the conversation came with.
        let _ = app.update(Message::AliasDraft("abc".to_owned(), String::new()));
        assert_eq!(app.chat_name(&chat), "Key 12ab");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn wide(app: &mut App) {
        let _ = app.update(Message::Resized(iced::Size::new(1100.0, 720.0)));
    }

    #[test]
    fn focus_hides_the_panels_and_sends_the_bar_away() {
        let _turn = one_at_a_time();
        let dir = folder("focus");
        let mut app = open_app(&dir);
        wide(&mut app);
        assert!(!app.focused(), "focus starts off");
        assert!(app.slide_bar.open);

        let _ = app.update(Message::ToggleFocus);
        assert!(app.focused());
        assert!(!app.slide_bar.open, "the bar slides away");
        assert!(app.slide_bar.moving(), "and it moves, not jumps");

        // The Aa button brings it back, and sends it away again.
        let _ = app.update(Message::FocusBar);
        assert!(app.slide_bar.open);
        let _ = app.update(Message::FocusBar);
        assert!(!app.slide_bar.open);

        // Leaving puts everything back at once, so nothing is left hidden.
        let _ = app.update(Message::ToggleFocus);
        assert!(!app.focused());
        assert!(app.slide_bar.open);
        assert!((app.slide_bar.shown() - 1.0).abs() < f32::EPSILON);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn focus_only_applies_to_writing_a_note_in_the_wide_layout() {
        let _turn = one_at_a_time();
        let dir = folder("focus-where");
        let mut app = open_app(&dir);
        wide(&mut app);
        let _ = app.update(Message::ToggleFocus);
        assert!(app.focused());
        // Another screen is not changed by it.
        let _ = app.update(Message::ShowTasks);
        assert!(!app.focused());
        let _ = app.update(Message::ShowNotes);
        assert!(app.focused(), "it is still on when the notes are back");
        // A narrow window has its own layout.
        let _ = app.update(Message::Resized(iced::Size::new(600.0, 700.0)));
        assert!(!app.focused());
        let _ = app.update(Message::Resized(iced::Size::new(1100.0, 720.0)));
        assert!(app.focused());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_window_draws_in_every_state_of_focus() {
        let _turn = one_at_a_time();
        let dir = folder("focus-draw");
        let mut app = open_app(&dir);
        wide(&mut app);
        let _ = app.update(Message::TitleChanged("A note".to_owned()));
        // Normal, going into focus (mid-slide), in focus with each thing pulled in, and out again.
        drop(app.view());
        let _ = app.update(Message::ToggleFocus);
        drop(app.view());
        let _ = app.update(Message::FocusBar);
        drop(app.view());
        let _ = app.update(Message::FocusBar);
        drop(app.view());
        // With the delete question open, which is drawn in the corner where the buttons were.
        let _ = app.update(Message::AskDelete);
        drop(app.view());
        let _ = app.update(Message::CancelDelete);
        let _ = app.update(Message::ToggleFocus);
        drop(app.view());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_tasks_note_is_not_in_the_notes_list_and_opens_over_the_tasks_tab() {
        let _turn = one_at_a_time();
        let dir = folder("tasks-note-hidden");
        let mut app = open_app(&dir);
        wide(&mut app);
        let _ = app.update(Message::ShowTasks);
        let _ = app.update(Message::TaskDraft("buy milk".to_owned()));
        let _ = app.update(Message::AddTask);
        settle(&mut app);
        assert!(app.catalog.iter().any(|note| note.title == "Tasks"));
        assert!(
            app.notes.iter().all(|note| note.title != "Tasks"),
            "the notes list does not show it"
        );
        let _ = app.update(Message::ToggleTasksNote);
        assert!(
            app.tasks_note_text()
                .is_some_and(|text| text.contains("buy milk"))
        );
        drop(app.view());
        let _ = app.update(Message::ShowNotes);
        let _ = app.update(Message::ShowTasks);
        assert!(!app.tasks_note_open, "it is shut on the way back");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_settings_icon_switches_the_right_panel_and_back() {
        let _turn = one_at_a_time();
        let dir = folder("note-settings");
        let mut app = open_app(&dir);
        wide(&mut app);
        let _ = app.update(Message::TitleChanged("A note".to_owned()));
        assert!(!app.note_panel, "links and backlinks are the default");
        drop(app.view());
        let _ = app.update(Message::ToggleNoteSettings);
        assert!(
            app.note_panel && app.show_context,
            "the panel opens on settings"
        );
        drop(app.view());
        let _ = app.update(Message::ToggleNoteSettings);
        assert!(!app.note_panel);
        drop(app.view());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_row_shows_delete_under_the_pointer_and_asks_before_deleting() {
        let _turn = one_at_a_time();
        let dir = folder("row-delete");
        let mut app = open_app(&dir);
        wide(&mut app);
        let _ = app.update(Message::TitleChanged("First".to_owned()));
        let _ = app.update(Message::Save);
        let id = app.selected.clone();
        let Some(id) = id else {
            // The save is a task in the window; nothing to delete in this harness.
            let _ = std::fs::remove_dir_all(&dir);
            return;
        };
        let _ = app.update(Message::RowHover(id.clone(), true));
        assert_eq!(app.hover_row.as_deref(), Some(id.as_str()));
        drop(app.view());
        let _ = app.update(Message::RowHover("other".to_owned(), false));
        assert_eq!(
            app.hover_row.as_deref(),
            Some(id.as_str()),
            "another row leaving changes nothing"
        );
        let _ = app.update(Message::RowHover(id.clone(), false));
        assert!(app.hover_row.is_none());
        let _ = app.update(Message::AskDeleteNote(id));
        assert!(app.confirm_delete, "it asks first");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_slide_eases_between_hidden_and_shown() {
        use super::Slide;
        assert!(Slide::settled(false).shown() < f32::EPSILON);
        assert!((Slide::settled(true).shown() - 1.0).abs() < f32::EPSILON);
        assert!(!Slide::settled(true).moving());
        let mut slide = Slide::settled(false);
        slide.set(true);
        assert!(slide.moving());
        let now = slide.shown();
        assert!((0.0..=1.0).contains(&now), "{now}");
        // Setting it to what it already is does not restart it.
        let before = slide.since;
        slide.set(true);
        assert_eq!(slide.since, before);
        slide.jump(false);
        assert!(slide.shown() < f32::EPSILON && !slide.moving());
    }

    #[test]
    fn a_word_is_added_to_the_dictionary_from_the_caret() {
        let _turn = one_at_a_time();
        let dir = folder("dictionary");
        let mut app = open_app(&dir);
        wide(&mut app);
        app.body.load("a zzquuxword here");
        app.body.place_byte(5);
        let _ = app.update(Message::AddWord);
        assert!(app.my_words.contains("zzquuxword"), "{}", app.message);
        assert!(app.message.contains("zzquuxword"));
        assert_eq!(super::profile::load_words(&dir), ["zzquuxword"]);

        // Again says so, and does not write twice.
        let _ = app.update(Message::AddWord);
        assert!(app.message.contains("already"), "{}", app.message);
        assert_eq!(app.my_words.len(), 1);

        // Between words there is nothing to add.
        app.body.place_byte(1);
        let _ = app.update(Message::AddWord);
        assert!(app.message.contains("cursor"), "{}", app.message);
        assert_eq!(app.my_words.len(), 1);

        // A new session reads the file back.
        let (again, _) = App::at(dir.clone());
        assert!(again.my_words.contains("zzquuxword"));
        super::markdown::set_personal_words(&[]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_dictionary_file_ignores_what_is_not_a_word() {
        let dir = folder("dictionary-file");
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(
            dir.join("dictionary"),
            "good\nNo\nab\nhas space\n123\nfine'ish\n\n",
        )
        .expect("write");
        assert_eq!(super::profile::load_words(&dir), ["good", "fine'ish"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn erasing_the_notebook_erases_the_dictionary_too() {
        let _turn = one_at_a_time();
        let dir = folder("wipe-dictionary");
        let mut app = open_app(&dir);
        wide(&mut app);
        app.body.load("a zzquuxwipe here");
        app.body.place_byte(5);
        let _ = app.update(Message::AddWord);
        assert!(dir.join("dictionary").is_file());
        assert!(app.my_words.contains("zzquuxwipe"));

        let _ = app.update(Message::WipeDraft("delete".to_owned()));
        let _ = app.update(Message::ConfirmWipe);
        assert!(!dir.join("dictionary").exists(), "the file is gone");
        assert!(app.my_words.is_empty(), "and so are the words in memory");
        // A word added to the next notebook does not bring the old one back.
        assert!(super::profile::load_words(&dir).is_empty());
        super::markdown::set_personal_words(&[]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_backup_copies_the_folder_remembers_when_and_can_be_opened() {
        let _turn = one_at_a_time();
        let dir = folder("backup-flow");
        let mut app = open_app(&dir);
        let id = Notebook::open(&dir)
            .expect("open")
            .add("Plan", "buy oats")
            .expect("add");
        let place = folder("backup-flow-usb");
        let _ = app.update(Message::BackupTo(place.display().to_string()));

        let _ = app.update(Message::BackUp);
        assert!(app.backup_busy, "it is working");
        assert_eq!(app.message, "Backing up.");
        // A second press while it works does nothing.
        let _ = app.update(Message::BackUp);
        assert!(app.backup_busy);

        // What the worker thread does, and its answer.
        let report =
            super::profile::backup_folder(&app.dir, &place, "aerodash-test").expect("the copy");
        let _ = app.update(Message::BackedUp(Ok(report.clone())));
        assert!(!app.backup_busy);
        assert!(app.backup_at > 0, "the time is remembered");
        assert!(app.message.contains("Backed up") && app.message.contains("aerodash-test"));
        let saved = super::profile::load_profile(&dir);
        assert_eq!(saved.backup_at, app.backup_at, "and kept for next time");
        assert_eq!(saved.backup_to, place.display().to_string());

        // The copy is a whole notebook.
        let copy = Notebook::open(&report.path).expect("the copy opens");
        assert_eq!(copy.show(&id).expect("the note").heads[0].body, "buy oats");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&place);
    }

    #[test]
    fn a_failed_backup_says_why_and_changes_nothing_else() {
        let _turn = one_at_a_time();
        let dir = folder("backup-fail");
        let mut app = open_app(&dir);
        let _ = app.update(Message::BackupTo(dir.join("inside").display().to_string()));
        let _ = app.update(Message::BackUp);
        assert!(app.backup_busy);
        let refused = super::profile::backup_folder(&app.dir, &dir.join("inside"), "copy");
        let _ = app.update(Message::BackedUp(refused));
        assert!(!app.backup_busy, "it can be tried again");
        assert!(app.message.contains("outside"), "{}", app.message);
        assert_eq!(app.backup_at, 0, "no backup is claimed");
        assert!(
            !dir.join("inside").exists(),
            "nothing was left in the notebook"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_note_typed_a_moment_ago_is_saved_before_the_copy_is_made() {
        let _turn = one_at_a_time();
        let dir = folder("backup-dirty");
        let mut app = open_app(&dir);
        wide(&mut app);
        let _ = app.update(Message::TitleChanged("Not saved yet".to_owned()));
        assert!(app.editor_dirty());
        let _ = app.update(Message::BackUp);
        assert!(!app.backup_busy, "the copy waits for the save");
        assert!(app.message.contains("Saving"), "{}", app.message);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_backup_card_draws_with_and_without_a_passphrase() {
        let _turn = one_at_a_time();
        let dir = folder("backup-card");
        let mut app = open_app(&dir);
        wide(&mut app);
        let _ = app.update(Message::ShowSettings);
        assert!(!ambient_cli::folder_key_is_wrapped(&dir));
        drop(app.view());
        ambient_cli::set_folder_unlock(&dir, "correct horse").expect("lock");
        assert!(ambient_cli::folder_key_is_wrapped(&dir));
        drop(app.view());
        // And after a copy, and while one is running.
        let _ = app.update(Message::BackedUp(Err("no".to_owned())));
        app.backup_at = 1_700_000_000_000;
        drop(app.view());
        app.backup_busy = true;
        drop(app.view());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sizes_read_as_people_say_them() {
        assert_eq!(super::size_label(0), "0 bytes");
        assert_eq!(super::size_label(812), "812 bytes");
        assert_eq!(super::size_label(1024), "1 KB");
        assert_eq!(super::size_label(340 * 1024), "340 KB");
        assert_eq!(super::size_label(3 * 1024 * 1024 + 450_000), "3.4 MB");
        assert_eq!(super::size_label(1_300_000_000), "1.2 GB");
    }

    #[test]
    fn a_typed_place_can_start_with_the_home_folder() {
        use std::path::PathBuf;
        assert_eq!(
            super::profile::expand_home("/media/usb"),
            PathBuf::from("/media/usb")
        );
        assert_eq!(
            super::profile::expand_home("backups"),
            PathBuf::from("backups")
        );
        if let Some(home) = std::env::var_os("HOME").filter(|home| !home.is_empty()) {
            assert_eq!(
                super::profile::expand_home("~/Backups"),
                PathBuf::from(&home).join("Backups")
            );
            assert_eq!(super::profile::expand_home("~"), PathBuf::from(home));
        }
    }

    /// Type `text` the way a person does: one key at a time through `update`, with the pause that
    /// follows a key allowed to fire now and then. After every key the page must hold exactly what
    /// was typed and the caret must sit just after it.
    fn type_through(app: &mut App, text: &str, wake_every: usize) {
        let mut typed = String::new();
        for (index, ch) in text.chars().enumerate() {
            let action = if ch == '\n' {
                iced::widget::text_editor::Action::Edit(iced::widget::text_editor::Edit::Enter)
            } else {
                iced::widget::text_editor::Action::Edit(iced::widget::text_editor::Edit::Insert(ch))
            };
            let _ = app.update(Message::BodyEdited(action));
            typed.push(ch);
            if wake_every > 0 && index % wake_every == wake_every - 1 {
                let _ = app.update(Message::Woke(super::Wake::Pause(app.pause_token)));
                settle(app);
            }
            let shown = app.body.text();
            assert_eq!(
                shown, typed,
                "after {index} keys, the page lost or moved a character"
            );
            assert_eq!(
                app.body.caret_byte(),
                typed.len(),
                "after {index} keys ({:?}), the caret is not just after what was typed",
                &typed[typed.len().saturating_sub(12)..]
            );
        }
    }

    #[test]
    fn fast_typing_keeps_every_character_and_the_caret_at_the_end() {
        let _turn = one_at_a_time();
        let dir = folder("typing");
        let mut app = open_app(&dir);
        wide(&mut app);
        let _ = app.update(Message::NewNote);
        let passage = "The **quick** brown fox jumpd over ==the== lazy dog, and ~~then~~ it did not.\n\
A second paragraph with a [link](https://example.com/a) in it, and a # that is not a heading.\n\
Some `code` and *an em* and __strong__ words. Done.\n- last, a list item with no Enter after it";
        type_through(&mut app, passage, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn typing_while_saves_come_back_keeps_every_character() {
        let _turn = one_at_a_time();
        let dir = folder("typing-saves");
        let mut app = open_app(&dir);
        wide(&mut app);
        let _ = app.update(Message::NewNote);
        let passage = "A line of words typed with a pause that fires every few keys, so a save is \
always on its way back while the next letters arrive. **Bold ones** too, and a misspeling.";
        type_through(&mut app, passage, 3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_key_nothing_took_is_typed_and_a_shortcut_is_not() {
        use iced::keyboard::key::Named;
        use iced::keyboard::{Key, Modifiers};
        let plain = Modifiers::empty();
        let letter =
            |text: &str| super::ignored_key(&Key::Character(text.into()), Some(text), plain);
        assert_eq!(letter("a"), Some(super::Typed::Text("a".to_owned())));
        assert_eq!(letter("é"), Some(super::Typed::Text("é".to_owned())));
        assert_eq!(
            super::ignored_key(&Key::Named(Named::Space), None, plain),
            Some(super::Typed::Text(" ".to_owned()))
        );
        assert_eq!(
            super::ignored_key(&Key::Named(Named::Enter), Some("\r"), plain),
            Some(super::Typed::Enter)
        );
        assert_eq!(
            super::ignored_key(&Key::Named(Named::Backspace), None, plain),
            Some(super::Typed::Backspace)
        );
        assert_eq!(
            super::ignored_key(&Key::Named(Named::Delete), None, plain),
            Some(super::Typed::Delete)
        );
        // A key that types nothing, and a key with a shortcut modifier, are left alone.
        assert_eq!(
            super::ignored_key(&Key::Named(Named::ArrowLeft), None, plain),
            None
        );
        assert_eq!(
            super::ignored_key(&Key::Named(Named::F5), None, plain),
            None
        );
        assert_eq!(
            super::ignored_key(&Key::Named(Named::Shift), None, plain),
            None
        );
        for held in [Modifiers::CTRL, Modifiers::LOGO, Modifiers::ALT] {
            assert_eq!(
                super::ignored_key(&Key::Character("b".into()), Some("b"), held),
                None
            );
        }
        // A control character in the text is not typed.
        assert_eq!(
            super::ignored_key(&Key::Character("x".into()), Some("\u{7}"), plain),
            None
        );
    }

    #[test]
    fn a_key_the_editor_dropped_is_typed_into_the_page_in_order() {
        let _turn = one_at_a_time();
        let dir = folder("dropped-keys");
        let mut app = open_app(&dir);
        wide(&mut app);
        let _ = app.update(Message::NewNote);
        for typed in [
            super::Typed::Text("hello".to_owned()),
            super::Typed::Text(" ".to_owned()),
            super::Typed::Text("wor".to_owned()),
            super::Typed::Backspace,
            super::Typed::Text("rld".to_owned()),
            super::Typed::Enter,
            super::Typed::Text("next".to_owned()),
        ] {
            let _ = app.update(Message::TypedIgnored(typed));
        }
        assert_eq!(app.body.text(), "hello world\nnext");
        assert_eq!(app.body.caret_byte(), app.body.text().len());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_dropped_key_is_not_typed_when_something_else_is_up() {
        let _turn = one_at_a_time();
        let dir = folder("dropped-keys-guard");
        let mut app = open_app(&dir);
        wide(&mut app);
        let _ = app.update(Message::NewNote);
        let typed = || super::Typed::Text("x".to_owned());
        // The settings page, another tab, and the delete question each have their own keys.
        let _ = app.update(Message::ShowSettings);
        let _ = app.update(Message::TypedIgnored(typed()));
        let _ = app.update(Message::ShowTasks);
        let _ = app.update(Message::TypedIgnored(typed()));
        let _ = app.update(Message::ShowNotes);
        app.confirm_delete = true;
        let _ = app.update(Message::TypedIgnored(typed()));
        app.confirm_delete = false;
        assert_eq!(
            app.body.text(),
            "",
            "a key was typed into a page that was not up"
        );
        // And with the page up, it is.
        let _ = app.update(Message::TypedIgnored(typed()));
        assert_eq!(app.body.text(), "x");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
