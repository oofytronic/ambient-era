//! The notebook both the CLI and the desktop window use.

use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError, Weak};

use ambient_events::{EventId, VerifiedEvent, encode_unsigned, sign_wire, verify};
use ambient_identity::{
    DeviceState, decode_pair_offer_text, device_add_payload, project_log, project_log_verified,
    revoke_payload,
};
use ambient_spaces::{
    GenesisMember, Revocation, member_add_payload, member_remove_payload, note_put_payload,
    notes_genesis_payload, project_notes_verified,
};
use ambient_store::{StoreError, read_log, write_log, write_log_if};

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
    /// Tags from the note's tag line. A view, not a separate record.
    /// A `#name` in the writing is not a tag.
    pub tags: Vec<String>,
    /// Event id of the first head. This is the current version, not a grant.
    pub version: String,
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
    /// Event id of this snapshot. A link can name this version.
    pub version: String,
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

/// One stored snapshot of a note. `current` means it is still a head.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoteSnap {
    /// Title of that snapshot.
    pub title: String,
    /// Markdown body.
    pub body: String,
    /// True when this event is still a current head.
    pub current: bool,
}

/// An open notebook directory. The same files the CLI reads.
pub struct Notebook {
    local: Local,
    /// Last projection. Reused until [`log_stamp`] changes, so a click does not verify again.
    ///
    /// Shared, not copied: another [`Notebook`] opened on the same folder adopts it while the
    /// file is unchanged, so a save does not start from a cold load. See [`SHARED`].
    cache: RefCell<Option<Arc<NoteCache>>>,
}

#[derive(Clone)]
struct NoteCache {
    stamp: Option<(u64, u64)>,
    wires: Vec<Vec<u8>>,
    /// Every wire that verified, in file order. A signature is checked once, when its wire is
    /// loaded or written. Every projection after that starts from these.
    events: Vec<VerifiedEvent>,
    projected: Projected,
}

#[derive(Clone)]
struct Projected {
    view: Option<ambient_spaces::Projection>,
    names: BTreeMap<[u8; 32], String>,
    devices: Vec<DeviceLine>,
    /// `note.put` event id to its index in the loaded wires.
    /// Built during the verify that loads the log, so opening a version does not scan.
    snaps: HashMap<[u8; 32], usize>,
}

/// How many times a save runs again after another writer got in first. Two writers
/// colliding is rare, and a third in a row almost never happens, so a small number is plenty.
const APPEND_TRIES: usize = 8;

/// Which notebook a cache belongs to: the notes file, the identity, and this device.
type ShareKey = (PathBuf, [u8; 32], [u8; 32]);

/// The newest cache for each folder open in this process.
///
/// A [`Notebook`] is opened for every save and every sync, and each used to start with a full
/// load that checks every signature. A fresh one now adopts the cache a live notebook already
/// holds, provided the file still has the stamp that cache was built from. The entries are weak,
/// so nothing stays in memory once the last notebook that holds one is gone.
static SHARED: Mutex<Vec<(ShareKey, Weak<NoteCache>)>> = Mutex::new(Vec::new());

fn shared_get(key: &ShareKey, stamp: Option<(u64, u64)>) -> Option<Arc<NoteCache>> {
    let list = SHARED.lock().unwrap_or_else(PoisonError::into_inner);
    list.iter()
        .find(|(held, _)| held == key)
        .and_then(|(_, cache)| cache.upgrade())
        .filter(|cache| cache.stamp == stamp)
}

fn shared_put(key: ShareKey, cache: &Arc<NoteCache>) {
    let mut list = SHARED.lock().unwrap_or_else(PoisonError::into_inner);
    list.retain(|(held, weak)| held != &key && weak.strong_count() > 0);
    list.push((key, Arc::downgrade(cache)));
}

impl Notebook {
    /// Create a new notebook. Fails if `dir/key` already exists.
    ///
    /// # Errors
    ///
    /// Returns [`CliError`] when the directory or the first events cannot be written.
    pub fn create(dir: &Path) -> Result<Self, CliError> {
        Self::create_with_label(dir, "this-computer")
    }

    /// Create a notebook whose device label is `label` (1 to 32 bytes).
    ///
    /// The label is stored in the key file and on this device's record.
    /// It is not a username and it is not sent as one.
    ///
    /// # Errors
    ///
    /// Returns [`CliError`] when the directory, the label, or the first events cannot be written.
    pub fn create_with_label(dir: &Path, label: &str) -> Result<Self, CliError> {
        let local = keys::create(dir, label)?;
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
        self.list_held()
    }

    /// Notes already in memory. Does not stat the log.
    ///
    /// The window uses this on a click. A background write hands over a new
    /// [`Notebook`] when the file has changed, so the caret never re-reads it.
    ///
    /// # Errors
    ///
    /// Returns [`CliError`] when this notebook has not been opened.
    pub fn list_held(&self) -> Result<Vec<NoteSummary>, CliError> {
        let guard = self.cache.borrow();
        let Some(item) = guard.as_ref() else {
            return Err(CliError::message("folder is not open"));
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
    pub fn visit_notes(&self, visit: impl FnMut(&str, &str, &str, bool)) -> Result<(), CliError> {
        self.ensure()?;
        self.visit_held(visit)
    }

    /// Walk the notes already in memory. Does not stat the log.
    ///
    /// The closure must not call back into this notebook.
    ///
    /// # Errors
    ///
    /// Returns [`CliError`] when this notebook has not been opened.
    pub fn visit_held(
        &self,
        mut visit: impl FnMut(&str, &str, &str, bool),
    ) -> Result<(), CliError> {
        let guard = self.cache.borrow();
        let Some(item) = guard.as_ref() else {
            return Err(CliError::message("folder is not open"));
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
        self.ensure()?;
        self.show_held(id_hex)
    }

    /// Open one note from memory. Does not stat the log.
    ///
    /// # Errors
    ///
    /// Returns [`CliError`] when the id is not 64 hex characters, the note is
    /// absent, or this notebook has not been opened.
    pub fn show_held(&self, id_hex: &str) -> Result<ShownNote, CliError> {
        let note_id = parse_hex32(id_hex)?;
        let guard = self.cache.borrow();
        let Some(item) = guard.as_ref() else {
            return Err(CliError::message("folder is not open"));
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
                    version: head.event_id.to_string(),
                })
                .collect(),
        })
    }

    /// Read the log into memory if it changed.
    ///
    /// A later [`snapshot_held`](Self::snapshot_held) does not read the file.
    ///
    /// # Errors
    ///
    /// Returns [`CliError`] when the notes file cannot be opened.
    pub fn warm(&self) -> Result<(), CliError> {
        self.ensure()
    }

    /// One snapshot of a note, from memory.
    ///
    /// Current heads are checked first. An older version is the one wire recorded
    /// when the log was loaded. That load already verified every event.
    ///
    /// # Errors
    ///
    /// Returns [`CliError`] when either id is not 64 hex characters, or this
    /// notebook has not been opened.
    pub fn snapshot_held(&self, id_hex: &str, version: &str) -> Result<Option<NoteSnap>, CliError> {
        let note_id = parse_hex32(id_hex)?;
        let version_id = parse_hex32(version)?;
        let guard = self.cache.borrow();
        let Some(item) = guard.as_ref() else {
            return Err(CliError::message("folder is not open"));
        };
        if let Some(view) = item.projected.view.as_ref()
            && let Some(heads) = view.notes.get(&note_id)
            && let Some(head) = heads.iter().find(|head| head.event_id.0 == version_id)
        {
            return Ok(Some(NoteSnap {
                title: head.title.clone(),
                body: head.body.clone(),
                current: true,
            }));
        }
        let Some(&index) = item.projected.snaps.get(&version_id) else {
            return Ok(None);
        };
        let Some(wire) = item.wires.get(index) else {
            return Ok(None);
        };
        let Ok(event) = verify(wire) else {
            return Ok(None);
        };
        let Ok((id, title, body, _)) = ambient_spaces::note_put_parts(&event.payload) else {
            return Ok(None);
        };
        if id != note_id {
            return Ok(None);
        }
        Ok(Some(NoteSnap {
            title,
            body,
            current: false,
        }))
    }

    /// Append a new note. Returns its hex id.
    ///
    /// # Errors
    ///
    /// Returns [`CliError`] when the title is too long or the log cannot be written.
    pub fn add(&self, title: &str, body: &str) -> Result<String, CliError> {
        let note_id = random_note()?;
        let payload = note_put_payload(&note_id, title, body, &[])
            .map_err(|_| CliError::message("title must be at most 200 bytes"))?;
        self.append(|held| {
            if held.wires.is_empty() {
                return Err(CliError::message("sync this device before writing a note"));
            }
            let view = held
                .projected
                .view
                .as_ref()
                .ok_or_else(|| CliError::message("notes log rejected"))?;
            let prev = view.tip.get(&self.local.device.signing_key).copied();
            sign_device(&self.local, prev, Some(view.space_id), "note.put", &payload)
        })?;
        Ok(hex(&note_id))
    }

    /// Append a snapshot. Parents are every current head, so this resolves a conflict.
    ///
    /// # Errors
    ///
    /// Returns [`CliError`] when the note does not exist or the title is too long.
    pub fn edit(&self, id_hex: &str, title: &str, body: &str) -> Result<(), CliError> {
        let note_id = parse_hex32(id_hex)?;
        self.append(|held| {
            let view = held
                .projected
                .view
                .as_ref()
                .ok_or_else(|| CliError::message("no such note"))?;
            let heads = view
                .notes
                .get(&note_id)
                .ok_or_else(|| CliError::message("no such note"))?;
            let parents: Vec<EventId> = heads.iter().map(|head| head.event_id).collect();
            let payload = note_put_payload(&note_id, title, body, &parents)
                .map_err(|_| CliError::message("title must be at most 200 bytes"))?;
            let prev = view.tip.get(&self.local.device.signing_key).copied();
            sign_device(&self.local, prev, Some(view.space_id), "note.put", &payload)
        })
    }

    /// Sign one event against the notes as they are now, and append it.
    ///
    /// `build` sees the loaded notes and returns the signed wire. The write only goes through if
    /// the file is still the one those notes came from. If another writer got there first the
    /// notes are loaded again and `build` runs once more, so the new event links to what is
    /// really there and nothing that writer appended is lost. The loaded notes are updated in
    /// place, so the next read does not load or verify the log again.
    fn append(
        &self,
        build: impl Fn(&NoteCache) -> Result<Vec<u8>, CliError>,
    ) -> Result<(), CliError> {
        for _ in 0..APPEND_TRIES {
            self.ensure()?;
            let Some(held) = self.cache.borrow_mut().take() else {
                return Err(CliError::message("could not open notes file"));
            };
            let wire = match build(&held) {
                Ok(wire) => wire,
                Err(err) => {
                    *self.cache.borrow_mut() = Some(held);
                    return Err(err);
                }
            };
            // Unique when no other notebook holds this cache, and then nothing is copied.
            let mut next = Arc::try_unwrap(held).unwrap_or_else(|shared| (*shared).clone());
            next.wires.push(wire);
            match write_log_if(
                &self.local.notes_path,
                &self.local.store_key,
                next.stamp,
                &next.wires,
            ) {
                Ok(stamp) => {
                    next.stamp = Some(stamp);
                    self.install_appended(next);
                    return Ok(());
                }
                Err(StoreError::Stale) => {
                    // The cache was consumed. The next pass loads what the other writer wrote.
                }
                Err(_) => {
                    next.wires.pop();
                    *self.cache.borrow_mut() = Some(Arc::new(next));
                    return Err(CliError::message("could not write notes file"));
                }
            }
        }
        Err(CliError::message(
            "the notes changed while saving. Try again",
        ))
    }

    /// Fold the wire just written into the loaded notes. The event is this device's own, so
    /// it is checked once and the projection is rebuilt from the events already held. If
    /// anything about that fails the cache is dropped and the next read loads from the file.
    fn install_appended(&self, mut next: NoteCache) {
        let index = next.wires.len().saturating_sub(1);
        let Some(event) = next.wires.get(index).and_then(|wire| verify(wire).ok()) else {
            return;
        };
        let mut snaps = std::mem::take(&mut next.projected.snaps);
        if event.event_type == "note.put" {
            snaps.insert(event.id.0, index);
        }
        next.events.push(event);
        let Ok(projected) = project_events(&self.local, &next.events, snaps) else {
            return;
        };
        next.projected = projected;
        let item = Arc::new(next);
        shared_put(self.share_key(), &item);
        *self.cache.borrow_mut() = Some(item);
    }

    fn share_key(&self) -> ShareKey {
        (
            self.local.notes_path.clone(),
            self.local.identity_public,
            self.local.device.signing_key,
        )
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
        self.devices_held()
    }

    /// Devices already projected. Does not stat the log.
    ///
    /// # Errors
    ///
    /// Returns [`CliError`] when this notebook has not been opened.
    pub fn devices_held(&self) -> Result<Vec<DeviceLine>, CliError> {
        let guard = self.cache.borrow();
        let Some(item) = guard.as_ref() else {
            return Err(CliError::message("folder is not open"));
        };
        Ok(item.projected.devices.clone())
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
        let key = self.share_key();
        if let Some(shared) = shared_get(&key, stamp) {
            *self.cache.borrow_mut() = Some(shared);
            return Ok(());
        }
        let wires = load(&self.local)?;
        let (events, projected) = open_projected(&self.local, &wires)?;
        let item = Arc::new(NoteCache {
            stamp,
            wires,
            events,
            projected,
        });
        shared_put(key, &item);
        *self.cache.borrow_mut() = Some(item);
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
        version: heads
            .first()
            .map(|head| head.event_id.to_string())
            .unwrap_or_default(),
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
        for tag in split_note_tags(&head.body).1 {
            tags.insert(tag);
        }
    }
    tags.into_iter().collect()
}

/// Split the hidden tag line off a note body.
///
/// The line is `%%ambient-tags: clinic, follow-up%%` and only at the end.
/// Anything else, including a `#name` in the writing, stays in the prose.
#[must_use]
pub fn split_note_tags(body: &str) -> (String, Vec<String>) {
    let trimmed_end = body.trim_end_matches(['\n', '\r']);
    let (head, line) = match trimmed_end.rsplit_once('\n') {
        Some((head, line)) => (Some(head), line.trim()),
        None => (None, trimmed_end.trim()),
    };
    let Some(tags) = parse_tag_line(line) else {
        return (body.to_owned(), Vec::new());
    };
    (head.unwrap_or("").to_owned(), tags)
}

/// Put `tags` back on the end of `prose`. An empty list leaves the prose alone.
#[must_use]
pub fn join_note_tags(prose: &str, tags: &[String]) -> String {
    let (prose, _) = split_note_tags(prose);
    let prose = prose.trim_end_matches(['\n', '\r']);
    if tags.is_empty() {
        return prose.to_owned();
    }
    let line = format!("%%ambient-tags: {}%%", tags.join(", "));
    if prose.is_empty() {
        line
    } else {
        format!("{prose}\n{line}")
    }
}

fn parse_tag_line(line: &str) -> Option<Vec<String>> {
    let rest = line.strip_prefix("%%ambient-tags:")?;
    let rest = rest.strip_suffix("%%")?.trim();
    let mut tags = Vec::new();
    if rest.is_empty() {
        return Some(tags);
    }
    for part in rest.split(',') {
        let tag = part.trim().trim_start_matches('#').trim();
        if tag.is_empty() || tag.len() > 40 || tag.contains(['%', ',', '\n', '|']) {
            continue;
        }
        if tags
            .iter()
            .any(|have: &String| have.eq_ignore_ascii_case(tag))
        {
            continue;
        }
        tags.push(tag.to_owned());
    }
    Some(tags)
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
        .1
        .view
        .ok_or_else(|| CliError::message("notes log rejected"))
}

/// Check every signature once, then project. Returns the verified events so later projections
/// do not check them again. A wire that does not verify is skipped, as it always was.
fn open_projected(
    local: &Local,
    wires: &[Vec<u8>],
) -> Result<(Vec<VerifiedEvent>, Projected), CliError> {
    if wires.is_empty() {
        return Ok((
            Vec::new(),
            Projected {
                view: None,
                names: BTreeMap::new(),
                devices: Vec::new(),
                snaps: HashMap::new(),
            },
        ));
    }
    let mut events = Vec::with_capacity(wires.len());
    let mut snaps = HashMap::new();
    for (index, wire) in wires.iter().enumerate() {
        let Ok(event) = verify(wire) else {
            continue;
        };
        if event.event_type == "note.put" {
            snaps.insert(event.id.0, index);
        }
        events.push(event);
    }
    let projected = project_events(local, &events, snaps)?;
    Ok((events, projected))
}

/// Project verified events. No signature is checked here.
fn project_events(
    local: &Local,
    events: &[VerifiedEvent],
    snaps: HashMap<[u8; 32], usize>,
) -> Result<Projected, CliError> {
    let mut identity_events: Vec<&VerifiedEvent> = Vec::new();
    let mut space_events: Vec<&VerifiedEvent> = Vec::new();
    for event in events {
        if event.signer == local.identity_public {
            identity_events.push(event);
            if event.event_type != "space.member-add" {
                continue;
            }
        }
        space_events.push(event);
    }
    let log = project_log_verified(&local.identity_public, &identity_events)
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
    let view = project_notes_verified(&space_events, &revoked_keys, Some(local.identity_public))
        .map_err(|_| CliError::message("notes log rejected"))?;
    Ok(Projected {
        view: Some(view),
        names,
        devices,
        snaps,
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{Notebook, open_projected};

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ambient-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn an_older_snapshot_stays_in_the_log() {
        let dir = scratch("snap");
        let book = Notebook::create(&dir).expect("create");
        let id = book.add("Market", "buy milk").expect("add");
        let first = book.show(&id).expect("show");
        let version = first.heads[0].version.clone();
        book.edit(&id, "Market", "buy oats").expect("edit");
        let now = book.show(&id).expect("now");
        let older = book
            .snapshot_held(&id, &version)
            .expect("scan")
            .expect("found");
        assert!(!older.current);
        assert_eq!(older.body, "buy milk");
        let current = book
            .snapshot_held(&id, &now.heads[0].version)
            .expect("current scan")
            .expect("current");
        assert!(current.current);
        assert_eq!(current.body, "buy oats");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_write_updates_the_held_notes_without_reloading() {
        let dir = scratch("held");
        let book = Notebook::create(&dir).expect("create");
        let id = book.add("Plan", "first").expect("add");
        // `list_held` and `show_held` never look at the file. They must already see the write.
        assert!(book.list_held().expect("held").iter().any(|n| n.id == id));
        assert_eq!(book.show_held(&id).expect("show").heads[0].body, "first");
        let version = book.show_held(&id).expect("show").heads[0].version.clone();
        book.edit(&id, "Plan", "second").expect("edit");
        assert_eq!(book.show_held(&id).expect("show").heads[0].body, "second");
        // The older snapshot is reachable from the index that the write extended.
        let older = book
            .snapshot_held(&id, &version)
            .expect("lookup")
            .expect("found");
        assert_eq!(older.body, "first");
        assert!(!older.current);
        // And the file agrees.
        let fresh = Notebook::open(&dir).expect("reopen");
        assert_eq!(fresh.show_held(&id).expect("show").heads[0].body, "second");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_incremental_projection_matches_a_full_reload() {
        let dir = scratch("same");
        let book = Notebook::create(&dir).expect("create");
        let mut ids = Vec::new();
        for n in 0..6 {
            ids.push(
                book.add(&format!("n{n}"), &format!("body {n}"))
                    .expect("add"),
            );
        }
        book.edit(&ids[1], "n1", "changed").expect("edit");
        book.edit(&ids[4], "four", "changed too").expect("edit");
        let held = book.cache.borrow().clone().expect("cache");
        let (events, from_disk) = open_projected(&book.local, &held.wires).expect("project");
        assert_eq!(held.events, events);
        assert_eq!(held.projected.view, from_disk.view);
        assert_eq!(held.projected.names, from_disk.names);
        assert_eq!(held.projected.devices, from_disk.devices);
        assert_eq!(held.projected.snaps, from_disk.snaps);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_new_notebook_adopts_the_warm_cache() {
        let dir = scratch("share");
        let book = Notebook::create(&dir).expect("create");
        book.add("a", "one").expect("add");
        let other = Notebook::open(&dir).expect("open");
        let (left, right) = (
            book.cache.borrow().clone().expect("cache"),
            other.cache.borrow().clone().expect("cache"),
        );
        assert!(Arc::ptr_eq(&left, &right), "the second open loaded again");
        // A write by one is picked up by the next open, still without a reload.
        other.add("b", "two").expect("add");
        let third = Notebook::open(&dir).expect("open");
        let (written, adopted) = (
            other.cache.borrow().clone().expect("cache"),
            third.cache.borrow().clone().expect("cache"),
        );
        assert!(
            Arc::ptr_eq(&written, &adopted),
            "the third open loaded again"
        );
        assert_eq!(third.list_held().expect("list").len(), 2);
        // The first notebook notices the file moved and catches up.
        assert_eq!(book.list().expect("list").len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn nothing_is_kept_once_the_last_notebook_is_gone() {
        let dir = scratch("weak");
        let weak = {
            let book = Notebook::create(&dir).expect("create");
            book.add("a", "secret").expect("add");
            Arc::downgrade(&book.cache.borrow().clone().expect("cache"))
        };
        assert!(weak.upgrade().is_none(), "the cache outlived its notebook");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn writers_in_separate_notebooks_lose_nothing() {
        let dir = scratch("race");
        drop(Notebook::create(&dir).expect("create"));
        let workers: Vec<_> = (0..3)
            .map(|worker| {
                let dir = dir.clone();
                std::thread::spawn(move || {
                    let book = Notebook::open(&dir).expect("open");
                    for round in 0..4 {
                        book.add(&format!("w{worker}-{round}"), "text")
                            .expect("add");
                    }
                })
            })
            .collect();
        for worker in workers {
            worker.join().expect("worker");
        }
        let book = Notebook::open(&dir).expect("open");
        let mut titles: Vec<String> = book
            .list_held()
            .expect("list")
            .into_iter()
            .map(|note| note.title)
            .collect();
        titles.sort();
        let mut want: Vec<String> = (0..3)
            .flat_map(|w| (0..4).map(move |r| format!("w{w}-{r}")))
            .collect();
        want.sort();
        assert_eq!(titles, want);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
