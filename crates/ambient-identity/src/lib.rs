//! Identity derivation and the genesis event.
//!
//! Behavior follows `docs/spec/02-identity-log.md`. The root seed is not a
//! passkey. Device add, revoke, and pairing are not implemented yet.

mod error;
mod identity;

pub use error::IdentityError;
pub use identity::{Identity, derive};
