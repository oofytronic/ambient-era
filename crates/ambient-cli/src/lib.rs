//! Notes, a second device, and one conversation on one machine.
//!
//! `init`, `add`, `edit`, `list`, `search`, and `show` talk to a directory.
//! The directory holds `key` (mode 0600) and `notes` (wire events sealed with
//! ChaCha20-Poly1305). A paired device does not hold the root. This is not the
//! OS keystore, and it does not claim forward secrecy.
//!
//! The desktop window calls the functions below. It does not seal events or
//! speak the relay JSON itself.

use std::path::Path;

mod draft;
mod keys;
mod notes;
mod session;
mod sync;

pub use draft::{Draft, Shortcut, shortcut};
pub use notes::{CliError, run};
pub use session::{DeviceLine, NoteSummary, Notebook, ShownHead, ShownNote};

/// Loopback relay the window starts. Cleartext. Not a public deployment.
pub const LOOPBACK_URL: &str = "ws://127.0.0.1:8787";

/// Bind `127.0.0.1:8787` and serve in the background.
///
/// # Errors
///
/// Returns [`CliError`] when that address is already taken.
pub fn start_loopback_relay() -> Result<(), CliError> {
    sync::start_loopback()
}

/// Create a paired folder and return the device offer. The folder does not hold the root.
///
/// # Errors
///
/// Returns [`CliError`] when the folder already has a key, or the offer cannot be encoded.
pub fn device_offer(dir: &Path, label: &str) -> Result<String, CliError> {
    sync::device(dir, label)
}

/// Push and pull sealed events through `url`.
///
/// # Errors
///
/// Returns [`CliError`] when the relay cannot be reached or a log cannot be written.
pub fn sync_folder(dir: &Path, url: &str) -> Result<String, CliError> {
    sync::sync(dir, url)
}

/// This folder's contact card. Not a device offer. Do not pass it to [`Notebook::pair_device`].
///
/// # Errors
///
/// Returns [`CliError`] when the card cannot be read or written.
pub fn contact_card(dir: &Path) -> Result<String, CliError> {
    sync::card(dir)
}

/// Open the one conversation in this folder, using the other person's card.
///
/// # Errors
///
/// Returns [`CliError`] when the card is this device, a conversation already exists, or the log cannot be written.
pub fn start_conversation(dir: &Path, card: &str) -> Result<(), CliError> {
    sync::talk(dir, card)
}

/// Append one chat message. It stays local until [`sync_folder`].
///
/// # Errors
///
/// Returns [`CliError`] when there is no conversation or the message is too long.
pub fn send_chat(dir: &Path, text: &str) -> Result<(), CliError> {
    sync::say(dir, text)
}

/// Message texts in event-id order, one string each. Empty when there is no conversation yet.
///
/// # Errors
///
/// Returns [`CliError`] when the chat log cannot be opened.
pub fn chat_lines(dir: &Path) -> Result<Vec<String>, CliError> {
    let text = sync::hear(dir)?;
    Ok(text
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect())
}
