use core::fmt;

use ambient_events::EventError;

/// A derivation or genesis failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityError {
    /// HKDF refused the output length. Not expected for these 32-byte outputs.
    Derivation,
    /// The genesis event could not be encoded or signed.
    Event(EventError),
}

impl fmt::Display for IdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Derivation => formatter.write_str("derivation"),
            Self::Event(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for IdentityError {}
