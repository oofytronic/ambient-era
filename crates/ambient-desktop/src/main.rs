//! Linux notes window.
//!
//! Iced draws the controls in a system window. There is no webview. The folder
//! is the same one the `ambient` CLI uses. Pass a path, or it uses `./notebook`.
//! Nothing in this window is sent over the network.

use std::path::PathBuf;

use ambient_cli::{Draft, NoteSummary, Notebook};
use iced::keyboard::Key;
use iced::widget::{button, column, row, scrollable, text, text_editor, text_input};
use iced::{Element, Fill, Task, Theme};

fn main() -> iced::Result {
    iced::application(App::boot, App::update, App::view)
        .title(App::title)
        .window_size((1100.0, 720.0))
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
}

#[derive(Clone, Debug)]
enum Message {
    QueryChanged(String),
    Select(String),
    NewNote,
    TitleChanged(String),
    BodyEdited(text_editor::Action),
    Save,
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
        };
        match open_notebook(&app.dir) {
            Ok(book) => {
                app.notebook = Some(book);
                app.reload();
            }
            Err(err) => app.open_error = err,
        }
        (app, Task::none())
    }

    fn title(&self) -> String {
        format!("Ambient — {}", self.dir.display())
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::QueryChanged(query) => {
                self.query = query;
                self.reload();
            }
            Message::Select(id) => self.show_note(&id),
            Message::NewNote => self.clear_editor(),
            Message::TitleChanged(title) => self.title_text = title,
            Message::BodyEdited(action) => self.body.perform(action),
            Message::Save => self.save(),
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        row![self.sidebar(), self.editor()].into()
    }

    fn sidebar(&self) -> Element<'_, Message> {
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
            text("Offline").size(12),
            text_input("Search", &self.query).on_input(Message::QueryChanged),
            button(text("New note")).on_press(Message::NewNote),
            scrollable(notes).height(Fill),
        ]
        .spacing(12)
        .padding(16)
        .width(280)
        .into()
    }

    fn editor(&self) -> Element<'_, Message> {
        let mut body = column![].spacing(12);
        if !self.open_error.is_empty() {
            body = body.push(text(format!(
                "Could not open this folder. {}",
                self.open_error
            )));
        }
        if !self.message.is_empty() {
            body = body.push(text(self.message.clone()));
        }
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
        let folder = self.dir.display().to_string();
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
        body = body.push(row![
            button(text("Save")).on_press(Message::Save),
            text(format!("Folder {folder}. The key file stays here. This window does not use the network.")).size(12),
        ].spacing(16));
        body.padding(24).into()
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
}

fn open_notebook(dir: &std::path::Path) -> Result<Notebook, String> {
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
