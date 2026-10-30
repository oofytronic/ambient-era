//! Linux notes window.
//!
//! Iced draws the controls in a system window. There is no webview. The folder
//! is the same one the `ambient` CLI uses. Pass a path, or it uses `./notebook`.
//! Sync, pairing, and chat call `ambient-cli`. This file does not seal events.
//! The relay it can start is cleartext on `127.0.0.1` only.

use std::path::{Path, PathBuf};

use ambient_cli::{Draft, NoteSummary, Notebook};
use iced::clipboard;
use iced::keyboard::Key;
use iced::widget::{button, column, row, scrollable, text, text_editor, text_input};
use iced::{Element, Fill, Task, Theme};

fn main() -> iced::Result {
    iced::application(App::boot, App::update, App::view)
        .title(App::title)
        .window_size((1280.0, 820.0))
        .theme(theme)
        .run()
}

fn theme(_app: &App) -> Theme {
    Theme::Dark
}

struct App {
    dir: PathBuf,
    notebook: Option<Notebook>,
    open_error: String,
    notes: Vec<NoteSummary>,
    query: String,
    selected: Option<String>,
    title_text: String,
    body: text_editor::Content,
    conflicts: Vec<(String, String)>,
    message: String,
    show_chat: bool,
    chat: Vec<String>,
    draft_chat: String,
    my_card: String,
    their_card: String,
    relay_ready: bool,
}

#[derive(Clone, Debug)]
enum Message {
    QueryChanged(String),
    Select(String),
    NewNote,
    TitleChanged(String),
    BodyEdited(text_editor::Action),
    Save,
    ShowNotes,
    ShowChat,
    StartRelay,
    Sync,
    OpenPaired,
    OpenPerson,
    CopyCard,
    MyCardChanged(String),
    TheirCardChanged(String),
    StartConversation,
    DraftChat(String),
    SendChat,
}

impl App {
    fn boot() -> (Self, Task<Message>) {
        let dir = std::env::args()
            .nth(1)
            .map_or_else(|| PathBuf::from("notebook"), PathBuf::from);
        let mut app = Self {
            dir,
            notebook: None,
            open_error: String::new(),
            notes: Vec::new(),
            query: String::new(),
            selected: None,
            title_text: String::new(),
            body: text_editor::Content::new(),
            conflicts: Vec::new(),
            message: String::new(),
            show_chat: false,
            chat: Vec::new(),
            draft_chat: String::new(),
            my_card: String::new(),
            their_card: String::new(),
            relay_ready: false,
        };
        match open_notebook(&app.dir) {
            Ok(book) => {
                let paired = !book.holds_root();
                app.notebook = Some(book);
                app.reload();
                app.reload_chat();
                if let Ok(card) = ambient_cli::contact_card(&app.dir) {
                    app.my_card = card;
                }
                if paired && app.notes.is_empty() {
                    app.say("Paired device. It does not hold the root. Press Sync after the first window has synced.");
                }
            }
            Err(err) => app.open_error = err,
        }
        (app, Task::none())
    }

    fn title(&self) -> String {
        let role = match self.notebook.as_ref() {
            Some(book) if book.holds_root() => "root",
            Some(_) => "paired device",
            None => "not open",
        };
        format!("Ambient — {} ({role})", self.dir.display())
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::QueryChanged(query) => {
                self.query = query;
                self.reload();
                Task::none()
            }
            Message::Select(id) => {
                self.show_note(&id);
                Task::none()
            }
            Message::NewNote => {
                self.clear_editor();
                Task::none()
            }
            Message::TitleChanged(title) => {
                self.title_text = title;
                Task::none()
            }
            Message::BodyEdited(action) => {
                self.body.perform(action);
                Task::none()
            }
            Message::Save => {
                self.save();
                Task::none()
            }
            Message::ShowNotes => {
                self.show_chat = false;
                Task::none()
            }
            Message::ShowChat => {
                self.show_chat = true;
                self.reload_chat();
                Task::none()
            }
            Message::StartRelay => {
                self.start_relay();
                Task::none()
            }
            Message::Sync => {
                self.sync_now();
                Task::none()
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
            Message::DraftChat(text) => {
                self.draft_chat = text;
                Task::none()
            }
            Message::SendChat => {
                self.send_chat();
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        row![self.sidebar(), self.center(), self.link_panel()].into()
    }

    fn sidebar(&self) -> Element<'_, Message> {
        let role = match self.notebook.as_ref() {
            Some(book) if book.holds_root() => "Holds the root",
            Some(_) => "Paired device",
            None => "Not open",
        };
        let mut notes = column![];
        for note in &self.notes {
            let mut label = if note.title.is_empty() {
                "Untitled".to_owned()
            } else {
                note.title.clone()
            };
            if self.selected.as_deref() == Some(note.id.as_str()) {
                label.insert_str(0, "● ");
            }
            if note.conflict {
                label.push_str("  conflict");
            }
            notes = notes.push(
                button(text(label))
                    .width(Fill)
                    .on_press(Message::Select(note.id.clone())),
            );
        }
        column![
            text("Ambient").size(22),
            text(role).size(12),
            text_input("Search", &self.query).on_input(Message::QueryChanged),
            button(text("New note")).on_press(Message::NewNote),
            scrollable(notes).height(Fill),
        ]
        .spacing(12)
        .padding(16)
        .width(260)
        .into()
    }

    fn center(&self) -> Element<'_, Message> {
        let mut body = column![
            row![
                button(text("Notes")).on_press(Message::ShowNotes),
                button(text("Chat")).on_press(Message::ShowChat),
            ]
            .spacing(8)
        ]
        .spacing(12)
        .width(Fill)
        .height(Fill);
        if !self.open_error.is_empty() {
            body = body.push(text(format!(
                "Could not open this folder. {}",
                self.open_error
            )));
        }
        if !self.message.is_empty() {
            body = body.push(text(self.message.as_str()));
        }
        if self.show_chat {
            body = body.push(self.chat_pane());
        } else {
            body = body.push(self.notes_pane());
        }
        body.padding(24).into()
    }

    fn notes_pane(&self) -> Element<'_, Message> {
        let mut body = column![].spacing(12).height(Fill);
        if !self.conflicts.is_empty() {
            let mut versions = column![text(
                "Two versions. Write the one you want to keep, then save."
            )]
            .spacing(8);
            for (index, (title, note_body)) in self.conflicts.iter().enumerate() {
                let number = index.saturating_add(1);
                versions = versions.push(text(format!("Version {number}: {title}")));
                versions = versions.push(text(note_body));
            }
            body = body.push(scrollable(versions).height(160));
        }
        body = body.push(
            text_input("Title", &self.title_text)
                .on_input(Message::TitleChanged)
                .size(28),
        );
        body = body.push(
            text_editor(&self.body)
                .height(Fill)
                .placeholder("Write")
                .on_action(Message::BodyEdited)
                .key_binding(binding),
        );
        body = body.push(
            row![
                button(text("Save")).on_press(Message::Save),
                text("Ctrl+S saves. Ctrl+N is a new note. The key file stays in this folder.")
                    .size(12),
            ]
            .spacing(16),
        );
        body.into()
    }

    fn chat_pane(&self) -> Element<'_, Message> {
        let mut lines = column![].spacing(8);
        if self.chat.is_empty() {
            lines = lines.push(text(
                "No messages yet. Start the conversation from the panel, then Sync both windows.",
            ));
        }
        for line in &self.chat {
            lines = lines.push(text(line));
        }
        column![
            scrollable(lines).height(Fill),
            text_input("Message", &self.draft_chat)
                .on_input(Message::DraftChat)
                .on_submit(Message::SendChat),
            button(text("Send")).on_press(Message::SendChat),
            text("Send stores it here. It reaches the other window only after Sync in both.")
                .size(12),
        ]
        .spacing(12)
        .height(Fill)
        .into()
    }

    fn link_panel(&self) -> Element<'_, Message> {
        let relay_label = if self.relay_ready {
            "Relay is on"
        } else {
            "Start relay"
        };
        let mut panel = column![
            text("Test").size(22),
            text(self.dir.display().to_string()).size(12),
            text("1. Relay").size(16),
            text("Start it in one window. Cleartext on this computer only. Not a public server.").size(12),
            button(text(relay_label))
                .width(Fill)
                .on_press(Message::StartRelay),
            button(text("Sync")).width(Fill).on_press(Message::Sync),
            text("2. Second device").size(16),
            text("Save a note here. Open the paired window. Press Sync in both. That folder does not hold the root.").size(12),
        ]
        .spacing(8)
        .width(Fill);
        if self.notebook.as_ref().is_some_and(Notebook::holds_root) {
            panel = panel.push(
                button(text("Open paired window"))
                    .width(Fill)
                    .on_press(Message::OpenPaired),
            );
            panel = panel.push(
                button(text("Open the other person"))
                    .width(Fill)
                    .on_press(Message::OpenPerson),
            );
        }
        panel = panel.push(text("3. Conversation").size(16));
        panel = panel.push(text("Copy your card in the other person's window and paste it below. A card is not a device offer. Only one window presses Start conversation. Then Sync both, open Chat, and send.").size(12));
        panel = panel.push(text("Your card").size(12));
        panel = panel.push(text_input("Your card", &self.my_card).on_input(Message::MyCardChanged));
        panel = panel.push(
            button(text("Copy card"))
                .width(Fill)
                .on_press(Message::CopyCard),
        );
        panel = panel.push(text("Their card").size(12));
        panel = panel.push(
            text_input("Paste their card", &self.their_card).on_input(Message::TheirCardChanged),
        );
        panel = panel.push(
            button(text("Start conversation"))
                .width(Fill)
                .on_press(Message::StartConversation),
        );
        column![scrollable(panel).height(Fill)]
            .padding(16)
            .width(340)
            .into()
    }

    fn reload(&mut self) {
        let Some(book) = self.notebook.as_ref() else {
            return;
        };
        let found = if self.query.is_empty() {
            book.list()
        } else {
            book.search(&self.query)
        };
        match found {
            Ok(notes) => self.notes = notes,
            Err(err) => self.message = err.to_string(),
        }
    }

    fn reload_chat(&mut self) {
        match ambient_cli::chat_lines(&self.dir) {
            Ok(lines) => self.chat = lines,
            Err(err) => self.message = err.to_string(),
        }
    }

    fn show_note(&mut self, id: &str) {
        let Some(book) = self.notebook.as_ref() else {
            return;
        };
        match book.show(id) {
            Ok(note) => {
                let draft = Draft::from_note(&note);
                self.selected = Some(id.to_owned());
                self.title_text = draft.title;
                self.body = text_editor::Content::with_text(&draft.body);
                self.conflicts = draft.conflicts;
                self.show_chat = false;
                self.message.clear();
            }
            Err(err) => self.message = err.to_string(),
        }
    }

    fn clear_editor(&mut self) {
        self.selected = None;
        self.title_text.clear();
        self.body = text_editor::Content::new();
        self.conflicts.clear();
        self.show_chat = false;
        self.message.clear();
    }

    fn save(&mut self) {
        let body = self.body.text();
        let title = self.title_text.clone();
        let current = self.selected.clone();
        let Some(book) = self.notebook.as_ref() else {
            return;
        };
        let saved = match current.as_deref() {
            Some(id) => book.edit(id, &title, &body).map(|()| id.to_owned()),
            None => book.add(&title, &body),
        };
        match saved {
            Ok(id) => {
                self.message.clear();
                self.reload();
                self.show_note(&id);
            }
            Err(err) => self.message = err.to_string(),
        }
    }

    fn say(&mut self, text: &str) {
        text.clone_into(&mut self.message);
    }

    fn start_relay(&mut self) {
        match ambient_cli::start_loopback_relay() {
            Ok(()) => {
                self.relay_ready = true;
                self.say("Relay is listening on 127.0.0.1:8787. This computer only.");
            }
            Err(err) => {
                self.relay_ready = true;
                self.message = format!("{err}. Press Sync if that relay is already this app.");
            }
        }
    }

    fn sync_now(&mut self) {
        match ambient_cli::sync_folder(&self.dir, ambient_cli::LOOPBACK_URL) {
            Ok(_) => {
                self.say("Synced. Press Sync in the other window too.");
                match Notebook::open(&self.dir) {
                    Ok(book) => {
                        self.notebook = Some(book);
                        self.reload();
                    }
                    Err(err) => self.message = err.to_string(),
                }
                self.reload_chat();
            }
            Err(err) => self.message = err.to_string(),
        }
    }

    fn open_paired(&mut self) {
        if !self.notebook.as_ref().is_some_and(Notebook::holds_root) {
            self.say("This window is the paired device. It does not hold the root. Press Sync.");
            return;
        }
        let path = sibling(&self.dir, "-device");
        if path.join("key").exists() {
            self.spawn_at(
                &path,
                "Opened the paired window. Press Sync in both. Close a duplicate if one was already open.",
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
            "Paired. That window does not hold the root. Save a note, then press Sync in both.",
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
            "Opened the other person. In that window press Copy card, then paste it here. Only this window presses Start conversation. Then Sync both.",
        );
    }

    fn copy_card(&mut self) -> Task<Message> {
        match ambient_cli::contact_card(&self.dir) {
            Ok(card) => {
                self.my_card.clone_from(&card);
                self.say("Card copied. Paste it into Their card on the other window. This is not a device offer.");
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
            self.say("Paste their card first. Copy it from their window.");
            return;
        }
        match ambient_cli::start_conversation(&self.dir, card) {
            Ok(()) => {
                self.show_chat = true;
                self.say("Conversation saved here. Press Sync, then Sync in their window.");
                self.reload_chat();
            }
            Err(err) => self.message = err.to_string(),
        }
    }

    fn send_chat(&mut self) {
        let text = self.draft_chat.trim().to_owned();
        if text.is_empty() {
            self.say("Write a message first.");
            return;
        }
        match ambient_cli::send_chat(&self.dir, &text) {
            Ok(()) => {
                self.draft_chat.clear();
                self.show_chat = true;
                self.say("Saved here. Press Sync, then Sync in the other window.");
                self.reload_chat();
            }
            Err(err) => self.message = err.to_string(),
        }
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

fn open_notebook(dir: &Path) -> Result<Notebook, String> {
    let opened = if dir.join("key").exists() {
        Notebook::open(dir)
    } else {
        Notebook::create(dir)
    };
    opened.map_err(|err| err.to_string())
}

fn binding(press: text_editor::KeyPress) -> Option<text_editor::Binding<Message>> {
    let character = match press.key.as_ref() {
        Key::Character(key) => Some(key.to_owned()),
        _ => None,
    };
    let command = press.modifiers.control() || press.modifiers.command();
    match (character.as_deref(), command) {
        (Some("s"), true) => Some(text_editor::Binding::Custom(Message::Save)),
        (Some("n"), true) => Some(text_editor::Binding::Custom(Message::NewNote)),
        _ => text_editor::Binding::from_key_press(press),
    }
}
