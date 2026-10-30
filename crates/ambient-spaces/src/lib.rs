//! Notes and conversation Spaces.
//!
//! Behavior follows `docs/spec/03-spaces.md`. A private Space event is sealed with
//! HPKE before it is given to a relay. v0.1 note bodies are whole snapshots.
//! Concurrent edits are kept, not merged.

mod error;
mod notes;
mod seal;

pub use error::SpaceError;
pub use notes::{
    ChatLine, Conversation, GenesisMember, MemberInfo, NoteHead, Projection, Revocation, Skip,
    conversation_genesis_payload, member_add_payload, msg_put_payload, note_put_payload,
    notes_genesis_payload, project_conversation, project_notes,
};
pub use seal::{open_blob, seal_blob};
