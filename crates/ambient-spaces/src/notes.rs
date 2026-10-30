use std::collections::{BTreeMap, BTreeSet, HashMap};

use ambient_events::{Cbor, EventId, decode_cbor, encode_cbor, verify};
use ambient_identity::is_retained;

use crate::error::SpaceError;

/// A revocation taken from the identity log.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Revocation {
    /// Device signing key.
    pub signing_key: [u8; 32],
    /// Last accepted event, or null if none are accepted.
    pub up_to: Option<EventId>,
}

/// Why an event was not applied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Skip {
    /// The signer already had an event with this `prev`, and this id lost.
    Fork,
    /// The signer's revocation does not retain this event.
    Revoked,
    /// The signer is not a current writer or owner.
    NotMember,
    /// A predecessor from this signer has not arrived.
    Gap,
    /// `msg.put` in a notes Space, or the reverse.
    WrongKind,
}

/// One surviving snapshot of a note. A note with more than one head is a conflict.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoteHead {
    /// Event id of this `note.put`.
    pub event_id: EventId,
    /// Title, which may be empty.
    pub title: String,
    /// Markdown body.
    pub body: String,
}

/// One current member, as the sync client needs them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemberInfo {
    /// X25519 key new events are sealed to.
    pub encryption_key: [u8; 32],
    /// Inbox mailbox.
    pub mailbox: [u8; 32],
    /// 1 owner, 2 writer.
    pub role: u64,
}

/// Applied notes in one Notes space.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Projection {
    /// Event id of `space.genesis`.
    pub space_id: EventId,
    /// Last applied event from each signer. The next event uses this as `prev`.
    pub tip: BTreeMap<[u8; 32], EventId>,
    /// Heads for each note id. More than one head is a conflict. Do not pick a winner.
    pub notes: BTreeMap<[u8; 32], Vec<NoteHead>>,
    /// Current members, keyed by signing key.
    pub members: BTreeMap<[u8; 32], MemberInfo>,
    /// Events that were not applied.
    pub skipped: Vec<(EventId, Skip)>,
}

/// One applied chat message. Event-id order, not arrival order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChatLine {
    /// Event id of this `msg.put`.
    pub event_id: EventId,
    /// Contact signing key that sent it.
    pub sender: [u8; 32],
    /// Message text. A copy, not a grant.
    pub text: String,
}

/// Applied messages in one conversation Space.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Conversation {
    /// Event id of `space.genesis`.
    pub space_id: EventId,
    /// Last applied event from each signer.
    pub tip: BTreeMap<[u8; 32], EventId>,
    /// Current members, keyed by signing key.
    pub members: BTreeMap<[u8; 32], MemberInfo>,
    /// Applied messages.
    pub messages: Vec<ChatLine>,
    /// Events that were not applied.
    pub skipped: Vec<(EventId, Skip)>,
}

struct Item {
    id: EventId,
    prev: Option<EventId>,
    signer: [u8; 32],
    space: Option<EventId>,
    event_type: String,
    payload: Vec<u8>,
}

#[derive(Clone)]
struct Member {
    role: u64,
    encryption_key: [u8; 32],
    mailbox: [u8; 32],
}

/// Project a Notes space.
///
/// Forks keep the lexicographically smaller event id. A revoked signer's events
/// stay only when [`is_retained`] says so. Two `note.put` heads for one note id
/// are both returned.
///
/// # Errors
///
/// Missing genesis, a malformed payload, or a wire that does not verify.
pub fn project_notes(wires: &[&[u8]], revoked: &[Revocation]) -> Result<Projection, SpaceError> {
    let projected = project(wires, revoked, "notes")?;
    Ok(Projection {
        space_id: projected.space_id,
        tip: projected.tip,
        notes: projected.notes,
        members: projected.members,
        skipped: projected.skipped,
    })
}

/// Project a conversation Space. `note.put` is `wrong_kind` and is not applied.
///
/// # Errors
///
/// Missing genesis, a malformed payload, a wire that does not verify, or a notes Space.
pub fn project_conversation(
    wires: &[&[u8]],
    revoked: &[Revocation],
) -> Result<Conversation, SpaceError> {
    let projected = project(wires, revoked, "conversation")?;
    Ok(Conversation {
        space_id: projected.space_id,
        tip: projected.tip,
        members: projected.members,
        messages: projected.messages,
        skipped: projected.skipped,
    })
}

struct Projected {
    space_id: EventId,
    tip: BTreeMap<[u8; 32], EventId>,
    notes: BTreeMap<[u8; 32], Vec<NoteHead>>,
    members: BTreeMap<[u8; 32], MemberInfo>,
    messages: Vec<ChatLine>,
    skipped: Vec<(EventId, Skip)>,
}

fn project(
    wires: &[&[u8]],
    revoked: &[Revocation],
    expected: &str,
) -> Result<Projected, SpaceError> {
    let mut items = Vec::new();
    for wire in wires {
        let event = verify(wire).map_err(SpaceError::Event)?;
        items.push(Item {
            id: event.id,
            prev: event.prev,
            signer: event.signer,
            space: event.space,
            event_type: event.event_type,
            payload: event.payload,
        });
    }
    let (ready, mut skipped) = resolve_forks(&items);
    let revoked_map: BTreeMap<[u8; 32], Option<EventId>> = revoked
        .iter()
        .map(|item| (item.signing_key, item.up_to))
        .collect();
    let prevs = prevs_of(&items);
    let mut kept = Vec::new();
    let mut revoked_ids = BTreeSet::new();
    for item in ready {
        if let Some(up_to) = revoked_map.get(&item.signer) {
            let chain = prevs.get(&item.signer).cloned().unwrap_or_default();
            if !is_retained(*up_to, item.id, &chain) {
                revoked_ids.insert(item.id);
            }
        }
        kept.push(item);
    }
    let mut by_prev: HashMap<([u8; 32], Option<EventId>), Item> = HashMap::new();
    for item in kept {
        by_prev.insert((item.signer, item.prev), item);
    }
    let Some(genesis) = take_genesis(&mut by_prev) else {
        return Err(SpaceError::BadPayload);
    };
    if genesis.event_type != "space.genesis" || genesis.space.is_some() {
        return Err(SpaceError::BadPayload);
    }
    let (kind, mut members) = read_genesis(&genesis.payload)?;
    if kind != expected {
        return Err(SpaceError::WrongKind);
    }
    let Some(founder) = members.get(&genesis.signer) else {
        return Err(SpaceError::BadPayload);
    };
    if founder.role != 1 {
        return Err(SpaceError::BadPayload);
    }
    let space_id = genesis.id;
    if revoked_ids.contains(&space_id) {
        return Err(SpaceError::BadPayload);
    }
    let mut tip = BTreeMap::from([(genesis.signer, genesis.id)]);
    let mut notes_by_id = BTreeMap::new();
    let mut messages = Vec::new();
    apply_ready(
        &mut by_prev,
        &mut members,
        &mut notes_by_id,
        &mut messages,
        &mut tip,
        &mut skipped,
        &revoked_ids,
        space_id,
        expected == "conversation",
    )?;
    for item in by_prev.into_values() {
        skipped.push((item.id, Skip::Gap));
    }
    skipped.sort_by_key(|item| item.0);
    messages.sort_by_key(|item| item.event_id);
    Ok(Projected {
        space_id,
        tip,
        notes: heads(notes_by_id),
        members: publish_members(&members),
        messages,
        skipped,
    })
}

#[allow(clippy::too_many_arguments)] // One walk over the log. Splitting it would hide the order.
fn apply_ready(
    by_prev: &mut HashMap<([u8; 32], Option<EventId>), Item>,
    members: &mut BTreeMap<[u8; 32], Member>,
    notes_by_id: &mut BTreeMap<[u8; 32], Vec<StoredNote>>,
    messages: &mut Vec<ChatLine>,
    tip: &mut BTreeMap<[u8; 32], EventId>,
    skipped: &mut Vec<(EventId, Skip)>,
    revoked_ids: &BTreeSet<EventId>,
    space_id: EventId,
    conversation: bool,
) -> Result<(), SpaceError> {
    loop {
        let mut ready = Vec::new();
        for (key, item) in by_prev.iter() {
            if is_ready(item, tip, space_id) {
                ready.push(*key);
            }
        }
        if ready.is_empty() {
            return Ok(());
        }
        ready.sort_by(|left, right| {
            by_prev
                .get(left)
                .map(|item| item.id)
                .cmp(&by_prev.get(right).map(|item| item.id))
        });
        for key in ready {
            let Some(item) = by_prev.remove(&key) else {
                continue;
            };
            if revoked_ids.contains(&item.id) {
                skipped.push((item.id, Skip::Revoked));
                tip.insert(item.signer, item.id);
                continue;
            }
            match item.event_type.as_str() {
                "note.put" if !conversation => apply_note(notes_by_id, members, &item, skipped)?,
                "msg.put" if conversation => apply_message(messages, members, &item, skipped)?,
                "note.put" | "msg.put" => skipped.push((item.id, Skip::WrongKind)),
                "space.member-add" | "space.member-remove" | "space.inbox" => {
                    apply_membership(members, &item, skipped)?;
                }
                _ => return Err(SpaceError::Event(ambient_events::EventError::BadType)),
            }
            tip.insert(item.signer, item.id);
        }
    }
}

struct StoredNote {
    event_id: EventId,
    title: String,
    body: String,
    parents: Vec<EventId>,
}

fn is_ready(item: &Item, tip: &BTreeMap<[u8; 32], EventId>, space_id: EventId) -> bool {
    if item.space != Some(space_id) {
        return false;
    }
    match item.prev {
        Some(prev) => tip.get(&item.signer).copied() == Some(prev),
        None => !tip.contains_key(&item.signer),
    }
}
fn apply_note(
    notes: &mut BTreeMap<[u8; 32], Vec<StoredNote>>,
    members: &BTreeMap<[u8; 32], Member>,
    item: &Item,
    skipped: &mut Vec<(EventId, Skip)>,
) -> Result<(), SpaceError> {
    let Some(member) = members.get(&item.signer) else {
        skipped.push((item.id, Skip::NotMember));
        return Ok(());
    };
    if member.role != 1 && member.role != 2 {
        skipped.push((item.id, Skip::NotMember));
        return Ok(());
    }
    let (id, title, body, parents) = read_note(&item.payload)?;
    notes.entry(id).or_default().push(StoredNote {
        event_id: item.id,
        title,
        body,
        parents,
    });
    Ok(())
}

fn apply_message(
    messages: &mut Vec<ChatLine>,
    members: &BTreeMap<[u8; 32], Member>,
    item: &Item,
    skipped: &mut Vec<(EventId, Skip)>,
) -> Result<(), SpaceError> {
    let Some(member) = members.get(&item.signer) else {
        skipped.push((item.id, Skip::NotMember));
        return Ok(());
    };
    if member.role != 1 && member.role != 2 {
        skipped.push((item.id, Skip::NotMember));
        return Ok(());
    }
    messages.push(ChatLine {
        event_id: item.id,
        sender: item.signer,
        text: read_message(&item.payload)?,
    });
    Ok(())
}

fn publish_members(members: &BTreeMap<[u8; 32], Member>) -> BTreeMap<[u8; 32], MemberInfo> {
    members
        .iter()
        .map(|(key, member)| {
            (
                *key,
                MemberInfo {
                    encryption_key: member.encryption_key,
                    mailbox: member.mailbox,
                    role: member.role,
                },
            )
        })
        .collect()
}

fn apply_membership(
    members: &mut BTreeMap<[u8; 32], Member>,
    item: &Item,
    skipped: &mut Vec<(EventId, Skip)>,
) -> Result<(), SpaceError> {
    let Some(actor) = members.get(&item.signer) else {
        skipped.push((item.id, Skip::NotMember));
        return Ok(());
    };
    if actor.role != 1 {
        skipped.push((item.id, Skip::NotMember));
        return Ok(());
    }
    match item.event_type.as_str() {
        "space.member-add" => {
            let payload = decode_cbor(&item.payload).map_err(|_| SpaceError::BadPayload)?;
            let (key, member) = read_member(&payload)?;
            members.insert(key, member);
        }
        "space.member-remove" => {
            let key = read_remove(&item.payload)?;
            members.remove(&key);
        }
        "space.inbox" => {
            let (key, mailbox) = read_inbox(&item.payload)?;
            if let Some(member) = members.get_mut(&key) {
                member.mailbox = mailbox;
            }
        }
        _ => return Err(SpaceError::BadPayload),
    }
    Ok(())
}

fn heads(notes: BTreeMap<[u8; 32], Vec<StoredNote>>) -> BTreeMap<[u8; 32], Vec<NoteHead>> {
    notes
        .into_iter()
        .map(|(id, versions)| {
            let mut covered = BTreeSet::new();
            for version in &versions {
                mark_ancestors(version.event_id, &versions, &mut covered);
            }
            let mut heads = Vec::new();
            for version in versions {
                if covered.contains(&version.event_id) {
                    continue;
                }
                heads.push(NoteHead {
                    event_id: version.event_id,
                    title: version.title,
                    body: version.body,
                });
            }
            heads.sort_by_key(|head| head.event_id);
            (id, heads)
        })
        .collect()
}

fn mark_ancestors(id: EventId, versions: &[StoredNote], covered: &mut BTreeSet<EventId>) {
    let Some(version) = versions.iter().find(|item| item.event_id == id) else {
        return;
    };
    for parent in &version.parents {
        if covered.insert(*parent) {
            mark_ancestors(*parent, versions, covered);
        }
    }
}

fn resolve_forks(items: &[Item]) -> (Vec<Item>, Vec<(EventId, Skip)>) {
    let mut grouped: HashMap<([u8; 32], Option<EventId>), Vec<Item>> = HashMap::new();
    for item in items {
        grouped
            .entry((item.signer, item.prev))
            .or_default()
            .push(Item {
                id: item.id,
                prev: item.prev,
                signer: item.signer,
                space: item.space,
                event_type: item.event_type.clone(),
                payload: item.payload.clone(),
            });
    }
    let mut ready = Vec::new();
    let mut skipped = Vec::new();
    for mut group in grouped.into_values() {
        group.sort_by_key(|item| item.id);
        let mut group = group.into_iter();
        if let Some(winner) = group.next() {
            ready.push(winner);
        }
        skipped.extend(group.map(|item| (item.id, Skip::Fork)));
    }
    (ready, skipped)
}

fn take_genesis(by_prev: &mut HashMap<([u8; 32], Option<EventId>), Item>) -> Option<Item> {
    let key = by_prev
        .iter()
        .find(|(_, item)| item.event_type == "space.genesis" && item.prev.is_none())
        .map(|(key, _)| *key)?;
    by_prev.remove(&key)
}

fn prevs_of(items: &[Item]) -> BTreeMap<[u8; 32], BTreeMap<EventId, Option<EventId>>> {
    let mut out = BTreeMap::<[u8; 32], BTreeMap<EventId, Option<EventId>>>::new();
    for item in items {
        out.entry(item.signer)
            .or_default()
            .insert(item.id, item.prev);
    }
    out
}

/// One member written into `space.genesis`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenesisMember {
    /// Ed25519 signing key.
    pub signing_key: [u8; 32],
    /// X25519 encryption key.
    pub encryption_key: [u8; 32],
    /// 1 owner, 2 writer.
    pub role: u64,
    /// Inbox mailbox.
    pub mailbox: [u8; 32],
}

/// Payload for `space.genesis` of a notes Space.
///
/// # Errors
///
/// Returns [`SpaceError::BadPayload`] when a member role is not 1 or 2.
pub fn notes_genesis_payload(members: &[GenesisMember]) -> Result<Vec<u8>, SpaceError> {
    genesis_payload("notes", members)
}

/// Payload for `space.genesis` of a conversation Space.
///
/// # Errors
///
/// Returns [`SpaceError::BadPayload`] when a member role is not 1 or 2.
pub fn conversation_genesis_payload(members: &[GenesisMember]) -> Result<Vec<u8>, SpaceError> {
    genesis_payload("conversation", members)
}

/// Payload for `space.member-add`.
///
/// # Errors
///
/// Returns [`SpaceError::BadPayload`] when the role is not 1 or 2.
pub fn member_add_payload(member: &GenesisMember) -> Result<Vec<u8>, SpaceError> {
    if member.role != 1 && member.role != 2 {
        return Err(SpaceError::BadPayload);
    }
    encode_cbor(&member_cbor(member)).map_err(SpaceError::Event)
}

/// Payload for `msg.put`. Fields 2 and 3 are null: no reply, no citation.
///
/// # Errors
///
/// Returns [`SpaceError::BadPayload`] when the text is longer than 16000 bytes.
pub fn msg_put_payload(text: &str) -> Result<Vec<u8>, SpaceError> {
    if text.len() > 16000 {
        return Err(SpaceError::BadPayload);
    }
    encode_cbor(&Cbor::Map(vec![
        (Cbor::Uint(1), Cbor::Text(text.to_owned())),
        (Cbor::Uint(2), Cbor::Null),
        (Cbor::Uint(3), Cbor::Null),
    ]))
    .map_err(SpaceError::Event)
}

fn genesis_payload(kind: &str, members: &[GenesisMember]) -> Result<Vec<u8>, SpaceError> {
    let mut encoded = Vec::new();
    for member in members {
        if member.role != 1 && member.role != 2 {
            return Err(SpaceError::BadPayload);
        }
        encoded.push(member_cbor(member));
    }
    encode_cbor(&Cbor::Map(vec![
        (Cbor::Uint(1), Cbor::Text(kind.to_owned())),
        (Cbor::Uint(2), Cbor::Array(encoded)),
    ]))
    .map_err(SpaceError::Event)
}

fn member_cbor(member: &GenesisMember) -> Cbor {
    Cbor::Map(vec![
        (Cbor::Uint(1), Cbor::Bytes(member.signing_key.to_vec())),
        (Cbor::Uint(2), Cbor::Bytes(member.encryption_key.to_vec())),
        (Cbor::Uint(3), Cbor::Uint(member.role)),
        (Cbor::Uint(4), Cbor::Bytes(member.mailbox.to_vec())),
    ])
}

/// Payload for `note.put`.
///
/// # Errors
///
/// Returns [`SpaceError::BadPayload`] when the title is longer than 200 bytes.
pub fn note_put_payload(
    note_id: &[u8; 32],
    title: &str,
    body: &str,
    parents: &[EventId],
) -> Result<Vec<u8>, SpaceError> {
    if title.len() > 200 {
        return Err(SpaceError::BadPayload);
    }
    let parents = parents
        .iter()
        .map(|id| Cbor::Bytes(id.0.to_vec()))
        .collect();
    encode_cbor(&Cbor::Map(vec![
        (Cbor::Uint(1), Cbor::Bytes(note_id.to_vec())),
        (Cbor::Uint(2), Cbor::Text(title.to_owned())),
        (Cbor::Uint(3), Cbor::Text("text-snapshot-v1".to_owned())),
        (Cbor::Uint(4), Cbor::Bytes(body.as_bytes().to_vec())),
        (Cbor::Uint(5), Cbor::Array(parents)),
    ]))
    .map_err(SpaceError::Event)
}

fn read_genesis(payload: &[u8]) -> Result<(String, BTreeMap<[u8; 32], Member>), SpaceError> {
    let value = decode_cbor(payload).map_err(|_| SpaceError::BadPayload)?;
    let map = uint_map(&value)?;
    if map.len() != 2 {
        return Err(SpaceError::BadPayload);
    }
    let Cbor::Text(kind) = map.get(&1).ok_or(SpaceError::BadPayload)? else {
        return Err(SpaceError::BadPayload);
    };
    let Cbor::Array(members) = map.get(&2).ok_or(SpaceError::BadPayload)? else {
        return Err(SpaceError::BadPayload);
    };
    if members.is_empty() {
        return Err(SpaceError::BadPayload);
    }
    let mut out = BTreeMap::new();
    for member in members {
        let (key, parsed) = read_member(member)?;
        out.insert(key, parsed);
    }
    Ok((kind.clone(), out))
}

fn read_member(value: &Cbor) -> Result<([u8; 32], Member), SpaceError> {
    let map = uint_map(value)?;
    if map.len() != 4 {
        return Err(SpaceError::BadPayload);
    }
    let role = match map.get(&3) {
        Some(Cbor::Uint(role)) if *role == 1 || *role == 2 => *role,
        _ => return Err(SpaceError::BadPayload),
    };
    let encryption = bytes32(map.get(&2).ok_or(SpaceError::BadPayload)?)?;
    Ok((
        bytes32(map.get(&1).ok_or(SpaceError::BadPayload)?)?,
        Member {
            role,
            encryption_key: encryption,
            mailbox: bytes32(map.get(&4).ok_or(SpaceError::BadPayload)?)?,
        },
    ))
}

fn read_message(payload: &[u8]) -> Result<String, SpaceError> {
    let map = uint_map(&decode_owned(payload)?)?;
    if map.len() != 3 {
        return Err(SpaceError::BadPayload);
    }
    let text = match map.get(&1) {
        Some(Cbor::Text(text)) if text.len() <= 16000 => text.clone(),
        _ => return Err(SpaceError::BadPayload),
    };
    match map.get(&2) {
        Some(Cbor::Null | Cbor::Bytes(_)) => {}
        _ => return Err(SpaceError::BadPayload),
    }
    match map.get(&3) {
        Some(Cbor::Null | Cbor::Bytes(_)) => {}
        _ => return Err(SpaceError::BadPayload),
    }
    Ok(text)
}

fn read_note(payload: &[u8]) -> Result<([u8; 32], String, String, Vec<EventId>), SpaceError> {
    let map = uint_map(&decode_owned(payload)?)?;
    if map.len() != 5 {
        return Err(SpaceError::BadPayload);
    }
    let title = match map.get(&2) {
        Some(Cbor::Text(text)) if text.len() <= 200 => text.clone(),
        _ => return Err(SpaceError::BadPayload),
    };
    let Cbor::Text(codec) = map.get(&3).ok_or(SpaceError::BadPayload)? else {
        return Err(SpaceError::BadPayload);
    };
    if codec != "text-snapshot-v1" {
        return Err(SpaceError::BadPayload);
    }
    let Cbor::Bytes(body) = map.get(&4).ok_or(SpaceError::BadPayload)? else {
        return Err(SpaceError::BadPayload);
    };
    let body = String::from_utf8(body.clone()).map_err(|_| SpaceError::BadPayload)?;
    let Cbor::Array(parents) = map.get(&5).ok_or(SpaceError::BadPayload)? else {
        return Err(SpaceError::BadPayload);
    };
    let mut ids = Vec::new();
    for parent in parents {
        ids.push(EventId(bytes32(parent)?));
    }
    Ok((
        bytes32(map.get(&1).ok_or(SpaceError::BadPayload)?)?,
        title,
        body,
        ids,
    ))
}

fn read_remove(payload: &[u8]) -> Result<[u8; 32], SpaceError> {
    let map = uint_map(&decode_owned(payload)?)?;
    if map.len() != 1 {
        return Err(SpaceError::BadPayload);
    }
    bytes32(map.get(&1).ok_or(SpaceError::BadPayload)?)
}

fn read_inbox(payload: &[u8]) -> Result<([u8; 32], [u8; 32]), SpaceError> {
    let map = uint_map(&decode_owned(payload)?)?;
    if map.len() != 2 {
        return Err(SpaceError::BadPayload);
    }
    Ok((
        bytes32(map.get(&1).ok_or(SpaceError::BadPayload)?)?,
        bytes32(map.get(&2).ok_or(SpaceError::BadPayload)?)?,
    ))
}

fn decode_owned(payload: &[u8]) -> Result<Cbor, SpaceError> {
    decode_cbor(payload).map_err(|_| SpaceError::BadPayload)
}

fn uint_map(value: &Cbor) -> Result<BTreeMap<u64, Cbor>, SpaceError> {
    let Cbor::Map(pairs) = value else {
        return Err(SpaceError::BadPayload);
    };
    let mut map = BTreeMap::new();
    for (key, val) in pairs {
        let Cbor::Uint(index) = key else {
            return Err(SpaceError::BadPayload);
        };
        if map.insert(*index, val.clone()).is_some() {
            return Err(SpaceError::BadPayload);
        }
    }
    Ok(map)
}

fn bytes32(value: &Cbor) -> Result<[u8; 32], SpaceError> {
    let Cbor::Bytes(bytes) = value else {
        return Err(SpaceError::BadPayload);
    };
    <[u8; 32]>::try_from(bytes.as_slice()).map_err(|_| SpaceError::BadPayload)
}
