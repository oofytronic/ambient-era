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
use iced::widget::image as iced_image;
use iced::widget::{button, column, container, row, scrollable, text, text_editor, text_input};
use iced::{Element, Fill, Task, Theme};
use image::ImageEncoder;
use qrcode::QrCode;
use std::io::Cursor;

fn main() -> iced::Result {
    iced::application(App::boot, App::update, App::view)
        .title(App::title)
        .subscription(App::subscription)
        .window_size((1280.0, 820.0))
        .theme(theme)
        .run()
}

fn theme(_app: &App) -> Theme {
    Theme::Dark
}

#[allow(clippy::struct_excessive_bools)]
struct App {
    dir: PathBuf,
    notebook: Option<Notebook>,
    open_error: String,
    notes: Vec<NoteSummary>,
    query: String,
    selected: Option<String>,
    title_text: String,
    body: text_editor::Content,
    conflicts: Vec<(String, String, String)>,
    note_writer: String,
    message: String,
    show_chat: bool,
    chats: Vec<ambient_cli::ConversationSummary>,
    chat_id: Option<String>,
    devices: Vec<ambient_cli::DeviceLine>,
    chat: Vec<String>,
    draft_chat: String,
    my_card: String,
    their_card: String,
    relay_ready: bool,
    relay_ours: bool,
    sync_open: bool,
    sync_detail: String,
    pending: bool,
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
    AutoSync,
    ToggleSyncPop,
    ToggleRelay,
    OpenPaired,
    OpenPerson,
    #[allow(dead_code)]
    CopyCard,
    #[allow(dead_code)]
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
    ShowInvite,
    ShowDevices,
    ShowRecovery,
    CloseSheet,
    SelectChat(String),
    Revoke(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sheet {
    None,
    Invite,
    Devices,
    Recovery,
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
            note_writer: String::new(),
            message: String::new(),
            show_chat: false,
            chats: Vec::new(),
            chat_id: None,
            devices: Vec::new(),
            chat: Vec::new(),
            draft_chat: String::new(),
            my_card: String::new(),
            their_card: String::new(),
            relay_ready: false,
            relay_ours: false,
            sync_open: false,
            sync_detail: "Relay is starting.".to_owned(),
            pending: false,
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
                    app.say("Paired device. It does not hold the root. It syncs on its own while the relay is on.");
                }
                app.pending = ambient_cli::has_unsent(&app.dir).unwrap_or(false);
            }
            Err(err) => app.open_error = err,
        }
        if app.open_error != "this folder is locked" {
            app.start_relay();
        }
        (app, Task::done(Message::AutoSync))
    }

    fn subscription(app: &App) -> iced::Subscription<Message> {
        if !app.relay_ready {
            return iced::Subscription::none();
        }
        iced::time::every(std::time::Duration::from_secs(3)).map(|_| Message::AutoSync)
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
                self.sheet = Sheet::None;
                Task::none()
            }
            Message::ShowChat => {
                self.remember_unsaved();
                self.show_chat = true;
                self.sheet = Sheet::None;
                self.reload_chat();
                Task::none()
            }
            Message::AutoSync => {
                self.sync_quiet();
                Task::none()
            }
            Message::ToggleSyncPop => {
                self.sync_open = !self.sync_open;
                Task::none()
            }
            Message::ToggleRelay => {
                if self.relay_ready && !self.relay_ours {
                    "This window did not start the relay.".clone_into(&mut self.sync_detail);
                } else if self.relay_ready {
                    self.stop_relay();
                } else {
                    self.start_relay();
                    self.sync_quiet();
                }
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
            Message::SelectChat(id) => {
                self.chat_id = Some(id);
                self.show_chat = true;
                self.sheet = Sheet::None;
                self.reload_chat();
                Task::none()
            }
            Message::Revoke(id) => {
                self.revoke(&id);
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
            Message::ShowInvite => {
                if let Ok(link) = ambient_cli::contact_link(&self.dir) {
                    if link != self.contact_link {
                        self.show_qr = false;
                        self.qr_bytes = None;
                    }
                    self.contact_link = link;
                }
                self.sheet = Sheet::Invite;
                Task::none()
            }
            Message::ShowDevices => {
                self.devices = self
                    .notebook
                    .as_ref()
                    .and_then(|book| book.devices().ok())
                    .unwrap_or_default();
                self.sheet = Sheet::Devices;
                Task::none()
            }
            Message::ShowRecovery => {
                self.sheet = Sheet::Recovery;
                Task::none()
            }
            Message::CloseSheet => {
                self.sheet = Sheet::None;
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        if self.open_error == "this folder is locked" {
            return self.unlock_screen();
        }
        column![
            self.toolbar(),
            self.sync_pop(),
            row![self.sidebar(), self.main()].height(Fill)
        ]
        .height(Fill)
        .into()
    }

    fn toolbar(&self) -> Element<'_, Message> {
        let role = match self.notebook.as_ref() {
            Some(book) if book.holds_root() => "Holds the root",
            Some(_) => "Paired device",
            None => "Not open",
        };
        let mut bar = row![
            text("Ambient").size(20),
            text(role).size(12),
            text(self.message.as_str()).size(12).width(Fill),
            button(text("Devices")).on_press(Message::ShowDevices),
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center)
        .padding(12);
        if self.notebook.as_ref().is_some_and(Notebook::holds_root) {
            bar = bar.push(button(text("Recovery")).on_press(Message::ShowRecovery));
        }
        bar = bar.push(
            button(
                iced_image::Image::new(globe_handle(self.globe_color()))
                    .width(28)
                    .height(28),
            )
            .on_press(Message::ToggleSyncPop)
            .style(globe_button),
        );
        bar.into()
    }

    fn globe_color(&self) -> [u8; 3] {
        if !self.relay_ready {
            [138, 143, 152]
        } else if self.pending {
            [232, 93, 76]
        } else {
            [61, 220, 151]
        }
    }

    fn sync_pop(&self) -> Element<'_, Message> {
        if !self.sync_open {
            return column![].into();
        }
        let relay = if !self.relay_ready {
            "Relay is off."
        } else if self.relay_ours {
            "Relay is on. Cleartext on this computer only. Not a public server."
        } else {
            "Relay is on, started by another window. Cleartext on this computer only."
        };
        let switch = if self.relay_ready && self.relay_ours {
            "Turn relay off"
        } else if self.relay_ready {
            "Relay is not this window's"
        } else {
            "Turn relay on"
        };
        let state = if !self.relay_ready {
            "Not syncing."
        } else if self.pending {
            "Changes have not synced."
        } else {
            "Synced."
        };
        let cadence = if self.relay_ready {
            "Syncs on its own every few seconds."
        } else {
            "Turn the relay on to sync."
        };
        row![
            column![].width(Fill),
            container(
                column![
                    text(state).size(16),
                    text(cadence).size(12),
                    text(relay).size(12),
                    text(self.sync_detail.as_str()).size(12),
                    button(text(switch)).on_press(Message::ToggleRelay),
                ]
                .spacing(8)
                .width(320),
            )
            .padding(12)
            .width(320)
            .style(|_theme| container::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgb(
                    0.12, 0.12, 0.12
                ))),
                border: iced::Border {
                    radius: 8.0.into(),
                    width: 1.0,
                    color: iced::Color::from_rgb(0.3, 0.3, 0.3),
                },
                ..Default::default()
            }),
        ]
        .padding([0, 16])
        .into()
    }

    fn sidebar(&self) -> Element<'_, Message> {
        let notes_style = if self.show_chat { "Notes" } else { "Notes ·" };
        let chat_style = if self.show_chat { "Chat ·" } else { "Chat" };
        let mut side = column![
            row![
                button(text(notes_style))
                    .on_press(Message::ShowNotes)
                    .width(Fill),
                button(text(chat_style))
                    .on_press(Message::ShowChat)
                    .width(Fill),
            ]
            .spacing(8),
        ]
        .spacing(12)
        .padding(16)
        .width(260)
        .height(Fill);
        if self.show_chat {
            side = side.push(
                button(text("Invite"))
                    .width(Fill)
                    .on_press(Message::ShowInvite),
            );
            if self.chats.is_empty() {
                side = side.push(text("No conversations yet.").size(12));
            }
            for chat in &self.chats {
                let mut label = chat.label.clone();
                if self.chat_id.as_deref() == Some(chat.id.as_str()) {
                    label.insert_str(0, "● ");
                }
                side = side.push(
                    button(text(label))
                        .width(Fill)
                        .on_press(Message::SelectChat(chat.id.clone())),
                );
            }
            return side.into();
        }
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
            if !note.writer.is_empty() {
                label.push_str("  ");
                label.push_str(&note.writer);
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
        side = side.push(text_input("Search", &self.query).on_input(Message::QueryChanged));
        side = side.push(
            button(text("New note"))
                .width(Fill)
                .on_press(Message::NewNote),
        );
        side = side.push(scrollable(notes).height(Fill));
        side.into()
    }

    fn main(&self) -> Element<'_, Message> {
        let body = match self.sheet {
            Sheet::Invite => self.invite_sheet(),
            Sheet::Devices => self.devices_sheet(),
            Sheet::Recovery => self.recovery_sheet(),
            Sheet::None if self.show_chat => self.chat_pane(),
            Sheet::None => self.notes_pane(),
        };
        container(body).padding(24).width(Fill).height(Fill).into()
    }

    fn unlock_screen(&self) -> Element<'_, Message> {
        column![
            text("This folder is locked").size(22),
            text("The passphrase unwraps the key file for this run. It is not the root, not the recovery phrase, and not a platform passkey.").size(12),
            text_input("Passphrase", &self.unlock_input)
                .on_input(Message::UnlockChanged)
                .on_submit(Message::SubmitUnlock)
                .secure(true),
            button(text("Unlock")).on_press(Message::SubmitUnlock),
            text(self.message.as_str()),
        ]
        .spacing(12)
        .padding(24)
        .into()
    }

    fn notes_pane(&self) -> Element<'_, Message> {
        let mut body = column![].spacing(12).height(Fill);
        if !self.open_error.is_empty() {
            body = body.push(text(format!(
                "Could not open this folder. {}",
                self.open_error
            )));
        }
        if !self.conflicts.is_empty() {
            let mut versions = column![text(
                "Two versions. Write the one you want to keep, then save."
            )]
            .spacing(8);
            for (index, (writer, title, note_body)) in self.conflicts.iter().enumerate() {
                let number = index.saturating_add(1);
                versions = versions.push(text(format!("Version {number} · {writer}: {title}")));
                versions = versions.push(text(note_body));
            }
            body = body.push(scrollable(versions).height(160));
        }
        if !self.note_writer.is_empty() && self.conflicts.is_empty() {
            body = body.push(text(format!("Written by {}", self.note_writer)).size(12));
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
        let mut actions = row![button(text("Save")).on_press(Message::Save)].spacing(8);
        if self.selected.is_some() {
            actions = actions.push(button(text("Send to chat")).on_press(Message::SendNoteToChat));
        }
        body = body.push(actions);
        body.into()
    }

    fn chat_pane(&self) -> Element<'_, Message> {
        let mut lines = column![].spacing(10);
        if self.chat.is_empty() {
            lines = lines.push(text("No messages yet."));
            lines = lines.push(button(text("Invite someone")).on_press(Message::ShowInvite));
        }
        for line in &self.chat {
            lines = lines.push(
                container(text(line))
                    .padding(8)
                    .style(|_theme| container::Style {
                        background: Some(iced::Background::Color(iced::Color::from_rgb(
                            0.15, 0.15, 0.15,
                        ))),
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
        ]
        .spacing(12)
        .height(Fill)
        .into()
    }

    fn invite_sheet(&self) -> Element<'_, Message> {
        let mut sheet = column![
            Self::sheet_title("Invite"),
            text("Share your link, or paste theirs. A link is for text. A QR is for someone nearby. Only one side starts the conversation.").size(12),
            text("Your link").size(12),
            text(&self.contact_link).size(12),
            row![
                button(text("Copy link")).on_press(Message::CopyLink),
                button(text(if self.show_qr { "Hide QR" } else { "Show QR" })).on_press(Message::ToggleQr),
            ]
            .spacing(8),
        ]
        .spacing(12);
        if self.show_qr
            && let Some(bytes) = &self.qr_bytes
        {
            sheet = sheet.push(
                iced_image::Image::new(iced_image::Handle::from_bytes(bytes.clone()))
                    .width(200)
                    .height(200),
            );
        }
        sheet = sheet.push(
            text_input("Their link or card", &self.their_card).on_input(Message::TheirCardChanged),
        );
        sheet = sheet.push(button(text("Start conversation")).on_press(Message::StartConversation));
        scrollable(sheet).height(Fill).into()
    }

    fn devices_sheet(&self) -> Element<'_, Message> {
        let mut sheet = column![
            Self::sheet_title("Devices"),
            text(self.dir.display().to_string()).size(12),
            text("Unlock").size(16),
            text("Wraps the key file. Not the root, and not a platform passkey.").size(12),
            text_input("Passphrase", &self.unlock_input)
                .on_input(Message::UnlockChanged)
                .secure(true),
            button(text("Wrap key file")).on_press(Message::SubmitUnlock),
        ]
        .spacing(12);
        sheet = sheet.push(text("Devices").size(16));
        if self.devices.is_empty() {
            sheet = sheet.push(text("No devices yet.").size(12));
        }
        for device in &self.devices {
            let state = if device.revoked {
                "revoked"
            } else if device.current {
                "this device"
            } else {
                "active"
            };
            sheet = sheet.push(text(format!("{} · {state}", device.label)).size(14));
            if self.notebook.as_ref().is_some_and(Notebook::holds_root)
                && !device.current
                && !device.revoked
            {
                sheet = sheet.push(
                    button(text(format!("Revoke {}", device.label)))
                        .on_press(Message::Revoke(device.id.clone())),
                );
            }
        }
        if self.notebook.as_ref().is_some_and(Notebook::holds_root) {
            sheet = sheet.push(text("A paired folder does not hold the root. Revoke stops future seals. It does not erase notes that device already opened.").size(12));
            sheet = sheet.push(button(text("Open paired window")).on_press(Message::OpenPaired));
            sheet = sheet.push(button(text("Open the other person")).on_press(Message::OpenPerson));
        }
        scrollable(sheet).height(Fill).into()
    }

    fn recovery_sheet(&self) -> Element<'_, Message> {
        let mut sheet = column![
            Self::sheet_title("Recovery"),
            text("The phrase rebuilds the root on a new folder. It does not recreate the old device keys. The new folder adds itself so it can sync. Revoke the devices you no longer have. This is not social recovery.").size(12),
            button(text("Show recovery phrase")).on_press(Message::ShowPhrase),
        ]
        .spacing(12);
        if !self.recovery_phrase.is_empty() {
            sheet = sheet.push(text(&self.recovery_phrase).size(12));
            sheet = sheet.push(button(text("Copy phrase")).on_press(Message::CopyPhrase));
        }
        sheet = sheet.push(
            text_input("Paste phrase to restore", &self.restore_phrase)
                .on_input(Message::RestorePhraseChanged),
        );
        sheet = sheet.push(button(text("Restore into new folder")).on_press(Message::StartRestore));
        scrollable(sheet).height(Fill).into()
    }

    fn sheet_title(title: &str) -> Element<'static, Message> {
        let title = title.to_owned();
        row![
            text(title).size(22),
            button(text("Back")).on_press(Message::CloseSheet),
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center)
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
        self.chats = ambient_cli::list_conversations(&self.dir).unwrap_or_default();
        let known = self
            .chat_id
            .as_ref()
            .is_some_and(|id| self.chats.iter().any(|chat| &chat.id == id));
        if !known {
            self.chat_id = self.chats.first().map(|chat| chat.id.clone());
        }
        let Some(id) = self.chat_id.clone() else {
            self.chat.clear();
            return;
        };
        match ambient_cli::chat_lines_in(&self.dir, &id) {
            Ok(lines) => self.chat = lines,
            Err(err) => self.message = err.to_string(),
        }
    }

    fn show_note(&mut self, id: &str) {
        if self.selected.as_deref() == Some(id)
            && (self.title_text != self.disk_title || self.body.text() != self.disk_body)
        {
            self.show_chat = false;
            self.sheet = Sheet::None;
            return;
        }
        if self.selected.as_deref() != Some(id) {
            self.remember_unsaved();
        }
        let stash = self
            .unsaved
            .iter()
            .find(|(key, _, _)| key.as_deref() == Some(id))
            .map(|(_, title, body)| (title.clone(), body.clone()));
        let Some(book) = self.notebook.as_ref() else {
            return;
        };
        match book.show(id) {
            Ok(note) => {
                let draft = Draft::from_note(&note);
                self.selected = Some(id.to_owned());
                self.disk_title.clone_from(&draft.title);
                self.disk_body.clone_from(&draft.body);
                if let Some((title, body)) = stash {
                    self.title_text = title;
                    self.body = text_editor::Content::with_text(&body);
                } else {
                    self.title_text.clone_from(&draft.title);
                    self.body = text_editor::Content::with_text(&draft.body);
                }
                self.conflicts = draft.conflicts;
                self.note_writer = note
                    .heads
                    .first()
                    .map(|head| head.writer.clone())
                    .unwrap_or_default();
                self.show_chat = false;
                self.sheet = Sheet::None;
                self.message.clear();
            }
            Err(err) => self.message = err.to_string(),
        }
    }

    fn clear_editor(&mut self) {
        self.remember_unsaved();
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
        self.show_chat = false;
        self.sheet = Sheet::None;
        if let Some((title, body)) = stash {
            self.title_text = title;
            self.body = text_editor::Content::with_text(&body);
            return;
        }
        self.title_text.clear();
        self.body = text_editor::Content::new();
        self.message.clear();
    }

    fn remember_unsaved(&mut self) {
        if self.title_text == self.disk_title && self.body.text() == self.disk_body {
            return;
        }
        let key = self.selected.clone();
        self.unsaved.retain(|(id, _, _)| id != &key);
        self.unsaved
            .push((key, self.title_text.clone(), self.body.text()));
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
                self.unsaved
                    .retain(|(key, _, _)| key.as_deref() != Some(id.as_str()));
                if current.is_none() {
                    self.unsaved.retain(|(key, _, _)| key.is_some());
                }
                self.disk_title.clone_from(&self.title_text);
                self.disk_body = self.body.text();
                self.message.clear();
                self.reload();
                self.show_note(&id);
                self.mark_pending();
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
                "Relay stopped.".clone_into(&mut self.sync_detail);
            }
            Err(err) => self.sync_detail = err.to_string(),
        }
    }

    fn sync_quiet(&mut self) {
        if !self.relay_ready {
            return;
        }
        match ambient_cli::sync_folder(&self.dir, ambient_cli::LOOPBACK_URL) {
            Ok(detail) => {
                self.sync_detail = detail;
                match Notebook::open(&self.dir) {
                    Ok(book) => {
                        self.notebook = Some(book);
                        self.reload();
                    }
                    Err(err) => self.sync_detail = err.to_string(),
                }
                self.reload_chat();
                self.pending = ambient_cli::has_unsent(&self.dir).unwrap_or(true);
            }
            Err(err) => {
                let text = err.to_string();
                if text.contains("unreachable") && !self.relay_ours {
                    self.relay_ready = false;
                }
                self.sync_detail = text;
            }
        }
    }

    fn mark_pending(&mut self) {
        self.pending = ambient_cli::has_unsent(&self.dir).unwrap_or(true);
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
                self.chat_id = Some(id);
                self.show_chat = true;
                self.sheet = Sheet::None;
                self.their_card.clear();
                self.say("Conversation saved here. Both windows sync while the relay is on.");
                self.reload_chat();
                self.mark_pending();
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
        let Some(id) = self.chat_id.clone() else {
            self.say("Invite someone before sending.");
            return;
        };
        match ambient_cli::send_chat_in(&self.dir, &id, &text) {
            Ok(()) => {
                self.draft_chat.clear();
                self.show_chat = true;
                self.say("Saved here. It syncs while the relay is on.");
                self.reload_chat();
                self.mark_pending();
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
                match self.chat_id.clone() {
                    Some(chat) => match ambient_cli::send_chat_in(&self.dir, &chat, &text) {
                        Ok(()) => {
                            self.remember_unsaved();
                            self.show_chat = true;
                            self.say("Note copied into the chat. It syncs while the relay is on.");
                            self.reload_chat();
                            self.mark_pending();
                        }
                        Err(err) => self.message = err.to_string(),
                    },
                    None => self.say("Start a conversation before sending a note."),
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
                            self.pending = ambient_cli::has_unsent(&self.dir).unwrap_or(false);
                            self.start_relay();
                            self.sync_quiet();
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
                self.say(
                    "Key file wrapped. The passphrase is not the root and not the recovery phrase.",
                );
            }
            Err(err) => self.message = err.to_string(),
        }
    }
}

fn globe_button(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: None,
        ..Default::default()
    }
}

fn globe_handle(color: [u8; 3]) -> iced_image::Handle {
    use std::sync::OnceLock;
    static HANDLES: OnceLock<[iced_image::Handle; 3]> = OnceLock::new();
    let handles = HANDLES.get_or_init(|| {
        [
            iced_image::Handle::from_bytes(globe_png([138, 143, 152])),
            iced_image::Handle::from_bytes(globe_png([232, 93, 76])),
            iced_image::Handle::from_bytes(globe_png([61, 220, 151])),
        ]
    });
    if color == [232, 93, 76] {
        handles[1].clone()
    } else if color == [61, 220, 151] {
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
