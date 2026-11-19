use core::fmt;

/// A local log that could not be sealed or opened.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreError {
    /// The key, header, or counter is not usable.
    BadKey,
    /// Authentication failed or the bytes are not this file format.
    Corrupt,
    /// The file could not be read or written.
    Io,
    /// The file changed after it was read, so nothing was written.
    Stale,
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadKey => formatter.write_str("bad_key"),
            Self::Corrupt => formatter.write_str("corrupt"),
            Self::Io => formatter.write_str("io"),
            Self::Stale => formatter.write_str("stale"),
        }
    }
}

impl std::error::Error for StoreError {}
