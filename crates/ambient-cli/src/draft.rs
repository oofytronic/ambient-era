use crate::session::ShownNote;

/// What the editor shows for one note.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Draft {
    /// Title field.
    pub title: String,
    /// Body field. Empty when the note is in conflict, so saving is a choice.
    pub body: String,
    /// Heads to show above the editor. Empty when there is one head.
    pub conflicts: Vec<(String, String)>,
}

impl Draft {
    /// A blank note that has not been saved.
    #[must_use]
    pub fn blank() -> Self {
        Self {
            title: String::new(),
            body: String::new(),
            conflicts: Vec::new(),
        }
    }

    /// Fill the editor from a stored note.
    ///
    /// A conflict keeps every head visible and clears the body. Saving then
    /// writes one snapshot whose parents are those heads.
    #[must_use]
    pub fn from_note(note: &ShownNote) -> Self {
        let title = note
            .heads
            .first()
            .map_or(String::new(), |head| head.title.clone());
        if note.conflict {
            let conflicts = note
                .heads
                .iter()
                .map(|head| (head.title.clone(), head.body.clone()))
                .collect();
            Self {
                title,
                body: String::new(),
                conflicts,
            }
        } else {
            let body = note
                .heads
                .first()
                .map_or(String::new(), |head| head.body.clone());
            Self {
                title,
                body,
                conflicts: Vec::new(),
            }
        }
    }
}

/// Ctrl+S or Cmd+S saves. Ctrl+N or Cmd+N starts a note.
#[must_use]
pub fn shortcut(key: &str, ctrl: bool, meta: bool) -> Option<Shortcut> {
    if !(ctrl || meta) {
        return None;
    }
    match key {
        "s" => Some(Shortcut::Save),
        "n" => Some(Shortcut::NewNote),
        _ => None,
    }
}

/// A keyboard action the window handles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Shortcut {
    /// Write the editor to the notebook.
    Save,
    /// Clear the editor for a note that does not exist yet.
    NewNote,
}

#[cfg(test)]
mod tests {
    use super::{Draft, Shortcut, shortcut};
    use crate::session::{ShownHead, ShownNote};

    #[test]
    fn conflict_clears_the_body_and_keeps_both_heads() {
        let note = ShownNote {
            id: "ab".repeat(32),
            conflict: true,
            heads: vec![
                ShownHead {
                    title: "Left".to_owned(),
                    body: "alpha".to_owned(),
                    writer: "laptop".to_owned(),
                },
                ShownHead {
                    title: "Right".to_owned(),
                    body: "beta".to_owned(),
                    writer: "phone".to_owned(),
                },
            ],
        };
        let draft = Draft::from_note(&note);
        assert_eq!(draft.title, "Left");
        assert!(draft.body.is_empty());
        assert_eq!(draft.conflicts.len(), 2);
        assert_eq!(shortcut("s", true, false), Some(Shortcut::Save));
        assert_eq!(shortcut("n", false, true), Some(Shortcut::NewNote));
        assert_eq!(shortcut("s", false, false), None);
    }
}
