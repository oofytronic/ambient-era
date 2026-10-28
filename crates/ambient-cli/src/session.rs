//! The notebook both the CLI and the desktop window use.

use std::path::Path;

use ambient_events::{EventId, encode_unsigned, sign_wire, verify};
use ambient_identity::{DeviceState, device_add_payload, project_log};
use ambient_spaces::{
    GenesisMember, Revocation, note_put_payload, notes_genesis_payload, project_notes,
};
use ambient_store::{read_log, write_log};

use crate::keys::{self, Local};
use crate::notes::CliError;

/// One note in a list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoteSummary {
    /// Hex note id.
    pub id: String,
    /// Title of the first head. Empty when the note has no title.
    pub title: String,
    /// More than one head is a conflict. The window must not pick a winner.
    pub conflict: bool,
}

/// One surviving snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShownHead {
    /// Title of this head.
    pub title: String,
    /// Markdown body.
    pub body: String,
}

/// A note opened for reading or editing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShownNote {
    /// Hex note id.
    pub id: String,
    /// True when more than one head is current.
    pub conflict: bool,
    /// Every current head, in event-id order.
    pub heads: Vec<ShownHead>,
}

/// An open notebook directory. The same files the CLI reads.
pub struct Notebook {
    local: Local,
}

impl Notebook {
    /// Create a new notebook. Fails if `dir/key` already exists.
    ///
    /// # Errors
    ///
    /// Returns [`CliError`] when the directory or the first events cannot be written.
    pub fn create(dir: &Path) -> Result<Self, CliError> {
        let local = keys::create(dir, "this-computer")?;
        let genesis = local
            .identity
            .genesis_wire()
            .map_err(|_| CliError::message("could not sign genesis"))?;
        let genesis_id = verify(&genesis)
            .map_err(|_| CliError::message("genesis did not verify"))?
            .id;
        let add_payload = device_add_payload(
            &local.device.signing_key,
            &local.device.encryption_key,
            &local.device.label,
            &local.device.mailbox,
        )
        .map_err(|_| CliError::message("could not encode device"))?;
        let add = local
            .identity
            .sign_log(Some(genesis_id), "id.device-add", &add_payload)
            .map_err(|_| CliError::message("could not sign device"))?;
        let space_payload = notes_genesis_payload(&[GenesisMember {
            signing_key: local.device.signing_key,
            encryption_key: local.device.encryption_key,
            role: 1,
            mailbox: local.device.mailbox,
        }])
        .map_err(|_| CliError::message("could not encode notes space"))?;
        let space = sign_device(&local, None, None, "space.genesis", &space_payload)?;
        write_log(&local.notes_path, &local.store_key, &[genesis, add, space])
            .map_err(|_| CliError::message("could not write notes file"))?;
        Ok(Self { local })
    }

    /// Open a notebook created earlier, including one created by the CLI.
    ///
    /// # Errors
    ///
    /// Returns [`CliError`] when the key file is missing or the log does not open.
    pub fn open(dir: &Path) -> Result<Self, CliError> {
        let local = keys::open(dir)?;
        let _ = load(&local)?;
        Ok(Self { local })
    }

    /// Notes in id order.
    ///
    /// # Errors
    ///
    /// Returns [`CliError`] when the log cannot be projected.
    pub fn list(&self) -> Result<Vec<NoteSummary>, CliError> {
        let view = self.project()?;
        Ok(view
            .notes
            .iter()
            .map(|(id, heads)| summary(id, heads))
            .collect())
    }

    /// Notes whose title or body contains `query`. The match is case-sensitive.
    ///
    /// # Errors
    ///
    /// Returns [`CliError`] when the query is empty or the log cannot be projected.
    pub fn search(&self, query: &str) -> Result<Vec<NoteSummary>, CliError> {
        if query.is_empty() {
            return Err(CliError::message("empty query"));
        }
        let view = self.project()?;
        let mut found = Vec::new();
        for (id, heads) in &view.notes {
            if heads
                .iter()
                .any(|head| head.title.contains(query) || head.body.contains(query))
            {
                found.push(summary(id, heads));
            }
        }
        Ok(found)
    }

    /// Open one note, including every conflicting head.
    ///
    /// # Errors
    ///
    /// Returns [`CliError`] when the id is not 64 hex characters or the note does not exist.
    pub fn show(&self, id_hex: &str) -> Result<ShownNote, CliError> {
        let note_id = parse_hex32(id_hex)?;
        let view = self.project()?;
        let heads = view
            .notes
            .get(&note_id)
            .ok_or_else(|| CliError::message("no such note"))?;
        Ok(ShownNote {
            id: hex(&note_id),
            conflict: heads.len() > 1,
            heads: heads
                .iter()
                .map(|head| ShownHead {
                    title: head.title.clone(),
                    body: head.body.clone(),
                })
                .collect(),
        })
    }

    /// Append a new note. Returns its hex id.
    ///
    /// # Errors
    ///
    /// Returns [`CliError`] when the title is too long or the log cannot be written.
    pub fn add(&self, title: &str, body: &str) -> Result<String, CliError> {
        let mut wires = load(&self.local)?;
        let view = view_notes(&self.local, &wires)?;
        let note_id = random_note()?;
        let payload = note_put_payload(&note_id, title, body, &[])
            .map_err(|_| CliError::message("title must be at most 200 bytes"))?;
        let prev = view.tip.get(&self.local.device.signing_key).copied();
        wires.push(sign_device(
            &self.local,
            prev,
            Some(view.space_id),
            "note.put",
            &payload,
        )?);
        write_log(&self.local.notes_path, &self.local.store_key, &wires)
            .map_err(|_| CliError::message("could not write notes file"))?;
        Ok(hex(&note_id))
    }

    /// Append a snapshot. Parents are every current head, so this resolves a conflict.
    ///
    /// # Errors
    ///
    /// Returns [`CliError`] when the note does not exist or the title is too long.
    pub fn edit(&self, id_hex: &str, title: &str, body: &str) -> Result<(), CliError> {
        let note_id = parse_hex32(id_hex)?;
        let mut wires = load(&self.local)?;
        let view = view_notes(&self.local, &wires)?;
        let heads = view
            .notes
            .get(&note_id)
            .ok_or_else(|| CliError::message("no such note"))?;
        let parents: Vec<EventId> = heads.iter().map(|head| head.event_id).collect();
        let payload = note_put_payload(&note_id, title, body, &parents)
            .map_err(|_| CliError::message("title must be at most 200 bytes"))?;
        let prev = view.tip.get(&self.local.device.signing_key).copied();
        wires.push(sign_device(
            &self.local,
            prev,
            Some(view.space_id),
            "note.put",
            &payload,
        )?);
        write_log(&self.local.notes_path, &self.local.store_key, &wires)
            .map_err(|_| CliError::message("could not write notes file"))?;
        Ok(())
    }

    fn project(&self) -> Result<ambient_spaces::Projection, CliError> {
        view_notes(&self.local, &load(&self.local)?)
    }
}

fn summary(id: &[u8; 32], heads: &[ambient_spaces::NoteHead]) -> NoteSummary {
    NoteSummary {
        id: hex(id),
        title: heads
            .first()
            .map_or(String::new(), |head| head.title.clone()),
        conflict: heads.len() > 1,
    }
}

fn view_notes(local: &Local, wires: &[Vec<u8>]) -> Result<ambient_spaces::Projection, CliError> {
    let refs: Vec<&[u8]> = wires.iter().map(Vec::as_slice).collect();
    let identity_wires: Vec<&[u8]> = refs
        .iter()
        .copied()
        .filter(|wire| {
            verify(wire)
                .ok()
                .is_some_and(|event| event.signer == local.identity.signing_key)
        })
        .collect();
    let log = project_log(&local.identity.signing_key, &identity_wires)
        .map_err(|_| CliError::message("identity log rejected"))?;
    let mut revoked = Vec::new();
    for (key, state) in &log.devices {
        if let DeviceState::Revoked { up_to, .. } = state {
            revoked.push(Revocation {
                signing_key: *key,
                up_to: *up_to,
            });
        }
    }
    let space_wires: Vec<&[u8]> = refs
        .iter()
        .copied()
        .filter(|wire| {
            verify(wire)
                .ok()
                .is_some_and(|event| event.signer != local.identity.signing_key)
        })
        .collect();
    project_notes(&space_wires, &revoked).map_err(|_| CliError::message("notes log rejected"))
}

fn load(local: &Local) -> Result<Vec<Vec<u8>>, CliError> {
    read_log(&local.notes_path, &local.store_key)
        .map_err(|_| CliError::message("could not open notes file"))
}

fn sign_device(
    local: &Local,
    prev: Option<EventId>,
    space: Option<EventId>,
    event_type: &str,
    payload: &[u8],
) -> Result<Vec<u8>, CliError> {
    let unsigned = encode_unsigned(
        &local.device.signing_key,
        prev.map(|id| id.0),
        None,
        space.map(|id| id.0),
        event_type,
        payload,
    )
    .map_err(|_| CliError::message("could not encode event"))?;
    sign_wire(&unsigned, local.device.signing_seed())
        .map_err(|_| CliError::message("could not sign event"))
}

fn random_note() -> Result<[u8; 32], CliError> {
    let mut bytes = [0_u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|_| CliError::message("random source failed"))?;
    Ok(bytes)
}

pub(crate) fn parse_hex32(text: &str) -> Result<[u8; 32], CliError> {
    if text.len() != 64 {
        return Err(CliError::message("note id must be 64 hex characters"));
    }
    let mut out = [0_u8; 32];
    for (index, slot) in out.iter_mut().enumerate() {
        let start = index.saturating_mul(2);
        let pair = text
            .get(start..start + 2)
            .ok_or_else(|| CliError::message("note id must be 64 hex characters"))?;
        *slot = u8::from_str_radix(pair, 16)
            .map_err(|_| CliError::message("note id must be 64 hex characters"))?;
    }
    Ok(out)
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let hi = usize::from(byte >> 4);
        let lo = usize::from(byte & 0x0f);
        out.push(char::from(HEX.get(hi).copied().unwrap_or(b'0')));
        out.push(char::from(HEX.get(lo).copied().unwrap_or(b'0')));
    }
    out
}
