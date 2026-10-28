//! Signed events: canonical CBOR, BLAKE3 ids, and Ed25519 checks.
//!
//! Behavior follows `docs/spec/01-events.md`. This crate does no I/O, reads no
//! clock, and generates no randomness.

mod cbor;
mod error;
mod event;

pub use error::EventError;
pub use event::{EventId, VerifiedEvent, encode_unsigned, sign_wire, signing_public, verify};
