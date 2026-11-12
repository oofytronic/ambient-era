//! Linux notes window.
//!
//! Iced draws the controls in a system window. There is no webview. The folder
//! is the same one the `ambient` CLI uses. Pass a path, or it uses `./notebook`.
//! Sync, pairing, and chat call `ambient-cli`. This file does not seal events.
//! The relay it can start is cleartext on `127.0.0.1` only.

use std::collections::BTreeMap;
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
    button, checkbox, column, container, row, scrollable, space, stack, text, text_editor,
    text_input, tooltip,
};
use iced::{Color, Element, Fill, Font, Size, Task, Theme};
use image::ImageEncoder;
use qrcode::QrCode;

mod clock;
mod files;
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
    draft_chat: String,
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
    syncing: bool,
    edited_at: Option<Instant>,
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
}

#[derive(Clone, Debug)]
enum Message {
    QueryChanged(String),
    PickTag(Option<String>),
    Select(String),
    NewNote,
    TitleChanged(String),
    BodyEdited(text_editor::Action),
    Place(usize),
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
    Synced(SyncOutcome),
    Resized(Size),
    FileDropped(PathBuf),
    PickFile,
    FilePicked(Result<PathBuf, String>),
    DropPending(usize),
    OpenPath(PathBuf),
    Format(FormatKind),
    TaskDraft(String),
    AddTask,
    AskDelete,
    CancelDelete,
    DeleteNote,
    ChooseSend(String),
    ConfirmSend,
    CancelSend,
    Breeze,
    AutoSave,
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
    WipeDraft(String),
    ExportNotes,
    ConfirmWipe,
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

impl App {
    fn boot() -> (Self, Task<Message>) {
        let dir = std::env::args()
            .nth(1)
            .map_or_else(|| PathBuf::from("notebook"), PathBuf::from);
        let mut app = Self {
            dir,
            notebook: None,
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
            draft_chat: String::new(),
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
            syncing: false,
            edited_at: None,
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
        };
        let saved = profile::load_profile(&app.dir);
        app.appearance = saved.appearance;
        app.your_name.clone_from(&saved.name);
        app.name_draft.clone_from(&saved.name);
        app.device_draft.clone_from(&saved.device);
        app.system_dark = profile::system_is_dark();
        for (id, name) in profile::load_names(&app.dir) {
            app.aliases.insert(id, name);
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
        let window = iced::event::listen_with(|event, _status, _id| match event {
            iced::Event::Window(
                iced::window::Event::Resized(size) | iced::window::Event::Opened { size, .. },
            ) => Some(Message::Resized(size)),
            iced::Event::Window(iced::window::Event::FileDropped(path)) => {
                Some(Message::FileDropped(path))
            }
            _ => None,
        });
        let mut parts = vec![window];
        if app.breeze_on() {
            parts.push(
                iced::time::every(std::time::Duration::from_millis(32)).map(|_| Message::Breeze),
            );
        }
        if app.notebook.is_some() && app.editor_dirty() {
            parts.push(
                iced::time::every(std::time::Duration::from_millis(400)).map(|_| Message::AutoSave),
            );
        }
        let remote = app.net_mode != ambient_cli::NetMode::ThisComputer;
        if app.notebook.is_some() && (app.relay_ready || remote) {
            parts.push(
                iced::time::every(std::time::Duration::from_secs(3)).map(|_| Message::AutoSync),
            );
        }
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
                Task::none()
            }
            Message::TitleChanged(title) => {
                self.title_text = title;
                self.edited_at = Some(Instant::now());
                Task::none()
            }
            Message::BodyEdited(action) => {
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
                    self.apply_change(change);
                    return self.note_follow(true);
                }
                if self.body.edit(&action) {
                    self.edited_at = Some(Instant::now());
                }
                self.note_follow(true)
            }
            Message::Place(byte) => {
                self.body.place_byte(byte);
                self.note_follow(false)
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
                    let cursor = self.body.cursor();
                    self.body.load(&next);
                    self.body.place(cursor);
                    self.edited_at = Some(Instant::now());
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
                    let changed = self
                        .body
                        .edit(&text_editor::Action::Edit(text_editor::Edit::Delete));
                    if changed {
                        self.edited_at = Some(Instant::now());
                    }
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
                Task::none()
            }
            Message::ShowTasks => {
                self.rebuild_tasks();
                self.screen = Screen::Tasks;
                self.sheet = Sheet::None;
                self.detail = false;
                Task::none()
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
                    self.sheet = Sheet::None;
                    if self.compact() {
                        self.detail = false;
                    }
                    self.reload_chat();
                }
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
                self.screen = Screen::Chat;
                self.sheet = Sheet::None;
                self.detail = true;
                self.reload_chat();
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
                self.sheet = Sheet::Invite;
                Task::none()
            }
            Message::ShowSettings => {
                self.devices = self
                    .notebook
                    .as_ref()
                    .and_then(|book| book.devices().ok())
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
            Message::TaskDraft(text) => {
                self.task_draft = text;
                Task::none()
            }
            Message::AddTask => {
                let follow = self.add_loose_task();
                if self.editor_dirty() {
                    Task::batch([follow, self.start_save(true)])
                } else {
                    follow
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
            Message::DeleteNote => {
                self.delete_open_note();
                Task::none()
            }
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
            Message::AutoSave => self.start_save(false),
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
            Message::WipeDraft(text) => {
                self.wipe_draft = text;
                Task::none()
            }
            Message::ExportNotes => self.export_notes(),
            Message::ConfirmWipe => self.wipe_notebook(),
            Message::Synced(outcome) => self.finish_sync(outcome),
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
        let modes = container(
            row![
                segment("Notes", self.screen == Screen::Notes, Message::ShowNotes),
                segment("Tasks", self.screen == Screen::Tasks, Message::ShowTasks),
                segment("Chat", self.screen == Screen::Chat, Message::ShowChat),
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
                side = side.push(chat_row(&self.chat_name(chat), &chat.id, selected));
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

    fn note_index(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let mut block = column![
            text_input("Search notes", &self.query)
                .on_input(Message::QueryChanged)
                .padding(10)
                .size(14)
                .style(field_style),
        ]
        .spacing(8)
        .width(Fill)
        .height(Fill);
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
            list = list.push(self.tag_row(None, self.catalog.len()));
            let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
            for note in &self.catalog {
                for tag in &note.tags {
                    *counts.entry(tag.as_str()).or_default() += 1;
                }
            }
            for (tag, count) in counts {
                list = list.push(self.tag_row(Some(tag), count));
            }
        } else if !self.catalog.is_empty() {
            list = list.push(
                text("A #tag in a note files it here.")
                    .size(12)
                    .color(ink.muted),
            );
        }
        for note in &self.notes {
            list = list.push(self.note_row(note));
        }
        block.push(vscroll(list).height(Fill).width(Fill)).into()
    }

    fn tag_row(&self, tag: Option<&str>, count: usize) -> Element<'_, Message> {
        let ink = self.ink();
        let active = self.tag.as_deref() == tag;
        let label = tag.map_or_else(|| "All".to_owned(), |name| format!("#{name}"));
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
        if !note.tags.is_empty() {
            if !meta.is_empty() {
                meta.push_str(" · ");
            }
            for (index, tag) in note.tags.iter().take(3).enumerate() {
                if index > 0 {
                    meta.push(' ');
                }
                meta.push('#');
                meta.push_str(tag);
            }
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

    fn tasks_pane(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let add = row![
            text_input("Add a task", &self.task_draft)
                .on_input(Message::TaskDraft)
                .on_submit(Message::AddTask)
                .padding(10)
                .width(Fill)
                .style(field_style),
            action_label("Add", Message::AddTask),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .width(Fill);
        let mut list = column![].spacing(14).width(Fill);
        let open: Vec<&TaskRow> = self.tasks.iter().filter(|task| !task.done).collect();
        let done: Vec<&TaskRow> = self.tasks.iter().filter(|task| task.done).collect();
        if open.is_empty() && done.is_empty() {
            list = list.push(text("Nothing to do yet.").font(serif()).size(32));
            list = list.push(
                text("Or write a line like - [ ] call the clinic in any note.")
                    .size(14)
                    .color(ink.muted),
            );
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
        column![
            add,
            text("This goes in a note named Tasks. A box in any other note shows up here too.")
                .size(13)
                .color(ink.muted),
            vscroll(list).height(Fill).width(Fill),
        ]
        .spacing(12)
        .width(Fill)
        .height(Fill)
        .into()
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
        let label = if task.text.is_empty() {
            "Empty task".to_owned()
        } else {
            task.text.clone()
        };
        let words = if task.done { ink.muted } else { ink.text };
        row![
            checkbox(task.done).on_toggle(move |done| Message::SetTask {
                id: id.clone(),
                line,
                done,
            }),
            button(
                column![
                    text(label).size(16).color(words),
                    text(title).size(12).color(ink.muted),
                ]
                .spacing(2)
                .width(Fill),
            )
            .width(Fill)
            .padding([2, 0])
            .on_press(Message::Select(open_id))
            .style(quiet_style),
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center)
        .into()
    }

    fn rebuild_tasks(&mut self) {
        let selected = self.selected.clone();
        let unsaved = self.unsaved.clone();
        let mut rows = Vec::new();
        if let Some(book) = self.notebook.as_ref() {
            let _ = book.visit_notes(|id, title, body, conflict| {
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
        let shown = self.notebook.as_ref()?.show(id).ok()?;
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
        if self.selected.as_deref() == Some(id) {
            let cursor = self.body.cursor();
            self.body.load(&next);
            self.body.place(cursor);
            self.edited_at = Some(Instant::now());
            return Task::none();
        }
        if let Some(slot) = self
            .unsaved
            .iter_mut()
            .find(|(key, _, _)| key.as_deref() == Some(id))
        {
            slot.2.clone_from(&next);
        }
        self.apply_note(Some(id.to_owned()), title, next)
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
                .size(if self.compact() { 32.0 } else { 36.0 })
                .padding([2.0, 0.0])
                .style(title_style),
        );
        if !self.compact() {
            body = body.push(Self::format_bar());
        }
        body = body.push(
            vscroll(markdown::view(&self.body, self.is_dark()))
                .id(iced::widget::Id::new("note-scroll"))
                .height(Fill),
        );
        body = body.push(self.note_media());
        if self.send_open {
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
            return body.into();
        }
        if self.confirm_delete {
            body = body.push(self.delete_confirm());
            return body.into();
        }
        self.overlay_tools(body.into(), true)
    }

    fn overlay_tools<'a>(
        &'a self,
        page: Element<'a, Message>,
        editing: bool,
    ) -> Element<'a, Message> {
        stack![
            page,
            container(self.tool_cluster(editing))
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

    fn tool_cluster(&self, editing: bool) -> Element<'_, Message> {
        let mut tools = row![circle_button("+", "New note", Message::NewNote)].spacing(8);
        if editing && (self.selected.is_some() || self.editor_dirty()) {
            let hint = if self.selected.is_some() {
                "Delete"
            } else {
                "Discard"
            };
            tools = tools.push(circle_button("×", hint, Message::AskDelete));
        }
        if editing
            && (self.selected.is_some()
                || !self.body.text().is_empty()
                || !self.title_text.is_empty())
        {
            tools = tools.push(circle_button("↗", "Send to chat", Message::SendNoteToChat));
        }
        tools.into()
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
        column![
            text(prompt).size(14).color(ink.muted),
            row![
                action_label(verb, Message::DeleteNote),
                ghost_label("Cancel", Message::CancelDelete),
            ]
            .spacing(8),
        ]
        .spacing(8)
        .into()
    }

    fn chat_pane(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let mut lines = column![].spacing(8);
        if self.chat.is_empty() && self.pending_files.is_empty() {
            lines = lines.push(text("No messages yet.").color(ink.muted));
            lines = lines.push(action_label("Invite someone", Message::ShowInvite));
        }
        let width = self.bubble_width();
        let mut day = String::new();
        for line in &self.chat {
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
        let narrow = self.chat_inner() < 420.0;
        let field = text_input("Message", &self.draft_chat)
            .on_input(Message::DraftChat)
            .on_submit(Message::SendChat)
            .padding(10)
            .width(Fill)
            .style(field_style);
        let composer_controls: Element<'_, Message> = if narrow {
            column![
                field,
                row![
                    ghost_label("Add", Message::PickFile),
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
        composer = composer.push(composer_controls);
        container(
            column![
                vscroll(lines.width(Fill))
                    .height(Fill)
                    .width(Fill)
                    .anchor_bottom(),
                composer,
            ]
            .spacing(12)
            .width(Fill)
            .height(Fill),
        )
        .width(Fill)
        .height(Fill)
        .clip(true)
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
        sheet = sheet.push(text("What do you call them?").size(12).color(ink.muted));
        sheet = sheet.push(
            text("Only this computer remembers it. A contact card cannot carry a name yet. Their key still shows in Settings.")
                .size(13)
                .color(ink.muted),
        );
        sheet = sheet.push(
            text_input("A name on this computer", &self.call_draft)
                .on_input(Message::CallDraft)
                .padding(10)
                .style(field_style),
        );
        sheet = sheet.push(action_label(
            "Start conversation",
            Message::StartConversation,
        ));
        vscroll(sheet).height(Fill).into()
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
            book.list()
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
        self.rebuild_tasks();
    }

    fn refresh_shown(&mut self) {
        let base = if self.query.is_empty() {
            self.catalog.clone()
        } else {
            match self.notebook.as_ref().map(|book| book.search(&self.query)) {
                Some(Ok(notes)) => notes,
                Some(Err(err)) => {
                    self.message = err.to_string();
                    Vec::new()
                }
                None => Vec::new(),
            }
        };
        self.notes = match &self.tag {
            Some(tag) => base
                .into_iter()
                .filter(|note| note.tags.iter().any(|item| item == tag))
                .collect(),
            None => base,
        };
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
        match ambient_cli::chat_messages_in(&self.dir, &id) {
            Ok(lines) => {
                let tz = clock::load_tz();
                let now = clock::now_ms();
                self.chat = lines
                    .into_iter()
                    .map(|line| {
                        let when = clock::when(line.unix_ms, &tz, now);
                        ChatBubble {
                            text: line.text,
                            time: when.time,
                            day: when.day,
                            mine: line.mine,
                        }
                    })
                    .collect();
            }
            Err(err) => self.message = err.to_string(),
        }
    }

    fn show_note(&mut self, id: &str) {
        self.send_open = false;
        self.confirm_delete = false;
        if self.selected.as_deref() == Some(id) && self.editor_dirty() {
            self.screen = Screen::Notes;
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
                    self.body.load(&body);
                    self.edited_at = Some(Instant::now());
                } else {
                    self.title_text.clone_from(&draft.title);
                    self.body.load(&draft.body);
                    self.edited_at = None;
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
            }
            Err(err) => self.message = err.to_string(),
        }
    }

    fn clear_editor(&mut self) {
        self.send_open = false;
        self.confirm_delete = false;
        self.remember_unsaved();
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
        if let Some((title, body)) = stash {
            self.title_text = title;
            self.body.load(&body);
            self.edited_at = Some(Instant::now());
            return;
        }
        self.title_text.clear();
        self.body.load("");
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
        Ink::from_dark(self.is_dark())
    }

    fn is_dark(&self) -> bool {
        match self.appearance {
            profile::Appearance::Light => false,
            profile::Appearance::Dark => true,
            profile::Appearance::System => self.system_dark,
        }
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
        self.saving = true;
        self.save_again = false;
        let token = self.save_token;
        save_task(
            self.dir.clone(),
            self.selected.clone(),
            self.title_text.clone(),
            self.body.text(),
            token,
        )
    }

    fn begin_sync(&mut self) -> Task<Message> {
        if self.notebook.is_none() {
            return Task::none();
        }
        if self.editor_dirty()
            && self
                .edited_at
                .is_some_and(|at| at.elapsed() >= Duration::from_millis(700))
        {
            return self.start_save(false);
        }
        if self.saving
            || self.syncing
            || (self.net_mode == ambient_cli::NetMode::ThisComputer && !self.relay_ready)
        {
            return Task::none();
        }
        self.syncing = true;
        sync_task(self.dir.clone(), self.writes)
    }

    fn finish_sync(&mut self, outcome: SyncOutcome) -> Task<Message> {
        self.syncing = false;
        if outcome.failed {
            if outcome.transport {
                self.net_up = false;
                if self.net_mode == ambient_cli::NetMode::ThisComputer
                    && outcome.detail.contains("unreachable")
                    && !self.relay_ours
                {
                    self.relay_ready = false;
                }
                "No relay answered.".clone_into(&mut self.sync_detail);
            } else {
                self.sync_detail = outcome.detail;
            }
            self.pending = true;
            return Task::none();
        }
        self.net_up = true;
        self.sync_detail = outcome.detail;
        if content_arrived(&self.sync_detail) {
            let opened = {
                let _guard = io_lock();
                Notebook::open(&self.dir)
            };
            match opened {
                Ok(book) => {
                    self.notebook = Some(book);
                    self.reload();
                    self.refresh_open_note();
                }
                Err(err) => self.sync_detail = err.to_string(),
            }
            self.reload_chat();
        }
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
        self.body.load(&draft.body);
        self.edited_at = None;
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

    fn editing_note(&self) -> bool {
        self.sheet == Sheet::None
            && self.screen == Screen::Notes
            && (!self.compact() || self.detail)
    }

    fn chat_inner(&self) -> f32 {
        let chrome = if self.compact() {
            self.gutter_x() * 2.0
        } else {
            self.side_width() + self.gutter_x() * 2.0 + 1.0
        };
        (self.size.width - chrome).max(120.0)
    }

    fn bubble_width(&self) -> f32 {
        let inner = self.chat_inner();
        (inner - 36.0).clamp(96.0, 460.0)
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
                format_button("[[]]", "Wikilink", FormatKind::Wiki),
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
        let mut nav = row![
            segment(
                "Notes",
                self.screen == Screen::Notes && self.sheet == Sheet::None,
                Message::ShowNotes,
            ),
            segment(
                "Tasks",
                self.screen == Screen::Tasks && self.sheet == Sheet::None,
                Message::ShowTasks,
            ),
            segment(
                "Chat",
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
        self.overlay_tools(self.note_index(), false)
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
            side = side.push(chat_row(&self.chat_name(chat), &chat.id, selected));
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
        if !when.is_empty() {
            let time_color = if mine {
                Color {
                    a: 0.72,
                    ..ink.on_accent
                }
            } else {
                ink.muted
            };
            body = body.push(text(when).size(11).color(time_color));
        }
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
                self.body
                    .perform(&text_editor::Action::Edit(text_editor::Edit::Paste(
                        Arc::new(insert),
                    )));
                self.edited_at = Some(Instant::now());
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
        self.apply_change(change);
    }

    fn apply_change(&mut self, change: markdown::Change) {
        if change.anchor == change.end {
            self.body.place(text_editor::Cursor {
                position: change.anchor,
                selection: None,
            });
        } else {
            self.body.place(text_editor::Cursor {
                position: change.end,
                selection: Some(change.anchor),
            });
        }
        self.body
            .perform(&text_editor::Action::Edit(text_editor::Edit::Paste(
                Arc::new(change.insert),
            )));
        self.body.place(change.after);
        self.edited_at = Some(Instant::now());
    }

    fn note_follow(&mut self, scroll: bool) -> Task<Message> {
        let jump = if scroll { self.body.follow() } else { None };
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
                self.reload_chat();
                self.mark_pending();
            }
            Err(err) => self.message = err.to_string(),
        }
    }

    fn send_chat(&mut self) {
        let mut text = self.draft_chat.trim().to_owned();
        let Some(id) = self.chat_id.clone() else {
            self.say("Invite someone before sending.");
            return;
        };
        let files = std::mem::take(&mut self.pending_files);
        for path in &files {
            match files::store(&self.dir, &id, path) {
                Ok(cite) => {
                    if !text.is_empty() && !text.ends_with('\n') {
                        text.push('\n');
                    }
                    text.push_str(&cite);
                }
                Err(err) => {
                    self.pending_files = files;
                    self.message = err;
                    return;
                }
            }
        }
        if text.is_empty() {
            self.pending_files = files;
            self.say("Write a message, or add a file.");
            return;
        }
        match ambient_cli::send_chat_in(&self.dir, &id, &text) {
            Ok(()) => {
                self.draft_chat.clear();
                self.screen = Screen::Chat;
                self.detail = true;
                self.say("Saved here. The words sync while a relay is on. A file stays on this computer.");
                self.reload_chat();
                self.mark_pending();
            }
            Err(err) => {
                self.draft_chat = text;
                self.message = err.to_string();
            }
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
        if self.title_text.trim().is_empty() && self.body.text().trim().is_empty() {
            self.say("Write a note first.");
            return;
        }
        self.reload_chat();
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
        match ambient_cli::send_chat_in(&self.dir, &id, &text) {
            Ok(()) => {
                self.send_open = false;
                self.chat_id = Some(id);
                self.say(&format!(
                    "Sent to {label}. The words sync while a relay is on."
                ));
                self.reload_chat();
                self.mark_pending();
            }
            Err(err) => self.message = err.to_string(),
        }
    }

    fn add_loose_task(&mut self) -> Task<Message> {
        let label = self.task_draft.trim().replace(['\n', '\r'], " ");
        if label.is_empty() {
            self.say("Write the task first.");
            return Task::none();
        }
        if self.notebook.is_none() {
            self.say("Folder is not open.");
            return Task::none();
        }
        let line = format!("- [ ] {label}");
        let existing = self.notebook.as_ref().and_then(|book| {
            book.list().ok().and_then(|notes| {
                notes
                    .into_iter()
                    .find(|note| note.title == "Tasks" && !note.conflict)
                    .map(|note| note.id)
            })
        });
        let Some(id) = existing else {
            self.task_draft.clear();
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
            self.say("Added to Tasks.");
        }
        follow
    }

    fn delete_open_note(&mut self) {
        self.confirm_delete = false;
        if self.saving {
            self.say("Still saving. Try delete again in a moment.");
            return;
        }
        let Some(id) = self.selected.clone() else {
            self.discard_draft();
            self.say("Discarded.");
            return;
        };
        let saved = {
            let _guard = io_lock();
            let Some(book) = self.notebook.as_ref() else {
                self.say("Folder is not open.");
                return;
            };
            book.edit(&id, "", ambient_cli::DELETED_BODY)
        };
        match saved {
            Ok(()) => {
                self.unsaved
                    .retain(|(key, _, _)| key.as_deref() != Some(id.as_str()));
                self.selected = None;
                self.title_text.clear();
                self.body.load("");
                self.disk_title.clear();
                self.disk_body.clear();
                self.conflicts.clear();
                self.note_writer.clear();
                self.edited_at = None;
                self.save_token = self.save_token.wrapping_add(1);
                self.writes = self.writes.saturating_add(1);
                self.pending = true;
                self.reload();
                self.say("Deleted. It has left the list.");
            }
            Err(err) => self.message = err.to_string(),
        }
    }

    fn discard_draft(&mut self) {
        self.unsaved.retain(|(key, _, _)| key.is_some());
        self.title_text.clear();
        self.body.load("");
        self.disk_title.clear();
        self.disk_body.clear();
        self.edited_at = None;
        self.confirm_delete = false;
        self.save_token = self.save_token.wrapping_add(1);
    }

    fn write_note(&mut self, id: &str, title: &str, body: &str) -> Task<Message> {
        if self.selected.as_deref() == Some(id) {
            title.clone_into(&mut self.title_text);
            self.body.load(body);
            self.edited_at = Some(Instant::now());
            return Task::none();
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
        self.reload_chat();
        if let Ok(card) = ambient_cli::contact_card(&self.dir) {
            self.my_card = card;
        }
        if let Ok(link) = ambient_cli::contact_link(&self.dir) {
            self.contact_link = link;
        }
        self.devices = self
            .notebook
            .as_ref()
            .and_then(|book| book.devices().ok())
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
        if paired && self.notes.is_empty() {
            self.say(
                "Paired device. It does not hold the root. It syncs on its own while the relay is on.",
            );
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
                    self.selected = Some(id);
                }
                if current && self.title_text == title && self.body.text() == body {
                    self.disk_title = title;
                    self.disk_body = body;
                    self.edited_at = None;
                }
                self.writes = self.writes.saturating_add(1);
                self.pending = true;
                self.take_notes(notes);
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
        Task::none()
    }

    fn finish_applied(&mut self, result: Result<Vec<NoteSummary>, String>) -> Task<Message> {
        match result {
            Ok(notes) => {
                self.writes = self.writes.saturating_add(1);
                self.pending = true;
                self.take_notes(notes);
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
        if self.screen == Screen::Tasks {
            self.rebuild_tasks();
        }
    }

    fn apply_note(&self, id: Option<String>, title: String, body: String) -> Task<Message> {
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
                self.persist_profile();
                self.open_error.clear();
                self.adopt_open_book(book);
                self.loaded = true;
                self.say("This notebook is on this computer.");
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
        if let Err(err) = book.visit_notes(|id, title, body, _conflict| {
            notes.push((id.to_owned(), title.to_owned(), body.to_owned()));
        }) {
            self.message = err.to_string();
            return Task::none();
        }
        if self.editor_dirty() {
            if let Some(id) = &self.selected {
                if let Some(slot) = notes.iter_mut().find(|(note_id, _, _)| note_id == id) {
                    slot.1.clone_from(&self.title_text);
                    slot.2 = self.body.text();
                }
            } else if !self.title_text.is_empty() || !self.body.text().is_empty() {
                notes.push((
                    "unsaved".to_owned(),
                    self.title_text.clone(),
                    self.body.text(),
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
            let _guard = io_lock();
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
        self.catalog.clear();
        self.notes.clear();
        self.tasks.clear();
        self.query.clear();
        self.tag = None;
        self.selected = None;
        self.title_text.clear();
        self.body.load("");
        self.disk_title.clear();
        self.disk_body.clear();
        self.conflicts.clear();
        self.note_writer.clear();
        self.chats.clear();
        self.chat.clear();
        self.chat_id = None;
        self.devices.clear();
        self.draft_chat.clear();
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
        self.call_draft.clear();
        self.wipe_draft.clear();
        self.aliases.clear();
        self.your_name.clear();
        self.name_draft.clear();
        self.device_draft.clear();
        self.appearance = profile::Appearance::Dark;
        self.open_error.clear();
        self.message.clear();
        self.edited_at = None;
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
                        self.data_section(),
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
    writes: u64,
}

fn io_lock() -> std::sync::MutexGuard<'static, ()> {
    static IO: Mutex<()> = Mutex::new(());
    IO.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn run_sync(dir: &Path) -> SyncOutcome {
    let _guard = io_lock();
    match ambient_cli::sync_notebook(dir) {
        Ok(detail) => SyncOutcome {
            pending: ambient_cli::has_unsent(dir).unwrap_or(true),
            detail,
            failed: false,
            transport: false,
            writes: 0,
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
                writes: 0,
            }
        }
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
                transport: false,
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

fn scroll_style(theme: &Theme, status: scrollable::Status) -> scrollable::Style {
    let mut style = scrollable::default(theme, status);
    let ink = ink_of(theme);
    let thumb = Color {
        a: if scroll_hot(status) { 0.62 } else { 0.22 },
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
                    let _guard = io_lock();
                    let book = Notebook::open(&dir).map_err(|err| err.to_string())?;
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
    let _guard = io_lock();
    let book = Notebook::open(dir).map_err(|err| err.to_string())?;
    let saved = match id {
        Some(id) => {
            book.edit(id, title, body).map_err(|err| err.to_string())?;
            id.to_owned()
        }
        None => book.add(title, body).map_err(|err| err.to_string())?,
    };
    let notes = book.list().map_err(|err| err.to_string())?;
    Ok((saved, notes))
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
    match (character.as_deref(), command) {
        (Some("s"), true) => Some(text_editor::Binding::Custom(Message::Save)),
        (Some("n"), true) => Some(text_editor::Binding::Custom(Message::NewNote)),
        (Some("c"), true) => Some(text_editor::Binding::Custom(Message::CopyNote)),
        (Some("x"), true) => Some(text_editor::Binding::Custom(Message::CutNote)),
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
