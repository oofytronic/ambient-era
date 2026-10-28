//! Offline notes on one machine.
//!
//! `init`, `add`, `edit`, `list`, `search`, and `show` talk to a directory.
//! The directory holds `key` (the root seed and this device's seeds, mode 0600)
//! and `notes` (wire events sealed with a key derived from the root). This is
//! not the OS keystore, and it does not claim forward secrecy.

mod keys;
mod notes;

pub use notes::run;
