//! The notebook both the CLI and the desktop window use.

use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
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

/// Body of a snapshot that means the note was deleted.
///
/// The event is still `note.put`. The list hides the note when every current
/// head is only this marker. The earlier text stays in the log.
pub const DELETED_BODY: &str = "%%ambient-deleted%%";

/// True when a snapshot body is the delete marker and nothing else.
#[must_use]
pub fn note_is_deleted(body: &str) -> bool {
    body.trim() == DELETED_BODY
}

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
    /// Tags written in the body as `#name`. A view, not a separate record.
    pub tags: Vec<String>,
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
    /// Last projection. Reused until [`log_stamp`] changes, so a click does not verify again.
    cache: RefCell<Option<NoteCache>>,
}

struct NoteCache {
    stamp: Option<(u64, u64)>,
    wires: Vec<Vec<u8>>,
    projected: Projected,
}

struct Projected {
    view: Option<ambient_spaces::Projection>,
    names: BTreeMap<[u8; 32], String>,
    devices: Vec<DeviceLine>,
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
        Ok(Self {
            local,
            cache: RefCell::new(None),
        })
    }

    /// Open a notebook created earlier, including one created by the CLI.
    ///
    /// # Errors
    ///
    /// Returns [`CliError`] when the key file is missing or the log does not open.
    pub fn open(dir: &Path) -> Result<Self, CliError> {
        let local = keys::open(dir)?;
        let book = Self {
            local,
            cache: RefCell::new(None),
        };
        book.ensure()?;
        Ok(book)
    }

    /// Notes in id order. An empty file, or no file yet, is an empty list.
    ///
    /// # Errors
    ///
    /// Returns [`CliError`] when the log cannot be projected.
    pub fn list(&self) -> Result<Vec<NoteSummary>, CliError> {
        self.ensure()?;
        let guard = self.cache.borrow();
        let Some(item) = guard.as_ref() else {
            return Ok(Vec::new());
        };
        let Some(view) = item.projected.view.as_ref() else {
            return Ok(Vec::new());
        };
        Ok(view
            .notes
            .iter()
            .filter(|(_, heads)| !heads_are_deleted(heads))
            .map(|(id, heads)| summary(id, heads, &item.projected.names))
            .collect())
    }

    /// Walk each visible note without cloning its body.
    ///
    /// The closure must not call back into this notebook.
    ///
    /// # Errors
    ///
    /// Returns [`CliError`] when the log cannot be projected.
    pub fn visit_notes(
        &self,
        mut visit: impl FnMut(&str, &str, &str, bool),
    ) -> Result<(), CliError> {
        self.ensure()?;
        let guard = self.cache.borrow();
        let Some(item) = guard.as_ref() else {
            return Ok(());
        };
        let Some(view) = item.projected.view.as_ref() else {
            return Ok(());
        };
        for (id, heads) in &view.notes {
            if heads_are_deleted(heads) {
                continue;
            }
            let hex_id = hex(id);
            let title = heads.first().map(|head| head.title.as_str()).unwrap_or("");
            let body = heads.first().map(|head| head.body.as_str()).unwrap_or("");
            visit(&hex_id, title, body, heads.len() > 1);
        }
        Ok(())
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
        self.ensure()?;
        let stamp = {
            let guard = self.cache.borrow();
            let Some(item) = guard.as_ref() else {
                return Ok(Vec::new());
            };
            let Some(view) = item.projected.view.as_ref() else {
                return Ok(Vec::new());
            };
            notes_stamp(view)
        };
        let Some(dir) = self.local.key_path.parent() else {
            return self.search_cloned(query);
        };
        if let Some(found) = crate::index::match_fresh(dir, stamp, query) {
            return match found {
                Ok(ids) => self.summaries_for(ids),
                Err(_) => self.search_cloned(query),
            };
        }
        let (view, names) = {
            let guard = self.cache.borrow();
            let Some(item) = guard.as_ref() else {
                return Ok(Vec::new());
            };
            let Some(view) = item.projected.view.clone() else {
                return Ok(Vec::new());
            };
            (view, item.projected.names.clone())
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
        match crate::index::match_ids(dir, &self.local.store_key, &rows, query, stamp) {
            Ok(ids) => {
                let wanted: BTreeSet<String> = ids.into_iter().collect();
                Ok(view
                    .notes
                    .iter()
                    .filter(|(id, heads)| wanted.contains(&hex(*id)) && !heads_are_deleted(heads))
                    .map(|(id, heads)| summary(id, heads, &names))
                    .collect())
            }
            Err(_) => Ok(search_scan(&view, query, &names)),
        }
    }

    fn search_cloned(&self, query: &str) -> Result<Vec<NoteSummary>, CliError> {
        let guard = self.cache.borrow();
        let Some(item) = guard.as_ref() else {
            return Ok(Vec::new());
        };
        let Some(view) = item.projected.view.as_ref() else {
            return Ok(Vec::new());
        };
        Ok(search_scan(view, query, &item.projected.names))
    }

    fn summaries_for(&self, ids: Vec<String>) -> Result<Vec<NoteSummary>, CliError> {
        let wanted: BTreeSet<String> = ids.into_iter().collect();
        let guard = self.cache.borrow();
        let Some(item) = guard.as_ref() else {
            return Ok(Vec::new());
        };
        let Some(view) = item.projected.view.as_ref() else {
            return Ok(Vec::new());
        };
        Ok(view
            .notes
            .iter()
            .filter(|(id, heads)| wanted.contains(&hex(*id)) && !heads_are_deleted(heads))
            .map(|(id, heads)| summary(id, heads, &item.projected.names))
            .collect())
    }

    /// Open one note, including every conflicting head.
    ///
    /// # Errors
    ///
    /// Returns [`CliError`] when the id is not 64 hex characters or the note does not exist.
    pub fn show(&self, id_hex: &str) -> Result<ShownNote, CliError> {
        let note_id = parse_hex32(id_hex)?;
        self.ensure()?;
        let guard = self.cache.borrow();
        let Some(item) = guard.as_ref() else {
            return Err(CliError::message("no such note"));
        };
        let Some(view) = item.projected.view.as_ref() else {
            return Err(CliError::message("no such note"));
        };
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
                    writer: item
                        .projected
                        .names
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
        self.ensure()?;
        let (mut wires, view) = {
            let guard = self.cache.borrow();
            let Some(item) = guard.as_ref() else {
                return Err(CliError::message("could not open notes file"));
            };
            if item.wires.is_empty() {
                return Err(CliError::message("sync this device before writing a note"));
            }
            let view = item
                .projected
                .view
                .clone()
                .ok_or_else(|| CliError::message("notes log rejected"))?;
            (item.wires.clone(), view)
        };
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
        self.ensure()?;
        let (mut wires, view) = {
            let guard = self.cache.borrow();
            let Some(item) = guard.as_ref() else {
                return Err(CliError::message("could not open notes file"));
            };
            let view = item
                .projected
                .view
                .clone()
                .ok_or_else(|| CliError::message("no such note"))?;
            (item.wires.clone(), view)
        };
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
        self.ensure()?;
        let guard = self.cache.borrow();
        let Some(view) = guard.as_ref().and_then(|item| item.projected.view.as_ref()) else {
            return Ok(Vec::new());
        };
        Ok(view.members.values().copied().collect())
    }

    /// Devices on the identity log. A paired folder can read the list. Only the root can revoke.
    ///
    /// # Errors
    ///
    /// Returns [`CliError`] when the identity log does not project.
    pub fn devices(&self) -> Result<Vec<DeviceLine>, CliError> {
        self.ensure()?;
        let guard = self.cache.borrow();
        Ok(guard
            .as_ref()
            .map(|item| item.projected.devices.clone())
            .unwrap_or_default())
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

    fn ensure(&self) -> Result<(), CliError> {
        let stamp = ambient_store::log_stamp(&self.local.notes_path)
            .map_err(|_| CliError::message("could not open notes file"))?;
        if self
            .cache
            .borrow()
            .as_ref()
            .is_some_and(|item| item.stamp == stamp)
        {
            return Ok(());
        }
        let wires = load(&self.local)?;
        let projected = open_projected(&self.local, &wires)?;
        *self.cache.borrow_mut() = Some(NoteCache {
            stamp,
            wires,
            projected,
        });
        Ok(())
    }
}

fn search_scan(
    view: &ambient_spaces::Projection,
    query: &str,
    names: &std::collections::BTreeMap<[u8; 32], String>,
) -> Vec<NoteSummary> {
    let mut found = Vec::new();
    for (id, heads) in &view.notes {
        if heads_are_deleted(heads) {
            continue;
        }
        if heads
            .iter()
            .any(|head| head.title.contains(query) || head.body.contains(query))
        {
            found.push(summary(id, heads, names));
        }
    }
    found
}

fn heads_are_deleted(heads: &[ambient_spaces::NoteHead]) -> bool {
    !heads.is_empty() && heads.iter().all(|head| note_is_deleted(&head.body))
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
        tags: note_tags(heads),
    }
}

fn short_hex(bytes: &[u8; 32]) -> String {
    hex(bytes).chars().take(8).collect()
}

fn notes_stamp(view: &ambient_spaces::Projection) -> u64 {
    let mut hasher = DefaultHasher::new();
    view.notes.len().hash(&mut hasher);
    for (id, heads) in &view.notes {
        id.hash(&mut hasher);
        heads.len().hash(&mut hasher);
        for head in heads {
            head.title.hash(&mut hasher);
            head.body.hash(&mut hasher);
        }
    }
    hasher.finish()
}

fn note_tags(heads: &[ambient_spaces::NoteHead]) -> Vec<String> {
    let mut tags = BTreeSet::new();
    for head in heads {
        for tag in tags_in(&head.body) {
            tags.insert(tag);
        }
    }
    tags.into_iter().collect()
}

/// Tags in `text`. A heading (`# Title`) is not a tag. Fenced and inline code are skipped.
fn tags_in(text: &str) -> Vec<String> {
    let mut tags = BTreeSet::new();
    let mut fence: Option<bool> = None;
    for line in text.split('\n') {
        if fence_line(line, &mut fence) {
            continue;
        }
        collect_tags(line, &mut tags);
    }
    tags.into_iter().collect()
}

fn fence_line(line: &str, fence: &mut Option<bool>) -> bool {
    let spaces = line.bytes().take_while(|byte| *byte == b' ').count();
    if spaces > 3 {
        return fence.is_some();
    }
    let trimmed_start = line.trim_start();
    let tilde = if trimmed_start.starts_with("~~~") {
        true
    } else if trimmed_start.starts_with("```") {
        false
    } else {
        return fence.is_some();
    };
    let marker = line.trim();
    let want = if tilde { '~' } else { '`' };
    let closer = marker.len() >= 3 && marker.chars().all(|char| char == want);
    match *fence {
        Some(open) if open == tilde && closer => *fence = None,
        None => *fence = Some(tilde),
        Some(_) => {}
    }
    true
}

fn collect_tags(line: &str, tags: &mut BTreeSet<String>) {
    let mut index = 0;
    let mut code = false;
    while index < line.len() {
        let Some(char) = line[index..].chars().next() else {
            break;
        };
        if char == '`' {
            code = !code;
            index = index.saturating_add(char.len_utf8());
            continue;
        }
        if !code && char == '#' {
            if let Some(end) = tag_end_at(line, index) {
                if let Some(name) = line.get(index.saturating_add(1)..end) {
                    tags.insert(name.to_owned());
                }
                index = end;
                continue;
            }
        }
        index = index.saturating_add(char.len_utf8());
    }
}

fn tag_end_at(line: &str, index: usize) -> Option<usize> {
    if index > 0 {
        let prev = line.get(..index)?.chars().next_back()?;
        if prev.is_ascii_alphanumeric() {
            return None;
        }
    }
    let rest = line.get(index..)?;
    if !rest.starts_with('#') || rest.starts_with("##") {
        return None;
    }
    let mut end = index.saturating_add(1);
    let mut saw = false;
    while end < line.len() {
        let char = line.get(end..)?.chars().next()?;
        if char.is_ascii_alphanumeric() || matches!(char, '_' | '-' | '/') {
            saw = true;
            end = end.saturating_add(char.len_utf8());
        } else {
            break;
        }
    }
    while end > index.saturating_add(1) && line.as_bytes().get(end.saturating_sub(1)) == Some(&b'/')
    {
        end = end.saturating_sub(1);
    }
    (saw && end > index.saturating_add(1)).then_some(end)
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
    open_projected(local, wires)?
        .view
        .ok_or_else(|| CliError::message("notes log rejected"))
}

fn open_projected(local: &Local, wires: &[Vec<u8>]) -> Result<Projected, CliError> {
    if wires.is_empty() {
        return Ok(Projected {
            view: None,
            names: BTreeMap::new(),
            devices: Vec::new(),
        });
    }
    let mut identity_wires = Vec::new();
    let mut space_wires = Vec::new();
    for wire in wires {
        let Ok(event) = verify(wire) else {
            continue;
        };
        if event.signer == local.identity_public && event.event_type != "space.member-add" {
            identity_wires.push(wire.as_slice());
            continue;
        }
        if event.signer == local.identity_public {
            identity_wires.push(wire.as_slice());
        }
        space_wires.push(wire.as_slice());
    }
    let log = project_log(&local.identity_public, &identity_wires)
        .map_err(|_| CliError::message("identity log rejected"))?;
    let mut names = BTreeMap::new();
    let mut devices = Vec::new();
    for (key, state) in &log.devices {
        let (label, revoked) = match state {
            DeviceState::Active { label, .. } => (label.clone(), false),
            DeviceState::Revoked { label, .. } => (label.clone(), true),
        };
        names.insert(*key, label.clone());
        devices.push(DeviceLine {
            id: hex(key),
            label,
            revoked,
            current: *key == local.device.signing_key,
        });
    }
    let mut revoked_keys = Vec::new();
    for (key, state) in &log.devices {
        if let DeviceState::Revoked { up_to, .. } = state {
            revoked_keys.push(Revocation {
                signing_key: *key,
                up_to: *up_to,
            });
        }
    }
    let view = project_notes_for_root(&space_wires, &revoked_keys, Some(local.identity_public))
        .map_err(|_| CliError::message("notes log rejected"))?;
    Ok(Projected {
        view: Some(view),
        names,
        devices,
    })
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
