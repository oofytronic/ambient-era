use core::fmt;

use ambient_events::EventError;

/// A derivation, log, or genesis failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityError {
    /// HKDF refused the output length. Not expected for these 32-byte outputs.
    Derivation,
    /// The envelope could not be encoded, verified, or signed.
    Event(EventError),
    /// The payload is not the event type this version defines.
    BadPayload,
}

impl fmt::Display for IdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Derivation => formatter.write_str("derivation"),
            Self::Event(error) => write!(formatter, "{error}"),
            Self::BadPayload => formatter.write_str("bad_payload"),
        }
    }
}

impl std::error::Error for IdentityError {}
