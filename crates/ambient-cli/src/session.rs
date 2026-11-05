//! The notebook both the CLI and the desktop window use.

use std::path::Path;

use ambient_events::{EventId, encode_unsigned, sign_wire, verify};
use ambient_identity::{
    DeviceState, decode_pair_offer_text, device_add_payload, project_log, revoke_payload,
};
use ambient_spaces::{
    GenesisMember, Revocation, member_add_payload, member_remove_payload, note_put_payload,
    notes_genesis_payload, project_notes_for_root,
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
    /// Label of the device that wrote the first head. Empty when unknown.
    pub writer: String,
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
    /// Label of the device that wrote this head.
    pub writer: String,
}

/// One device on the identity log.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceLine {
    /// Hex signing key.
    pub id: String,
    /// Local label. Not sent to a relay.
    pub label: String,
    /// True after `id.device-revoke`.
    pub revoked: bool,
    /// True for the key in this folder.
    pub current: bool,
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
            .as_ref()
            .ok_or_else(|| CliError::message("could not sign genesis"))?
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
            .as_ref()
            .ok_or_else(|| CliError::message("could not sign device"))?
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

    /// Notes in id order. An empty file, or no file yet, is an empty list.
    ///
    /// # Errors
    ///
    /// Returns [`CliError`] when the log cannot be projected.
    pub fn list(&self) -> Result<Vec<NoteSummary>, CliError> {
        if load(&self.local)?.is_empty() {
            return Ok(Vec::new());
        }
        let view = self.project()?;
        let names = device_labels(&self.local)?;
        Ok(view
            .notes
            .iter()
            .map(|(id, heads)| summary(id, heads, &names))
            .collect())
    }

    /// Notes matching `query`.
    ///
    /// Uses the SQLCipher FTS5 index when it opens. Words are matched
    /// case-insensitively. If the index cannot be built, falls back to a
    /// case-sensitive scan of the projection.
    ///
    /// # Errors
    ///
    /// Returns [`CliError`] when the query is empty or the log cannot be projected.
    pub fn search(&self, query: &str) -> Result<Vec<NoteSummary>, CliError> {
        if query.is_empty() {
            return Err(CliError::message("empty query"));
        }
        if load(&self.local)?.is_empty() {
            return Ok(Vec::new());
        }
        let view = self.project()?;
        let names = device_labels(&self.local)?;
        let Some(dir) = self.local.key_path.parent() else {
            return Ok(search_scan(&view, query, &names));
        };
        let mut rows = Vec::new();
        for (id, heads) in &view.notes {
            let mut title = String::new();
            let mut body = String::new();
            for head in heads {
                title.push_str(&head.title);
                title.push('\n');
                body.push_str(&head.body);
                body.push('\n');
            }
            rows.push((hex(id), title, body));
        }
        match crate::index::match_ids(dir, &self.local.store_key, &rows, query) {
            Ok(ids) => {
                let wanted: std::collections::BTreeSet<String> = ids.into_iter().collect();
                Ok(view
                    .notes
                    .iter()
                    .filter(|(id, _)| wanted.contains(&hex(*id)))
                    .map(|(id, heads)| summary(id, heads, &names))
                    .collect())
            }
            Err(_) => Ok(search_scan(&view, query, &names)),
        }
    }

    /// Open one note, including every conflicting head.
    ///
    /// # Errors
    ///
    /// Returns [`CliError`] when the id is not 64 hex characters or the note does not exist.
    pub fn show(&self, id_hex: &str) -> Result<ShownNote, CliError> {
        let note_id = parse_hex32(id_hex)?;
        let view = self.project()?;
        let names = device_labels(&self.local)?;
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
                    writer: names
                        .get(&head.signer)
                        .cloned()
                        .unwrap_or_else(|| short_hex(&head.signer)),
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
        if wires.is_empty() {
            return Err(CliError::message("sync this device before writing a note"));
        }
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

    /// True when this folder holds the root. A paired device does not.
    #[must_use]
    pub fn holds_root(&self) -> bool {
        self.local.identity.is_some()
    }

    /// Add this folder's device to the identity log and the Notes space.
    ///
    /// Used after a phrase restore. The identity key signs both events. The
    /// device key is new. Old device keys stay until the owner revokes them.
    ///
    /// # Errors
    ///
    /// Returns [`CliError`] when this folder does not hold the root or the log rejects the events.
    pub fn enroll_self(&self) -> Result<(), CliError> {
        let identity = self
            .local
            .identity
            .as_ref()
            .ok_or_else(|| CliError::message("only the root folder can enroll a device"))?;
        let mut wires = load(&self.local)?;
        let log = project_log(
            &self.local.identity_public,
            &identity_wires(&self.local, &wires)?,
        )
        .map_err(|_| CliError::message("identity log rejected"))?;
        let mut head = log.head;
        if !log.devices.contains_key(&self.local.device.signing_key) {
            let payload = device_add_payload(
                &self.local.device.signing_key,
                &self.local.device.encryption_key,
                &self.local.device.label,
                &self.local.device.mailbox,
            )
            .map_err(|_| CliError::message("could not encode device"))?;
            let add = identity
                .sign_log(head, "id.device-add", &payload)
                .map_err(|_| CliError::message("could not sign device"))?;
            let event = verify(&add).map_err(|_| CliError::message("could not sign device"))?;
            head = Some(event.id);
            wires.push(add);
        }
        let view = view_notes(&self.local, &wires)?;
        if !view.members.contains_key(&self.local.device.signing_key) {
            let member = GenesisMember {
                signing_key: self.local.device.signing_key,
                encryption_key: self.local.device.encryption_key,
                role: 1,
                mailbox: self.local.device.mailbox,
            };
            let payload = member_add_payload(&member)
                .map_err(|_| CliError::message("could not encode member"))?;
            let unsigned = encode_unsigned(
                &identity.signing_key,
                head.map(|id| id.0),
                None,
                Some(view.space_id.0),
                "space.member-add",
                &payload,
            )
            .map_err(|_| CliError::message("could not encode member"))?;
            let add = sign_wire(&unsigned, identity.signing_seed())
                .map_err(|_| CliError::message("could not sign member"))?;
            wires.push(add);
        }
        write_log(&self.local.notes_path, &self.local.store_key, &wires)
            .map_err(|_| CliError::message("could not write notes file"))?;
        Ok(())
    }

    /// Authorize a device from its offer and add it to the Notes space.
    ///
    /// The offer is the text the other device printed. This folder must hold the root.
    ///
    /// # Errors
    ///
    /// Returns [`CliError`] when the offer is bad, this folder is not the owner, or the log rejects the events.
    pub fn pair_device(&self, offer_text: &str, label: &str) -> Result<(), CliError> {
        if self.local.identity.is_none() {
            return Err(CliError::message(
                "only the folder with the root can pair a device",
            ));
        }
        let offer = decode_pair_offer_text(offer_text.trim())
            .map_err(|_| CliError::message("offer rejected"))?;
        let mut wires = load(&self.local)?;
        let identity_wires = identity_wires(&self.local, &wires)?;
        let head = project_log(&self.local.identity_public, &identity_wires)
            .map_err(|_| CliError::message("identity log rejected"))?
            .head;
        let payload = device_add_payload(
            &offer.signing_key,
            &offer.encryption_key,
            label,
            &offer.mailbox,
        )
        .map_err(|_| CliError::message("label must be 1 to 32 bytes"))?;
        let add = self
            .local
            .identity
            .as_ref()
            .ok_or_else(|| CliError::message("only the folder with the root can pair a device"))?
            .sign_log(head, "id.device-add", &payload)
            .map_err(|_| CliError::message("could not sign device"))?;
        wires.push(add);
        let view = view_notes(&self.local, &wires)?;
        let member = ambient_spaces::GenesisMember {
            signing_key: offer.signing_key,
            encryption_key: offer.encryption_key,
            role: 2,
            mailbox: offer.mailbox,
        };
        let payload = member_add_payload(&member)
            .map_err(|_| CliError::message("could not encode member"))?;
        let prev = view.tip.get(&self.local.device.signing_key).copied();
        wires.push(sign_device(
            &self.local,
            prev,
            Some(view.space_id),
            "space.member-add",
            &payload,
        )?);
        write_log(&self.local.notes_path, &self.local.store_key, &wires)
            .map_err(|_| CliError::message("could not write notes file"))?;
        Ok(())
    }

    /// Current Notes-space members. Empty when this folder has no events yet.
    ///
    /// # Errors
    ///
    /// Returns [`CliError`] when the log does not project.
    pub fn recipients(&self) -> Result<Vec<ambient_spaces::MemberInfo>, CliError> {
        let wires = load(&self.local)?;
        if wires.is_empty() {
            return Ok(Vec::new());
        }
        Ok(view_notes(&self.local, &wires)?
            .members
            .into_values()
            .collect())
    }

    /// Devices on the identity log. A paired folder can read the list. Only the root can revoke.
    ///
    /// # Errors
    ///
    /// Returns [`CliError`] when the identity log does not project.
    pub fn devices(&self) -> Result<Vec<DeviceLine>, CliError> {
        let wires = load(&self.local)?;
        let log = project_log(
            &self.local.identity_public,
            &identity_wires(&self.local, &wires)?,
        )
        .map_err(|_| CliError::message("identity log rejected"))?;
        let mut lines = Vec::new();
        for (key, state) in log.devices {
            let (label, revoked) = match state {
                DeviceState::Active { label, .. } => (label, false),
                DeviceState::Revoked { label, .. } => (label, true),
            };
            lines.push(DeviceLine {
                id: hex(&key),
                label,
                revoked,
                current: key == self.local.device.signing_key,
            });
        }
        Ok(lines)
    }

    /// Revoke a device key. Keeps the notes it already wrote. Stops sealing new events to it.
    ///
    /// # Errors
    ///
    /// Returns [`CliError`] when this folder is not the root, the key is this device, or it is already revoked.
    pub fn revoke_device(&self, id_hex: &str) -> Result<(), CliError> {
        if self.local.identity.is_none() {
            return Err(CliError::message(
                "only the folder with the root can revoke a device",
            ));
        }
        let key = parse_hex32(id_hex)?;
        if key == self.local.device.signing_key {
            return Err(CliError::message("this device cannot revoke itself"));
        }
        let mut wires = load(&self.local)?;
        let log = project_log(
            &self.local.identity_public,
            &identity_wires(&self.local, &wires)?,
        )
        .map_err(|_| CliError::message("identity log rejected"))?;
        match log.devices.get(&key) {
            Some(DeviceState::Active { .. }) => {}
            Some(DeviceState::Revoked { .. }) => {
                return Err(CliError::message("that device is already revoked"));
            }
            None => return Err(CliError::message("no such device")),
        }
        let view = view_notes(&self.local, &wires)?;
        let payload = revoke_payload(&key, view.tip.get(&key).copied())
            .map_err(|_| CliError::message("could not encode revoke"))?;
        let revoke = self
            .local
            .identity
            .as_ref()
            .ok_or_else(|| CliError::message("only the folder with the root can revoke a device"))?
            .sign_log(log.head, "id.device-revoke", &payload)
            .map_err(|_| CliError::message("could not sign revoke"))?;
        wires.push(revoke);
        if view.members.contains_key(&key) {
            let payload = member_remove_payload(&key)
                .map_err(|_| CliError::message("could not encode member remove"))?;
            let prev = view.tip.get(&self.local.device.signing_key).copied();
            wires.push(sign_device(
                &self.local,
                prev,
                Some(view.space_id),
                "space.member-remove",
                &payload,
            )?);
        }
        write_log(&self.local.notes_path, &self.local.store_key, &wires)
            .map_err(|_| CliError::message("could not write notes file"))?;
        Ok(())
    }

    fn project(&self) -> Result<ambient_spaces::Projection, CliError> {
        view_notes(&self.local, &load(&self.local)?)
    }
}

fn search_scan(
    view: &ambient_spaces::Projection,
    query: &str,
    names: &std::collections::BTreeMap<[u8; 32], String>,
) -> Vec<NoteSummary> {
    let mut found = Vec::new();
    for (id, heads) in &view.notes {
        if heads
            .iter()
            .any(|head| head.title.contains(query) || head.body.contains(query))
        {
            found.push(summary(id, heads, names));
        }
    }
    found
}

fn summary(
    id: &[u8; 32],
    heads: &[ambient_spaces::NoteHead],
    names: &std::collections::BTreeMap<[u8; 32], String>,
) -> NoteSummary {
    let writer = heads.first().map_or(String::new(), |head| {
        names
            .get(&head.signer)
            .cloned()
            .unwrap_or_else(|| short_hex(&head.signer))
    });
    NoteSummary {
        id: hex(id),
        title: heads
            .first()
            .map_or(String::new(), |head| head.title.clone()),
        writer,
        conflict: heads.len() > 1,
    }
}

fn device_labels(local: &Local) -> Result<std::collections::BTreeMap<[u8; 32], String>, CliError> {
    let wires = load(local)?;
    let log = project_log(&local.identity_public, &identity_wires(local, &wires)?)
        .map_err(|_| CliError::message("identity log rejected"))?;
    let mut names = std::collections::BTreeMap::new();
    for (key, state) in log.devices {
        let label = match state {
            DeviceState::Active { label, .. } | DeviceState::Revoked { label, .. } => label,
        };
        names.insert(key, label);
    }
    Ok(names)
}

fn short_hex(bytes: &[u8; 32]) -> String {
    hex(bytes).chars().take(8).collect()
}

fn identity_wires<'a>(local: &Local, wires: &'a [Vec<u8>]) -> Result<Vec<&'a [u8]>, CliError> {
    let mut out = Vec::new();
    for wire in wires {
        let event = verify(wire).map_err(|_| CliError::message("identity log rejected"))?;
        if event.signer == local.identity_public {
            out.push(wire.as_slice());
        }
    }
    Ok(out)
}

fn view_notes(local: &Local, wires: &[Vec<u8>]) -> Result<ambient_spaces::Projection, CliError> {
    let refs: Vec<&[u8]> = wires.iter().map(Vec::as_slice).collect();
    let identity_wires: Vec<&[u8]> = refs
        .iter()
        .copied()
        .filter(|wire| {
            verify(wire)
                .ok()
                .is_some_and(|event| event.signer == local.identity_public)
        })
        .collect();
    let log = project_log(&local.identity_public, &identity_wires)
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
            verify(wire).ok().is_some_and(|event| {
                event.signer != local.identity_public || event.event_type == "space.member-add"
            })
        })
        .collect();
    project_notes_for_root(&space_wires, &revoked, Some(local.identity_public))
        .map_err(|_| CliError::message("notes log rejected"))
}

fn load(local: &Local) -> Result<Vec<Vec<u8>>, CliError> {
    if !local.notes_path.exists() {
        return Ok(Vec::new());
    }
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
