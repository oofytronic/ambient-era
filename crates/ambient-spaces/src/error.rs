use core::fmt;

use ambient_events::EventError;

/// A Space payload or projection failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpaceError {
    /// The envelope was rejected.
    Event(EventError),
    /// Canonical CBOR that is not the payload this type defines.
    BadPayload,
    /// `note.put` in a conversation, or `msg.put` in notes, or a kind other than notes.
    WrongKind,
}

impl fmt::Display for SpaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Event(error) => write!(formatter, "{error}"),
            Self::BadPayload => formatter.write_str("bad_payload"),
            Self::WrongKind => formatter.write_str("wrong_kind"),
        }
    }
}

impl std::error::Error for SpaceError {}
