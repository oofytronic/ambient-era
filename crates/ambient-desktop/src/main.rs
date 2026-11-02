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
use iced::widget::{button, column, container, row, scrollable, text, text_editor, text_input};
use iced::widget::image as iced_image;
use iced::{Element, Fill, Task, Theme};
use qrcode::QrCode;
use image::ImageEncoder;
use std::io::Cursor;

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
    recovery_phrase: String,
    restore_phrase: String,
    contact_link: String,
    show_qr: bool,
    qr_bytes: Option<Vec<u8>>,
    unlock_input: String,
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
    ShowPhrase,
    CopyPhrase,
    RestorePhraseChanged(String),
    StartRestore,
    CopyLink,
    ToggleQr,
    SendNoteToChat,
    UnlockChanged(String),
    SubmitUnlock,
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
            recovery_phrase: String::new(),
            restore_phrase: String::new(),
            contact_link: String::new(),
            show_qr: false,
            qr_bytes: None,
            unlock_input: String::new(),
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
                if let Ok(link) = ambient_cli::contact_link(&app.dir) {
                    app.contact_link = link;
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

    #[allow(clippy::too_many_lines)]
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
            Message::UnlockChanged(text) => {
                self.unlock_input = text;
                Task::none()
            }
            Message::SubmitUnlock => {
                self.submit_unlock();
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
        if self.open_error == "this folder is locked" {
            return column![
                text("This folder is locked").size(22),
                text("The key file is wrapped. The passphrase is not the root, not the recovery phrase, and not a platform passkey. This window has no WebAuthn API, so the passphrase is the local unlock.").size(12),
                text_input("Passphrase", &self.unlock_input)
                    .on_input(Message::UnlockChanged)
                    .on_submit(Message::SubmitUnlock)
                    .secure(true),
                button(text("Unlock")).on_press(Message::SubmitUnlock),
                text(self.message.as_str()),
            ]
            .spacing(12)
            .padding(24)
            .into();
        }
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
        let mut actions = row![
            button(text("Save")).on_press(Message::Save),
        ]
        .spacing(8);
        if self.selected.is_some() {
            actions = actions.push(
                button(text("Send to chat")).on_press(Message::SendNoteToChat),
            );
        }
        body = body.push(
            row![
                actions,
                text("Ctrl+S saves. Ctrl+N is a new note. Send to chat copies the note text into the conversation.")
                    .size(12),
            ]
            .spacing(16),
        );
        body.into()
    }

    fn chat_pane(&self) -> Element<'_, Message> {
        let mut lines = column![].spacing(10);
        if self.chat.is_empty() {
            lines = lines.push(text(
                "No messages yet. Start the conversation from the panel, then Sync both windows.",
            ));
        }
        for line in &self.chat {
            lines = lines.push(
                container(text(line))
                    .padding(8)
                    .style(|_theme| container::Style {
                        background: Some(iced::Background::Color(iced::Color::from_rgb(0.15, 0.15, 0.15))),
                        ..Default::default()
                    }),
            );
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
            text("Unlock").size(16),
            text("Wraps the key file with a passphrase. Not the root, not the recovery phrase, and not a platform passkey.").size(12),
            text_input("Passphrase", &self.unlock_input)
                .on_input(Message::UnlockChanged)
                .secure(true),
            button(text(if self.open_error == "this folder is locked" { "Unlock" } else { "Wrap key file" }))
                .width(Fill)
                .on_press(Message::SubmitUnlock),
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
            panel = panel.push(text("4. Recovery").size(16));
            panel = panel.push(text("Show the phrase, then restore into a new folder on this computer.").size(12));
            panel = panel.push(
                button(text("Show recovery phrase"))
                    .width(Fill)
                    .on_press(Message::ShowPhrase),
            );
            if !self.recovery_phrase.is_empty() {
                panel = panel.push(text(&self.recovery_phrase).size(12));
                panel = panel.push(
                    button(text("Copy phrase"))
                        .width(Fill)
                        .on_press(Message::CopyPhrase),
                );
            }
            panel = panel.push(
                text_input("Paste phrase to restore", &self.restore_phrase)
                    .on_input(Message::RestorePhraseChanged),
            );
            panel = panel.push(
                button(text("Restore into new folder"))
                    .width(Fill)
                    .on_press(Message::StartRestore),
            );
        }
        panel = panel.push(text("3. Conversation").size(16));
        panel = panel.push(text("Share the link or the QR. A link is for text. A QR is for a person or device nearby. Paste their link or card below. Only one window presses Start conversation. Then Sync both.").size(12));
        panel = panel.push(text("Your invite").size(12));
        panel = panel.push(text(&self.contact_link).size(11));
        panel = panel.push(
            row![
                button(text("Copy link"))
                    .width(Fill)
                    .on_press(Message::CopyLink),
                button(text(if self.show_qr { "Hide QR" } else { "Show QR" }))
                    .width(Fill)
                    .on_press(Message::ToggleQr),
            ]
            .spacing(8),
        );
        if self.show_qr {
            if let Some(bytes) = &self.qr_bytes {
                panel = panel.push(
                    iced_image::Image::new(iced_image::Handle::from_bytes(bytes.clone()))
                        .width(200)
                        .height(200),
                );
            }
        }
        panel = panel.push(text("Their link or card").size(12));
        panel = panel.push(
            text_input("Paste their link or card", &self.their_card).on_input(Message::TheirCardChanged),
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


    fn show_phrase(&mut self) {
        match ambient_cli::recovery_phrase(&self.dir) {
            Ok(phrase) => {
                self.recovery_phrase = phrase;
                self.say("Recovery phrase shown. Copy it and store it offline.");
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
                    "Restored. That window holds the root. The notes from the sealed log should be there.",
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

    fn send_note_to_chat(&mut self) {
        let Some(id) = self.selected.clone() else {
            self.say("Select a note first.");
            return;
        };
        let Some(book) = self.notebook.as_ref() else {
            return;
        };
        match book.show(&id) {
            Ok(note) => {
                let draft = Draft::from_note(&note);
                let mut text = String::new();
                if !draft.title.is_empty() {
                    text.push_str(&draft.title);
                    text.push_str("\n\n");
                }
                text.push_str(&draft.body);
                match ambient_cli::send_chat(&self.dir, &text) {
                    Ok(()) => {
                        self.show_chat = true;
                        self.say("Note copied into the chat. Press Sync so the other window sees it.");
                        self.reload_chat();
                    }
                    Err(err) => self.message = err.to_string(),
                }
            }
            Err(err) => self.message = err.to_string(),
        }
    }

    fn submit_unlock(&mut self) {
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
                            self.reload_chat();
                            if let Ok(card) = ambient_cli::contact_card(&self.dir) {
                                self.my_card = card;
                            }
                            if let Ok(link) = ambient_cli::contact_link(&self.dir) {
                                self.contact_link = link;
                            }
                            self.say("Unlocked for this run. The key file on disk stays wrapped.");
                        }
                        Err(err) => self.open_error = err.to_string(),
                    }
                }
                Err(err) => self.message = err.to_string(),
            }
            return;
        }
        match ambient_cli::set_folder_unlock(&self.dir, &passphrase) {
            Ok(()) => {
                self.unlock_input.clear();
                self.say("Key file wrapped. The passphrase is not the root and not the recovery phrase.");
            }
            Err(err) => self.message = err.to_string(),
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
