//! An encrypted local log of opaque wire bytes.
//!
//! This is not SQLCipher and not a search index. The file holds ciphertext.
//! Search happens after the caller decrypts and projects. The key is an argument,
//! never a row.

mod error;
mod log;

pub use error::StoreError;
pub use log::{open_log, read_log, seal_log, write_log};
