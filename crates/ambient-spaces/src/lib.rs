//! Notes and conversation Spaces.
//!
//! Behavior follows `docs/spec/03-spaces.md`. HPKE sealing is not in this crate yet.
//! v0.1 note bodies are whole snapshots. Concurrent edits are kept, not merged.

mod error;
mod notes;

pub use error::SpaceError;
pub use notes::{
    GenesisMember, NoteHead, Projection, Revocation, Skip, note_put_payload, notes_genesis_payload,
    project_notes,
};
