//! Identity derivation, the key-event log, and pairing offers.
//!
//! Behavior follows `docs/spec/02-identity-log.md`. The root seed is not a
//! passkey. The recovery seal (HPKE of the log) is not implemented yet.

mod device;
mod error;
mod identity;
mod log;

pub use device::{DeviceKeys, device_from_seeds};
pub use error::IdentityError;
pub use identity::{Identity, derive};
pub use log::{
    DeviceState, LogProjection, PairOffer, decode_pair_offer, decode_pair_offer_text,
    device_add_payload, encode_pair_offer, inbox_payload, is_retained, pair_offer_text,
    project_log, revoke_payload,
};
