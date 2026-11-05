//! Linux notes window.
//!
//! Iced draws the controls in a system window. There is no webview. The folder
//! is the same one the `ambient` CLI uses. Pass a path, or it uses `./notebook`.
//! Sync, pairing, and chat call `ambient-cli`. This file does not seal events.
//! The relay it can start is cleartext on `127.0.0.1` only.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use ambient_cli::{Draft, NoteSummary, Notebook};
use iced::clipboard;
use iced::keyboard::Key;
use iced::widget::image as iced_image;
use iced::widget::{button, column, container, row, scrollable, text, text_editor, text_input};
use iced::{Color, Element, Fill, Font, Task, Theme};
use image::ImageEncoder;
use qrcode::QrCode;

const SANS_BYTES: &[u8] = include_bytes!("../fonts/DMSans-Regular.ttf");
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
        .font(SERIF_BYTES)
        .run()
}

fn theme(app: &App) -> Theme {
    let ink = Ink::from_dark(app.dark);
    Theme::custom(
        if app.dark {
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
    dark: bool,
    syncing: bool,
    edited_at: Option<Instant>,
    writes: u64,
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
    ToggleTheme,
    Synced(SyncOutcome),
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
            dark: true,
            syncing: false,
            edited_at: None,
            writes: 0,
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
                self.edited_at = Some(Instant::now());
                Task::none()
            }
            Message::BodyEdited(action) => {
                self.body.perform(action);
                self.edited_at = Some(Instant::now());
                Task::none()
            }
            Message::Save => {
                self.save_keeping_editor();
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
            Message::SubmitUnlock => self.submit_unlock(),
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
            Message::ToggleTheme => {
                self.dark = !self.dark;
                Task::none()
            }
            Message::Synced(outcome) => self.finish_sync(outcome),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        if self.open_error == "this folder is locked" {
            return self.unlock_screen();
        }
        column![
            self.toolbar(),
            hairline(),
            row![self.sidebar(), hairline_v(), self.main()].height(Fill)
        ]
        .height(Fill)
        .into()
    }

    fn toolbar(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let role = match self.notebook.as_ref() {
            Some(book) if book.holds_root() => "Holds the root",
            Some(_) => "Paired device",
            None => "Not open",
        };
        let mut bar = row![
            text("Ambient").font(serif()).size(24),
            text(role).size(12).color(ink.muted),
            text(self.message.as_str())
                .size(13)
                .color(ink.muted)
                .width(Fill),
            ghost_label("Devices", Message::ShowDevices),
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center)
        .padding([10.0, 16.0]);
        if self.notebook.as_ref().is_some_and(Notebook::holds_root) {
            bar = bar.push(ghost_label("Recovery", Message::ShowRecovery));
        }
        bar = bar.push(icon_button(mode_handle(self.dark), Message::ToggleTheme));
        bar = bar.push(anchored(
            icon_button(globe_handle(self.globe_color()), Message::ToggleSyncPop),
            self.sync_open.then(|| self.sync_card()),
        ));
        container(bar).width(Fill).into()
    }

    fn globe_color(&self) -> [u8; 3] {
        if !self.relay_ready {
            [140, 133, 122]
        } else if self.pending {
            [184, 78, 62]
        } else {
            [46, 140, 96]
        }
    }

    fn sync_card(&self) -> Element<'_, Message> {
        let ink = self.ink();
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
        let line = iced::Length::Fixed(276.0);
        let switch_button = if self.relay_ready && !self.relay_ours {
            ghost_label(switch, Message::ToggleRelay)
        } else {
            action_label(switch, Message::ToggleRelay)
        };
        container(
            column![
                text(state).font(serif()).size(20).width(line),
                text(cadence).size(13).color(ink.muted).width(line),
                text(relay).size(13).color(ink.muted).width(line),
                text(self.sync_detail.as_str())
                    .size(13)
                    .color(ink.text)
                    .width(line),
                switch_button,
            ]
            .spacing(8),
        )
        .padding(14)
        .width(300)
        .style(card_style)
        .into()
    }

    fn sidebar(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let modes = container(
            row![
                segment("Notes", !self.show_chat, Message::ShowNotes),
                segment("Chat", self.show_chat, Message::ShowChat),
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
        if self.show_chat {
            side = side.push(action_fill("Invite", Message::ShowInvite));
            if self.chats.is_empty() {
                side = side.push(text("No conversations yet.").size(13).color(ink.muted));
            }
            for chat in &self.chats {
                let selected = self.chat_id.as_deref() == Some(chat.id.as_str());
                side = side.push(chat_row(&chat.label, &chat.id, selected));
            }
        } else {
            side = side.push(
                text_input("Search notes", &self.query)
                    .on_input(Message::QueryChanged)
                    .padding(10)
                    .size(14)
                    .style(field_style),
            );
            side = side.push(action_fill("New note", Message::NewNote));
            if self.notes.is_empty() {
                let empty = if self.query.is_empty() {
                    "No notes yet."
                } else {
                    "No notes match."
                };
                side = side.push(text(empty).size(13).color(ink.muted));
            }
            let mut notes = column![].spacing(2);
            for note in &self.notes {
                notes = notes.push(self.note_row(note));
            }
            side = side.push(scrollable(notes).height(Fill));
        }
        container(side)
            .width(280)
            .height(Fill)
            .style(surface_pane)
            .into()
    }

    fn note_row(&self, note: &NoteSummary) -> Element<'_, Message> {
        let ink = self.ink();
        let selected = self.selected.as_deref() == Some(note.id.as_str());
        let title = if note.title.is_empty() {
            "Untitled".to_owned()
        } else {
            note.title.clone()
        };
        let mut lines = column![text(title).font(serif()).size(17).width(Fill)].spacing(2);
        let mut meta = note.writer.clone();
        if note.conflict {
            if !meta.is_empty() {
                meta.push_str(" · ");
            }
            meta.push_str("conflict");
        }
        if !meta.is_empty() {
            lines = lines.push(text(meta).size(12).color(ink.muted).width(Fill));
        }
        button(lines)
            .width(Fill)
            .padding([8.0, 10.0])
            .on_press(Message::Select(note.id.clone()))
            .style(list_row(selected))
            .into()
    }

    fn main(&self) -> Element<'_, Message> {
        let body = match self.sheet {
            Sheet::Invite => self.invite_sheet(),
            Sheet::Devices => self.devices_sheet(),
            Sheet::Recovery => self.recovery_sheet(),
            Sheet::None if self.show_chat => self.chat_pane(),
            Sheet::None => self.notes_pane(),
        };
        container(body)
            .padding([22.0, 32.0])
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
            body = body.push(scrollable(versions).height(160));
        }
        if !self.note_writer.is_empty() && self.conflicts.is_empty() {
            body = body.push(
                text(format!("Written by {}", self.note_writer))
                    .size(13)
                    .color(ink.muted),
            );
        }
        body = body.push(
            text_input("Title", &self.title_text)
                .on_input(Message::TitleChanged)
                .font(serif())
                .size(32)
                .padding([2.0, 0.0])
                .style(title_style),
        );
        body = body.push(
            text_editor(&self.body)
                .height(Fill)
                .padding(14)
                .size(16)
                .font(sans())
                .placeholder("Write")
                .on_action(Message::BodyEdited)
                .key_binding(binding)
                .style(editor_style),
        );
        let mut actions = row![action_label("Save", Message::Save)].spacing(8);
        if self.selected.is_some() {
            actions = actions.push(ghost_label("Send to chat", Message::SendNoteToChat));
        }
        body = body.push(actions);
        body.into()
    }

    fn chat_pane(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let mut lines = column![].spacing(10);
        if self.chat.is_empty() {
            lines = lines.push(text("No messages yet.").color(ink.muted));
            lines = lines.push(action_label("Invite someone", Message::ShowInvite));
        }
        for line in &self.chat {
            lines = lines.push(
                container(text(line).size(15))
                    .padding([10.0, 12.0])
                    .style(bubble_style),
            );
        }
        column![
            scrollable(lines).height(Fill),
            text_input("Message", &self.draft_chat)
                .on_input(Message::DraftChat)
                .on_submit(Message::SendChat)
                .padding(10)
                .style(field_style),
            action_label("Send", Message::SendChat),
        ]
        .spacing(12)
        .height(Fill)
        .into()
    }

    fn invite_sheet(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let mut sheet = column![
            Self::sheet_title("Invite"),
            text("Share your link, or paste theirs. A link is for text. A QR is for someone nearby. Only one side starts the conversation.").size(14).color(ink.muted),
            text("Your link").size(12).color(ink.muted),
            text(&self.contact_link).size(14),
            row![
                ghost_label("Copy link", Message::CopyLink),
                ghost_label(if self.show_qr { "Hide QR" } else { "Show QR" }, Message::ToggleQr),
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
            text_input("Their link or card", &self.their_card)
                .on_input(Message::TheirCardChanged)
                .padding(10)
                .style(field_style),
        );
        sheet = sheet.push(action_label(
            "Start conversation",
            Message::StartConversation,
        ));
        scrollable(sheet).height(Fill).into()
    }

    fn devices_sheet(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let mut sheet = column![
            Self::sheet_title("Devices"),
            text(self.dir.display().to_string())
                .size(13)
                .color(ink.muted),
            text("Unlock").font(serif()).size(22),
            text("Wraps the key file. Not the root, and not a platform passkey.")
                .size(14)
                .color(ink.muted),
            text_input("Passphrase", &self.unlock_input)
                .on_input(Message::UnlockChanged)
                .secure(true)
                .padding(10)
                .style(field_style),
            action_label("Wrap key file", Message::SubmitUnlock),
        ]
        .spacing(12);
        sheet = sheet.push(text("Devices").font(serif()).size(22));
        if self.devices.is_empty() {
            sheet = sheet.push(text("No devices yet.").size(13).color(ink.muted));
        }
        for device in &self.devices {
            let state = if device.revoked {
                "revoked"
            } else if device.current {
                "this device"
            } else {
                "active"
            };
            sheet = sheet.push(text(format!("{} · {state}", device.label)).size(15));
            if self.notebook.as_ref().is_some_and(Notebook::holds_root)
                && !device.current
                && !device.revoked
            {
                let label = format!("Revoke {}", device.label);
                sheet = sheet.push(ghost_label_owned(label, Message::Revoke(device.id.clone())));
            }
        }
        if self.notebook.as_ref().is_some_and(Notebook::holds_root) {
            sheet = sheet.push(text("A paired folder does not hold the root. Revoke stops future seals. It does not erase notes that device already opened.").size(14).color(ink.muted));
            sheet = sheet.push(ghost_label("Open paired window", Message::OpenPaired));
            sheet = sheet.push(ghost_label("Open the other person", Message::OpenPerson));
        }
        scrollable(sheet).height(Fill).into()
    }

    fn recovery_sheet(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let mut sheet = column![
            Self::sheet_title("Recovery"),
            text("The phrase rebuilds the root on a new folder. It does not recreate the old device keys. The new folder adds itself so it can sync. Revoke the devices you no longer have. This is not social recovery.").size(14).color(ink.muted),
            action_label("Show recovery phrase", Message::ShowPhrase),
        ]
        .spacing(12);
        if !self.recovery_phrase.is_empty() {
            sheet = sheet.push(text(&self.recovery_phrase).size(15));
            sheet = sheet.push(ghost_label("Copy phrase", Message::CopyPhrase));
        }
        sheet = sheet.push(
            text_input("Paste phrase to restore", &self.restore_phrase)
                .on_input(Message::RestorePhraseChanged)
                .padding(10)
                .style(field_style),
        );
        sheet = sheet.push(action_label(
            "Restore into new folder",
            Message::StartRestore,
        ));
        scrollable(sheet).height(Fill).into()
    }

    fn sheet_title(title: &str) -> Element<'static, Message> {
        let title = title.to_owned();
        row![
            text(title).font(serif()).size(32),
            ghost_label("Back", Message::CloseSheet),
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
        if self.selected.as_deref() == Some(id) && self.editor_dirty() {
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
                    self.edited_at = Some(Instant::now());
                } else {
                    self.title_text.clone_from(&draft.title);
                    self.body = text_editor::Content::with_text(&draft.body);
                    self.edited_at = None;
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
            self.edited_at = Some(Instant::now());
            return;
        }
        self.title_text.clear();
        self.body = text_editor::Content::new();
        self.edited_at = None;
        self.message.clear();
    }

    fn remember_unsaved(&mut self) {
        if !self.editor_dirty() {
            return;
        }
        let key = self.selected.clone();
        self.unsaved.retain(|(id, _, _)| id != &key);
        self.unsaved
            .push((key, self.title_text.clone(), self.body.text()));
    }

    fn editor_dirty(&self) -> bool {
        self.title_text != self.disk_title || self.body.text() != self.disk_body
    }

    fn ink(&self) -> Ink {
        Ink::from_dark(self.dark)
    }

    fn save_keeping_editor(&mut self) {
        let body = self.body.text();
        let title = self.title_text.clone();
        let current = self.selected.clone();
        let saved = {
            let _guard = io_lock();
            let Some(book) = self.notebook.as_ref() else {
                return;
            };
            match current.as_deref() {
                Some(id) => book.edit(id, &title, &body).map(|()| id.to_owned()),
                None => book.add(&title, &body),
            }
        };
        match saved {
            Ok(id) => {
                self.unsaved
                    .retain(|(key, _, _)| key.as_deref() != Some(id.as_str()));
                if current.is_none() {
                    self.unsaved.retain(|(key, _, _)| key.is_some());
                }
                self.selected = Some(id);
                self.disk_title.clone_from(&self.title_text);
                self.disk_body = body;
                self.edited_at = None;
                self.conflicts.clear();
                self.message.clear();
                self.reload();
                self.writes = self.writes.saturating_add(1);
                self.pending = true;
                if self.note_writer.is_empty()
                    && let Some(note) = self
                        .notes
                        .iter()
                        .find(|note| self.selected.as_deref() == Some(note.id.as_str()))
                {
                    self.note_writer.clone_from(&note.writer);
                }
            }
            Err(err) => self.message = err.to_string(),
        }
    }

    fn begin_sync(&mut self) -> Task<Message> {
        if self.syncing || !self.relay_ready {
            return Task::none();
        }
        if self.editor_dirty()
            && self
                .edited_at
                .is_some_and(|at| at.elapsed() >= Duration::from_millis(700))
        {
            self.save_keeping_editor();
        }
        self.syncing = true;
        sync_task(self.dir.clone(), self.writes)
    }

    fn finish_sync(&mut self, outcome: SyncOutcome) -> Task<Message> {
        self.syncing = false;
        if outcome.failed {
            if outcome.detail.contains("unreachable") && !self.relay_ours {
                self.relay_ready = false;
            }
            self.sync_detail = outcome.detail;
            return Task::none();
        }
        self.sync_detail = outcome.detail;
        match Notebook::open(&self.dir) {
            Ok(book) => {
                self.notebook = Some(book);
                self.reload();
                self.refresh_open_note();
            }
            Err(err) => self.sync_detail = err.to_string(),
        }
        self.reload_chat();
        self.pending = outcome.pending || self.writes != outcome.writes;
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
        let Ok(note) = book.show(&id) else {
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
        self.body = text_editor::Content::with_text(&draft.body);
        self.edited_at = None;
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
                            self.reload_chat();
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
    writes: u64,
}

fn io_lock() -> std::sync::MutexGuard<'static, ()> {
    static IO: Mutex<()> = Mutex::new(());
    IO.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn run_sync(dir: &Path) -> SyncOutcome {
    let _guard = io_lock();
    match ambient_cli::sync_folder(dir, ambient_cli::LOOPBACK_URL) {
        Ok(detail) => SyncOutcome {
            pending: ambient_cli::has_unsent(dir).unwrap_or(true),
            detail,
            failed: false,
            writes: 0,
        },
        Err(err) => SyncOutcome {
            detail: err.to_string(),
            pending: true,
            failed: true,
            writes: 0,
        },
    }
}

fn sync_task(dir: PathBuf, writes: u64) -> Task<Message> {
    Task::perform(
        async move {
            let (sender, receiver) = iced::futures::channel::oneshot::channel();
            std::thread::spawn(move || {
                let _ = sender.send(run_sync(&dir));
            });
            let mut outcome = receiver.await.unwrap_or_else(|_| SyncOutcome {
                detail: "Sync stopped before it finished.".to_owned(),
                pending: true,
                failed: true,
                writes: 0,
            });
            outcome.writes = writes;
            outcome
        },
        Message::Synced,
    )
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
        background: iced::Background::Color(ink.field),
        border: iced::Border {
            radius: 10.0.into(),
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
        .padding([8.0, 14.0])
        .on_press(message)
        .style(action_style)
        .into()
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

fn segment(label: &str, active: bool, message: Message) -> Element<'_, Message> {
    button(text(label).size(14).width(Fill))
        .width(Fill)
        .padding([8.0, 10.0])
        .on_press(message)
        .style(segment_style(active))
        .into()
}

fn chat_row(label: &str, id: &str, selected: bool) -> Element<'static, Message> {
    let label = label.to_owned();
    let id = id.to_owned();
    button(text(label).size(15).width(Fill))
        .width(Fill)
        .padding([8.0, 10.0])
        .on_press(Message::SelectChat(id))
        .style(list_row(selected))
        .into()
}

fn icon_button(handle: iced_image::Handle, message: Message) -> Element<'static, Message> {
    button(iced_image::Image::new(handle).width(22).height(22))
        .padding(6)
        .on_press(message)
        .style(icon_style)
        .into()
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
