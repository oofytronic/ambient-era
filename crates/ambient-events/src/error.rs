use core::fmt;

/// A wire or schema failure. [`Self::code`] is the stable string in the spec.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventError {
    /// The bytes are not canonical CBOR for the subset this spec allows.
    Noncanonical,
    /// Field 1 is an integer other than 1.
    BadVersion,
    /// The signature, or the signer bytes, do not check out.
    BadSignature,
    /// The type string is not one this version accepts.
    BadType,
    /// The value is canonical CBOR but not an event, or the payload is not.
    BadPayload,
    /// Two events from one signer share a `prev`. Reserved for a later helper.
    Fork,
    /// The input ended before the CBOR value did.
    Truncated,
}

impl EventError {
    /// The stable error code used by conformance vectors.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Noncanonical => "noncanonical",
            Self::BadVersion => "bad_version",
            Self::BadSignature => "bad_signature",
            Self::BadType => "bad_type",
            Self::BadPayload => "bad_payload",
            Self::Fork => "fork",
            Self::Truncated => "truncated",
        }
    }
}

impl fmt::Display for EventError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for EventError {}
