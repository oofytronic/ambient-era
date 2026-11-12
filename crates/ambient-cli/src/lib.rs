//! Notes, devices, and conversations on one machine.
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
mod index;
mod keys;
mod notes;
mod session;
mod sync;

pub use draft::{Draft, Shortcut, shortcut};
pub use notes::{CliError, run};
pub use session::{
    DELETED_BODY, DeviceLine, NoteSummary, Notebook, ShownHead, ShownNote, note_is_deleted,
};

pub use sync::{INTRODUCTION, NetMode, NetSetting, check_relay_url};

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

/// Ask the relay this process started to stop. The port frees after the accept loop notices.
///
/// # Errors
///
/// Returns [`CliError`] when this process did not bind the relay.
pub fn stop_loopback_relay() -> Result<(), CliError> {
    sync::stop_loopback()
}

/// True when this process bound the loopback relay and has not stopped it.
#[must_use]
pub fn loopback_is_ours() -> bool {
    sync::relay_is_ours()
}

/// True when a local event has not been sealed to another member, or the recovery copy is behind.
///
/// # Errors
///
/// Returns [`CliError`] when the folder cannot be opened.
pub fn has_unsent(dir: &Path) -> Result<bool, CliError> {
    sync::has_unsent(dir)
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

/// Push and pull through the relay this folder remembers.
///
/// This computer is the default. A pinned relay is tried before its backup.
/// The introduction list is tried when that mode is selected. A failure to
/// reach one relay tries the next. The note log is not touched by a failure.
///
/// # Errors
///
/// Returns [`CliError`] when no relay answers, or a log cannot be written.
pub fn sync_notebook(dir: &Path) -> Result<String, CliError> {
    sync::sync_auto(dir)
}

/// The folder's relay choice. Missing file means this computer.
///
/// # Errors
///
/// Returns [`CliError`] when the file cannot be read.
pub fn net_setting(dir: &Path) -> Result<NetSetting, CliError> {
    sync::read_setting(dir)
}

/// Remember how this folder reaches a relay. An address off this computer has to be `wss`.
///
/// # Errors
///
/// Returns [`CliError`] when the address is refused or the file cannot be written.
pub fn set_net_setting(dir: &Path, setting: &NetSetting) -> Result<(), CliError> {
    sync::write_setting(dir, setting)
}

/// This folder's contact card. Not a device offer. Do not pass it to [`Notebook::pair_device`].
///
/// # Errors
///
/// Returns [`CliError`] when the card cannot be read or written.
pub fn contact_card(dir: &Path) -> Result<String, CliError> {
    sync::card(dir)
}

/// A shareable link for this folder's contact card.
///
/// The link is `ambient://contact/` followed by the same base64url the card uses.
/// Scanning a QR of this link, or pasting the link, starts a conversation the same way the raw card does.
///
/// # Errors
///
/// Returns [`CliError`] when the card cannot be read.
pub fn contact_link(dir: &Path) -> Result<String, CliError> {
    let card = sync::card(dir)?;
    Ok(format!("ambient://contact/{card}"))
}

/// Open a conversation with the other person's card. Returns the Space id.
///
/// A folder may hold more than one conversation. Each one has its own contact key.
///
/// # Errors
///
/// Returns [`CliError`] when the card is this device, that invite was already used, or the log cannot be written.
pub fn start_conversation(dir: &Path, card: &str) -> Result<String, CliError> {
    sync::talk(dir, card)
}

/// Append one chat message to the only conversation. It stays local until [`sync_folder`].
///
/// # Errors
///
/// Returns [`CliError`] when there is not exactly one conversation or the message is too long.
pub fn send_chat(dir: &Path, text: &str) -> Result<(), CliError> {
    sync::say(dir, text)
}

/// Append one chat message to the conversation `id`.
///
/// # Errors
///
/// Returns [`CliError`] when that conversation does not exist or the message is too long.
pub fn send_chat_in(dir: &Path, id: &str, text: &str) -> Result<(), CliError> {
    sync::say_in(dir, id, text)
}

/// Message texts in the only conversation. Empty when there is none.
///
/// # Errors
///
/// Returns [`CliError`] when there is more than one conversation or the log cannot be opened.
pub fn chat_lines(dir: &Path) -> Result<Vec<String>, CliError> {
    let text = sync::hear(dir)?;
    Ok(text
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect())
}

/// One conversation. `id` is the Space id. `label` is a short local name for the other key, not a global user id. `other` is the start of that key, so a rename can keep it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversationSummary {
    /// Hex Space id.
    pub id: String,
    /// Short label for the other member.
    pub label: String,
    /// First bytes of the other member's key, hex. Empty when unknown.
    pub other: String,
}

/// Conversations in this folder, in storage order.
///
/// # Errors
///
/// Returns [`CliError`] when a chat log cannot be opened.
pub fn list_conversations(dir: &Path) -> Result<Vec<ConversationSummary>, CliError> {
    Ok(sync::conversations(dir)?
        .into_iter()
        .map(|(id, label, other)| ConversationSummary { id, label, other })
        .collect())
}

/// One chat message for the window, oldest first.
///
/// `unix_ms` is the envelope's advisory clock. It is not the order of the log.
/// The vec is already in reading order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChatMessage {
    /// Message text. A copy, not a grant.
    pub text: String,
    /// Advisory unix milliseconds. `None` when the event did not set one.
    pub unix_ms: Option<u64>,
    /// True when this folder's contact card for that conversation signed it.
    pub mine: bool,
}

/// Message texts in one conversation, oldest first.
///
/// A message that contains a newline is split across strings. The window uses
/// [`chat_messages_in`] so one message stays one bubble.
///
/// # Errors
///
/// Returns [`CliError`] when that conversation does not exist.
pub fn chat_lines_in(dir: &Path, id: &str) -> Result<Vec<String>, CliError> {
    let text = sync::hear_in(dir, id)?;
    Ok(text
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect())
}

/// Messages in one conversation, oldest first.
///
/// # Errors
///
/// Returns [`CliError`] when that conversation does not exist.
pub fn chat_messages_in(dir: &Path, id: &str) -> Result<Vec<ChatMessage>, CliError> {
    sync::messages_in(dir, id)
}

/// Wrap the key file with a passphrase. The passphrase is not the root and not a passkey.
///
/// # Errors
///
/// Returns [`CliError`] when the passphrase is shorter than 8 characters, or the folder is locked
/// in this process.
pub fn set_folder_unlock(dir: &Path, passphrase: &str) -> Result<(), CliError> {
    keys::set_unlock(dir, passphrase)
}

/// Unlock a wrapped key file for this process. The unwrapped root is not written back to disk.
///
/// # Errors
///
/// Returns [`CliError`] when the passphrase is wrong or this folder is not wrapped.
pub fn unlock_folder(dir: &Path, passphrase: &str) -> Result<(), CliError> {
    keys::unlock(dir, passphrase)
}

/// The recovery phrase for this notebook. Only the root folder has one.
///
/// # Errors
///
/// Returns [`CliError`] when this folder does not hold the root or the phrase cannot be encoded.
pub fn recovery_phrase(dir: &Path) -> Result<String, CliError> {
    let local = keys::open(dir)?;
    let Some(_identity) = &local.identity else {
        return Err(CliError::message(
            "only the root folder has a recovery phrase",
        ));
    };
    // The root is not stored in Identity; we need to re-read it.
    // For v0.1 we re-derive from the key file.
    let bytes =
        std::fs::read(&local.key_path).map_err(|_| CliError::message("missing key file"))?;
    if bytes.get(4) != Some(&1) {
        return Err(CliError::message("paired device has no recovery phrase"));
    }
    let root = keys::copy32_pub(&bytes, 5)?;
    keys::recovery_phrase(&root)
}

/// Restore a new folder from a recovery phrase by fetching the sealed log.
///
/// # Errors
///
/// Returns [`CliError`] when the phrase is bad or the relay has no recovery.
pub fn restore_folder(dir: &Path, phrase: &str, url: &str) -> Result<(), CliError> {
    sync::restore(dir, phrase, url)
}
