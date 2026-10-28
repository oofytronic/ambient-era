//! Offline notes window. Pass a folder, or it uses `./notebook`.
//!
//! Linux needs WebKitGTK 4.1 to build this binary. The window reads and writes
//! the same files as the `ambient` CLI. It does not open a network connection.

use std::path::PathBuf;
use std::sync::OnceLock;

use ambient_cli::{Draft, NoteSummary, Notebook, Shortcut, shortcut};
use dioxus::core::Event;
use dioxus::prelude::*;

fn main() {
    let dir = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("notebook"), PathBuf::from);
    let _ = dir_slot().set(dir);
    dioxus::LaunchBuilder::new()
        .with_cfg(
            dioxus::desktop::Config::new().with_window(
                dioxus::desktop::WindowBuilder::new()
                    .with_title("Ambient")
                    .with_inner_size(dioxus::desktop::LogicalSize::new(1100.0, 720.0)),
            ),
        )
        .launch(app);
}

fn dir_slot() -> &'static OnceLock<PathBuf> {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    &DIR
}

fn app() -> Element {
    let dir = dir_slot()
        .get()
        .cloned()
        .unwrap_or_else(|| PathBuf::from("notebook"));
    let book = use_signal(|| {
        let opened = if dir.join("key").exists() {
            Notebook::open(&dir)
        } else {
            Notebook::create(&dir)
        };
        opened.map_err(|err| err.to_string())
    });
    let mut notes = use_signal(Vec::<NoteSummary>::new);
    let mut query = use_signal(String::new);
    let mut selected = use_signal(|| None::<String>);
    let mut draft = use_signal(Draft::blank);
    let mut message = use_signal(String::new);

    use_effect(move || reload(book, query, notes, message));

    let on_key = move |event: Event<KeyboardData>| {
        let Key::Character(key) = event.key() else {
            return;
        };
        let mods = event.modifiers();
        let Some(action) = shortcut(&key, mods.ctrl(), mods.meta()) else {
            return;
        };
        event.prevent_default();
        match action {
            Shortcut::Save => save(book, selected, draft, query, notes, message),
            Shortcut::NewNote => new_note(selected, draft, message),
        }
    };

    screen(
        dir,
        book,
        Pane {
            notes,
            query,
            selected,
            draft,
            message,
        },
        on_key,
    )
}

#[derive(Clone, Copy)]
struct Pane {
    notes: Signal<Vec<NoteSummary>>,
    query: Signal<String>,
    selected: Signal<Option<String>>,
    draft: Signal<Draft>,
    message: Signal<String>,
}

fn screen(
    dir: PathBuf,
    book: Signal<Result<Notebook, String>>,
    pane: Pane,
    on_key: impl FnMut(Event<KeyboardData>) + 'static,
) -> Element {
    let Pane {
        mut notes,
        mut query,
        mut selected,
        mut draft,
        mut message,
    } = pane;
    let current = selected.read().clone();
    let editor = draft.read().clone();
    let banner = message.read().clone();
    let folder = dir.display().to_string();
    let open_error = book.read().as_ref().err().cloned();
    let listed = notes.read().clone();

    rsx! {
        style { {CSS} }
        div { class: "app", onkeydown: on_key,
            aside { class: "sidebar",
                header { class: "brand",
                    h1 { "Ambient" }
                    p { "Offline" }
                }
                input {
                    class: "search",
                    placeholder: "Search",
                    value: "{query}",
                    oninput: move |event| {
                        query.set(event.value());
                        reload(book, query, notes, message);
                    },
                }
                button { class: "new", onclick: move |_| new_note(selected, draft, message), "New note" }
                ul { class: "notes",
                    for note in listed {
                        li { key: "{note.id}",
                            button {
                                class: if current.as_ref() == Some(&note.id) { "note selected" } else { "note" },
                                onclick: {
                                    let id = note.id.clone();
                                    move |_| open_note(book, id, selected, draft, message)
                                },
                                span { class: "note-title", { label(&note.title) } }
                                if note.conflict {
                                    span { class: "flag", "conflict" }
                                }
                            }
                        }
                    }
                }
            }
            main { class: "editor",
                if let Some(err) = open_error {
                    p { class: "banner", "Could not open this folder. {err}" }
                }
                if !banner.is_empty() {
                    p { class: "banner", "{banner}" }
                }
                if !editor.conflicts.is_empty() {
                    section { class: "conflicts",
                        h2 { "Two versions. Write the one you want to keep, then save." }
                        for (index, (title, body)) in editor.conflicts.iter().cloned().enumerate() {
                            article { key: "{index}",
                                h3 { "Version {index + 1}: {title}" }
                                pre { "{body}" }
                            }
                        }
                    }
                }
                input {
                    class: "title",
                    placeholder: "Title",
                    value: "{editor.title}",
                    oninput: move |event| set_title(draft, event.value()),
                }
                textarea {
                    class: "body",
                    placeholder: "Write",
                    value: "{editor.body}",
                    oninput: move |event| set_body(draft, event.value()),
                }
                footer {
                    button { class: "save", onclick: move |_| save(book, selected, draft, query, notes, message), "Save" }
                    p { class: "path", "Folder {folder}. The key file stays here. This window does not use the network." }
                }
            }
        }
    }
}

fn label(title: &str) -> String {
    if title.is_empty() {
        "Untitled".to_owned()
    } else {
        title.to_owned()
    }
}

fn new_note(
    mut selected: Signal<Option<String>>,
    mut draft: Signal<Draft>,
    mut message: Signal<String>,
) {
    selected.set(None);
    draft.set(Draft::blank());
    message.set(String::new());
}

fn set_title(mut draft: Signal<Draft>, title: String) {
    let mut next = draft.read().clone();
    next.title = title;
    draft.set(next);
}

fn set_body(mut draft: Signal<Draft>, body: String) {
    let mut next = draft.read().clone();
    next.body = body;
    draft.set(next);
}

fn reload(
    book: Signal<Result<Notebook, String>>,
    query: Signal<String>,
    mut notes: Signal<Vec<NoteSummary>>,
    mut message: Signal<String>,
) {
    let found = {
        let guard = book.read();
        let Some(open) = guard.as_ref().ok() else {
            return;
        };
        if query.read().is_empty() {
            open.list()
        } else {
            open.search(&query.read())
        }
    };
    match found {
        Ok(items) => notes.set(items),
        Err(err) => message.set(err.to_string()),
    }
}

fn open_note(
    book: Signal<Result<Notebook, String>>,
    id: String,
    mut selected: Signal<Option<String>>,
    mut draft: Signal<Draft>,
    mut message: Signal<String>,
) {
    let shown = {
        let guard = book.read();
        let Some(open) = guard.as_ref().ok() else {
            return;
        };
        open.show(&id)
    };
    match shown {
        Ok(note) => {
            selected.set(Some(id));
            draft.set(Draft::from_note(&note));
            message.set(String::new());
        }
        Err(err) => message.set(err.to_string()),
    }
}

fn save(
    book: Signal<Result<Notebook, String>>,
    selected: Signal<Option<String>>,
    draft: Signal<Draft>,
    query: Signal<String>,
    notes: Signal<Vec<NoteSummary>>,
    message: Signal<String>,
) {
    let title = draft.read().title.clone();
    let body = draft.read().body.clone();
    let current = selected.read().clone();
    let saved = {
        let guard = book.read();
        let Some(open) = guard.as_ref().ok() else {
            return;
        };
        match current.as_deref() {
            Some(id) => open.edit(id, &title, &body).map(|()| id.to_owned()),
            None => open.add(&title, &body),
        }
    };
    match saved {
        Ok(id) => {
            let mut message = message;
            message.set(String::new());
            open_note(book, id, selected, draft, message);
            reload(book, query, notes, message);
        }
        Err(err) => {
            let mut message = message;
            message.set(err.to_string());
        }
    }
}

const CSS: &str = r#"
:root {
  color-scheme: dark;
  font-family: "Iowan Old Style", Palatino, "Palatino Linotype", Georgia, serif;
  background: #141311;
  color: #ece7df;
}
* { box-sizing: border-box; }
html, body, #main { margin: 0; height: 100%; }
.app { display: grid; grid-template-columns: 280px 1fr; height: 100vh; }
.sidebar { border-right: 1px solid #2c2924; padding: 20px 16px; display: flex; flex-direction: column; gap: 12px; background: #1b1916; }
.brand h1 { font-size: 22px; font-weight: 500; margin: 0; }
.brand p { margin: 2px 0 0; color: #9a917f; font-family: ui-sans-serif, sans-serif; font-size: 12px; letter-spacing: 0.08em; text-transform: uppercase; }
.search, .title, .body, button { font: inherit; color: inherit; background: transparent; border: 1px solid #3a352c; border-radius: 6px; }
.search, .title, .body { padding: 8px 10px; width: 100%; }
.search { font-family: ui-sans-serif, sans-serif; font-size: 14px; }
.notes { list-style: none; margin: 0; padding: 0; overflow: auto; display: flex; flex-direction: column; gap: 4px; }
.note { width: 100%; text-align: left; padding: 8px 10px; display: flex; justify-content: space-between; gap: 8px; cursor: pointer; font-family: ui-sans-serif, sans-serif; font-size: 14px; }
.note.selected { background: #2a261f; border-color: #6d6248; }
.flag { color: #d7a15a; font-size: 11px; letter-spacing: 0.06em; text-transform: uppercase; }
.new, .save { cursor: pointer; font-family: ui-sans-serif, sans-serif; font-size: 14px; padding: 8px 12px; }
.save { background: #ece7df; color: #141311; border: 0; }
.editor { display: flex; flex-direction: column; padding: 28px 36px 16px; gap: 12px; min-width: 0; }
.title { font-size: 28px; border: 0; padding-left: 0; }
.body { flex: 1; resize: none; line-height: 1.5; border: 0; padding-left: 0; font-size: 18px; }
.conflicts { border: 1px solid #6d5428; border-radius: 8px; padding: 12px 14px; max-height: 220px; overflow: auto; }
.conflicts h2, .conflicts h3 { font-family: ui-sans-serif, sans-serif; font-size: 13px; font-weight: 600; margin: 0 0 8px; }
.conflicts pre { white-space: pre-wrap; margin: 0 0 12px; font-family: inherit; }
.banner { margin: 0; color: #e0b4b4; font-family: ui-sans-serif, sans-serif; font-size: 14px; }
footer { display: flex; align-items: center; gap: 16px; }
.path { margin: 0; color: #8d8678; font-family: ui-sans-serif, sans-serif; font-size: 12px; }
button:focus-visible, input:focus-visible, textarea:focus-visible { outline: 1px solid #d7a15a; }
"#;
