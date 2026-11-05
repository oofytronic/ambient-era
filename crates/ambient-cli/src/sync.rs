//! Push and pull sealed events through a relay.
//!
//! The notes window calls the wrappers in the crate root. It does not seal here.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use ambient_events::{encode_unsigned, sign_wire, verify};
use ambient_identity::{
    decode_pair_offer_text, device_from_seeds, encode_pair_offer, pair_offer_text,
};
use ambient_spaces::{
    ChatLine, GenesisMember, conversation_genesis_payload, msg_put_payload, open_blob,
    project_conversation, seal_blob,
};
use ambient_store::{read_log, write_log};
use ambient_transport::{Request, Response, decode_response, encode_request};
use tungstenite::Message;

use crate::keys::{self, Local};
use crate::notes::CliError;
use crate::session::Notebook;

pub(crate) fn start_loopback() -> Result<(), CliError> {
    let listener = std::net::TcpListener::bind("127.0.0.1:8787")
        .map_err(|_| CliError::message("relay already running on 127.0.0.1:8787"))?;
    let generation = RELAY_GEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    RELAY_OURS.store(true, std::sync::atomic::Ordering::SeqCst);
    std::thread::spawn(move || {
        let _ = ambient_relay::serve_until(&listener, &RELAY_GEN, generation);
        if RELAY_GEN.load(std::sync::atomic::Ordering::SeqCst) == generation {
            RELAY_OURS.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    });
    Ok(())
}

pub(crate) fn stop_loopback() -> Result<(), CliError> {
    if !RELAY_OURS.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(CliError::message("this window did not start the relay"));
    }
    RELAY_GEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    RELAY_OURS.store(false, std::sync::atomic::Ordering::SeqCst);
    Ok(())
}

pub(crate) fn relay_is_ours() -> bool {
    RELAY_OURS.load(std::sync::atomic::Ordering::SeqCst)
}

pub(crate) fn has_unsent(dir: &Path) -> Result<bool, CliError> {
    let local = keys::open(dir)?;
    let notes = read_wires(&local.notes_path, &local.store_key)?;
    let pushed = read_pushed(&dir.join("pushed"))?;
    if !notes.is_empty() {
        let recipients = Notebook::open(dir)?.recipients()?;
        if missing_push(&local.device.mailbox, &notes, &recipients, &pushed)? {
            return Ok(true);
        }
    }
    if let Some(identity) = &local.identity {
        for wire in &notes {
            let event = verify(wire).map_err(|_| CliError::message("notes log rejected"))?;
            if !pushed.contains(&(identity.recovery_mailbox, event.id.0)) {
                return Ok(true);
            }
        }
    }
    for slot in load_slots(dir, &local.store_key)? {
        let members = chat_members(&slot.wires)?;
        if missing_push(&slot.card.mailbox, &slot.wires, &members, &pushed)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn missing_push(
    own_mailbox: &[u8; 32],
    wires: &[Vec<u8>],
    members: &[ambient_spaces::MemberInfo],
    pushed: &PushSet,
) -> Result<bool, CliError> {
    for wire in wires {
        let event = verify(wire).map_err(|_| CliError::message("notes log rejected"))?;
        for member in members {
            if member.mailbox == *own_mailbox {
                continue;
            }
            if !pushed.contains(&(member.mailbox, event.id.0)) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

static RELAY_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static RELAY_OURS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn device(dir: &Path, label: &str) -> Result<String, CliError> {
    let (_local, offer) = keys::create_device(dir, label)?;
    Ok(offer)
}

pub(crate) fn card(dir: &Path) -> Result<String, CliError> {
    let slot = ensure_invite(dir)?;
    card_text(&slot.card)
}

pub(crate) fn talk(dir: &Path, card_text: &str) -> Result<String, CliError> {
    let _ = Notebook::open(dir)?;
    let slot = ensure_invite(dir)?;
    let trimmed = card_text.trim();
    let offer_text = trimmed
        .strip_prefix("ambient://contact/")
        .unwrap_or(trimmed);
    let theirs =
        decode_pair_offer_text(offer_text).map_err(|_| CliError::message("card rejected"))?;
    let local = keys::open(dir)?;
    let mut wires = read_wires(&slot.log_path, &local.store_key)?;
    if !wires.is_empty() {
        return Err(CliError::message("that invite was already used"));
    }
    if slot.card.signing_key == theirs.signing_key {
        return Err(CliError::message("that card is this device"));
    }
    let payload = conversation_genesis_payload(&[
        GenesisMember {
            signing_key: slot.card.signing_key,
            encryption_key: slot.card.encryption_key,
            role: 1,
            mailbox: slot.card.mailbox,
        },
        GenesisMember {
            signing_key: theirs.signing_key,
            encryption_key: theirs.encryption_key,
            role: 2,
            mailbox: theirs.mailbox,
        },
    ])
    .map_err(|_| CliError::message("could not encode conversation"))?;
    wires.push(sign_as(
        &slot.card,
        None,
        None,
        None,
        "space.genesis",
        &payload,
    )?);
    write_log(&slot.log_path, &local.store_key, &wires)
        .map_err(|_| CliError::message("could not write chats"))?;
    let view = conversation(&wires)?;
    let id = hex_id(&view.space_id.0);
    if slot
        .log_path
        .parent()
        .is_some_and(|parent| parent.ends_with("invite"))
    {
        let dest = dir.join("conversations").join(&id);
        fs::create_dir_all(dir.join("conversations"))
            .map_err(|_| CliError::message("could not write chats"))?;
        fs::rename(dir.join("invite"), &dest)
            .map_err(|_| CliError::message("could not write chats"))?;
    }
    Ok(id)
}

pub(crate) fn say(dir: &Path, text: &str) -> Result<(), CliError> {
    let id = sole_conversation(dir)?;
    say_in(dir, &id, text)
}

pub(crate) fn say_in(dir: &Path, id: &str, text: &str) -> Result<(), CliError> {
    let local = keys::open(dir)?;
    let mut slots = load_slots(dir, &local.store_key)?;
    let slot = slots
        .iter_mut()
        .find(|slot| slot.space_id().as_deref() == Some(id))
        .ok_or_else(|| CliError::message("no such conversation"))?;
    let view = conversation(&slot.wires)?;
    let payload = msg_put_payload(text).map_err(|_| CliError::message("message is too long"))?;
    let prev = view.tip.get(&slot.card.signing_key).copied();
    slot.wires.push(sign_as(
        &slot.card,
        prev,
        Some(view.space_id),
        now_ms(),
        "msg.put",
        &payload,
    )?);
    write_log(&slot.log_path, &local.store_key, &slot.wires)
        .map_err(|_| CliError::message("could not write chats"))?;
    Ok(())
}

pub(crate) fn hear(dir: &Path) -> Result<String, CliError> {
    match sole_conversation(dir) {
        Ok(id) => hear_in(dir, &id),
        Err(err) if err.to_string() == "no conversation" => Ok(String::new()),
        Err(err) => Err(err),
    }
}

pub(crate) fn hear_in(dir: &Path, id: &str) -> Result<String, CliError> {
    let local = keys::open(dir)?;
    let slots = load_slots(dir, &local.store_key)?;
    let slot = slots
        .iter()
        .find(|slot| slot.space_id().as_deref() == Some(id))
        .ok_or_else(|| CliError::message("no such conversation"))?;
    let view = conversation(&slot.wires)?;
    let mut out = String::new();
    for line in &view.messages {
        push_line(&mut out, line);
    }
    Ok(out)
}

pub(crate) fn conversations(dir: &Path) -> Result<Vec<(String, String)>, CliError> {
    let local = keys::open(dir)?;
    let slots = load_slots(dir, &local.store_key)?;
    let mut out = Vec::new();
    for slot in slots {
        let Some(id) = slot.space_id() else {
            continue;
        };
        let view = conversation(&slot.wires)?;
        let label = view
            .members
            .keys()
            .find(|key| **key != slot.card.signing_key)
            .map_or_else(
                || "Conversation".to_owned(),
                |key| format!("Person {}", short_id(key)),
            );
        out.push((id, label));
    }
    Ok(out)
}

#[allow(clippy::too_many_lines)]
pub(crate) fn sync(dir: &Path, url: &str) -> Result<String, CliError> {
    let mut local = keys::open(dir)?;
    let notes_path = local.notes_path.clone();
    let mut notes = read_wires(&notes_path, &local.store_key)?;
    let mut slots = load_slots(dir, &local.store_key)?;
    let mut requests = Vec::new();
    let mut pushed = read_pushed(&dir.join("pushed"))?;
    let recipients = if notes.is_empty() {
        Vec::new()
    } else {
        Notebook::open(dir)?.recipients()?
    };
    queue_pushes(
        &local.device.mailbox,
        &notes,
        &recipients,
        &mut pushed,
        &mut requests,
    )?;
    queue_recovery(&local, &notes, &mut pushed, &mut requests)?;
    for slot in &slots {
        let members = chat_members(&slot.wires)?;
        queue_pushes(
            &slot.card.mailbox,
            &slot.wires,
            &members,
            &mut pushed,
            &mut requests,
        )?;
    }
    let push_count = requests.len();
    let device_since = read_cursor(&dir.join("cursor-device"))?;
    requests.push(encode_request(&Request::Get {
        mailbox: local.device.mailbox,
        since: device_since,
        limit: 500,
    }));
    for slot in &slots {
        requests.push(encode_request(&Request::Get {
            mailbox: slot.card.mailbox,
            since: slot.cursor,
            limit: 500,
        }));
    }
    let replies = exchange(url, &requests)?;
    if replies.len() < push_count.saturating_add(1).saturating_add(slots.len()) {
        return Err(CliError::message("relay reply missing"));
    }
    for reply in replies.get(..push_count).unwrap_or(&[]) {
        match decode_response(reply) {
            Some(Response::Ok { .. }) => {}
            Some(Response::Err { code }) => return Err(CliError::message(code)),
            _ => return Err(CliError::message("relay reply rejected")),
        }
    }
    let device_blobs = blobs_from(
        replies
            .get(push_count)
            .ok_or_else(|| CliError::message("relay reply missing"))?,
    )?;
    let mut device_cursor = device_since;
    let mut arrived = Vec::new();
    for item in device_blobs {
        device_cursor = item.cursor;
        if let Ok(plain) = open_blob(
            local.device.encryption_secret(),
            &local.device.mailbox,
            &item.blob,
        ) {
            arrived.push(plain);
        }
    }
    let notes_before = notes.len();
    let chats_before: usize = slots.iter().map(|slot| slot.wires.len()).sum();
    let mut dropped = Vec::new();
    file_incoming(&mut local, &mut notes, &mut dropped, &arrived)?;
    for (index, slot) in slots.iter_mut().enumerate() {
        let reply = replies
            .get(push_count.saturating_add(1).saturating_add(index))
            .ok_or_else(|| CliError::message("relay reply missing"))?;
        let mut got = Vec::new();
        for item in blobs_from(reply)? {
            slot.cursor = item.cursor;
            if let Ok(plain) = open_blob(
                slot.card.encryption_secret(),
                &slot.card.mailbox,
                &item.blob,
            ) {
                got.push(plain);
            }
        }
        accept_chat(&mut slot.wires, &got);
    }
    if !notes.is_empty() {
        write_log(&notes_path, &local.store_key, &notes)
            .map_err(|_| CliError::message("could not write notes file"))?;
    }
    for slot in &slots {
        if !slot.wires.is_empty() {
            if let Some(parent) = slot.log_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|_| CliError::message("could not write chats"))?;
            }
            write_log(&slot.log_path, &local.store_key, &slot.wires)
                .map_err(|_| CliError::message("could not write chats"))?;
        }
        write_cursor(&slot.cursor_path, slot.cursor)?;
    }
    write_cursor(&dir.join("cursor-device"), device_cursor)?;
    write_pushed(&dir.join("pushed"), &pushed)?;
    let notes_in = notes.len().saturating_sub(notes_before);
    let chats_in: usize = slots
        .iter()
        .map(|slot| slot.wires.len())
        .sum::<usize>()
        .saturating_sub(chats_before);
    if push_count == 0 && notes_in == 0 && chats_in == 0 {
        return Ok("Nothing new.".to_owned());
    }
    Ok(format!(
        "{push_count} sent, {notes_in} notes in, {chats_in} messages in"
    ))
}

fn queue_pushes(
    own_mailbox: &[u8; 32],
    wires: &[Vec<u8>],
    members: &[ambient_spaces::MemberInfo],
    pushed: &mut PushSet,
    requests: &mut Vec<String>,
) -> Result<(), CliError> {
    for wire in wires {
        let event = verify(wire).map_err(|_| CliError::message("notes log rejected"))?;
        for member in members {
            if member.mailbox == *own_mailbox {
                continue;
            }
            let key = (member.mailbox, event.id.0);
            if !pushed.insert(key) {
                continue;
            }
            let mut ikm = [0_u8; 32];
            getrandom::getrandom(&mut ikm)
                .map_err(|_| CliError::message("random source failed"))?;
            let blob = seal_blob(&member.encryption_key, &member.mailbox, wire, &ikm)
                .map_err(|_| CliError::message("could not seal"))?;
            ikm.fill(0);
            requests.push(encode_request(&Request::Put {
                mailbox: member.mailbox,
                blob,
            }));
        }
    }
    Ok(())
}

fn chat_members(wires: &[Vec<u8>]) -> Result<Vec<ambient_spaces::MemberInfo>, CliError> {
    if wires.is_empty() {
        return Ok(Vec::new());
    }
    Ok(conversation(wires)?.members.into_values().collect())
}

fn conversation(wires: &[Vec<u8>]) -> Result<ambient_spaces::Conversation, CliError> {
    let refs: Vec<&[u8]> = wires.iter().map(Vec::as_slice).collect();
    project_conversation(&refs, &[]).map_err(|_| CliError::message("conversation rejected"))
}

fn file_incoming(
    local: &mut Local,
    notes: &mut Vec<Vec<u8>>,
    chats: &mut Vec<Vec<u8>>,
    arrived: &[Vec<u8>],
) -> Result<(), CliError> {
    let mut pending: Vec<Vec<u8>> = arrived.to_vec();
    for _ in 0..pending.len().saturating_add(1) {
        let batch = std::mem::take(&mut pending);
        if batch.is_empty() {
            break;
        }
        for wire in batch {
            match place(&wire, notes, chats) {
                Place::Notes => append_new(notes, wire),
                Place::Chats => append_new(chats, wire),
                Place::Later => pending.push(wire),
                Place::Skip => {}
            }
        }
    }
    if let Some(public) = identity_public_of(notes) {
        keys::remember_identity(local, public)?;
    }
    Ok(())
}

enum Place {
    Notes,
    Chats,
    Later,
    Skip,
}

fn place(wire: &[u8], notes: &[Vec<u8>], chats: &[Vec<u8>]) -> Place {
    let Ok(event) = verify(wire) else {
        return Place::Skip;
    };
    if seen(notes, event.id.0) || seen(chats, event.id.0) {
        return Place::Skip;
    }
    if event.event_type.starts_with("id.") || event.event_type == "note.put" {
        return Place::Notes;
    }
    if event.event_type == "msg.put" {
        return Place::Chats;
    }
    if event.event_type == "space.genesis" {
        return if genesis_kind(&event.payload) == Some("conversation") {
            Place::Chats
        } else {
            Place::Notes
        };
    }
    let Some(space) = event.space else {
        return Place::Notes;
    };
    if space_ids(chats).contains(&space.0) {
        Place::Chats
    } else if space_ids(notes).contains(&space.0) {
        Place::Notes
    } else {
        Place::Later
    }
}

fn genesis_kind(payload: &[u8]) -> Option<&'static str> {
    if payload
        .windows(b"conversation".len())
        .any(|window| window == b"conversation")
    {
        Some("conversation")
    } else if payload
        .windows(b"notes".len())
        .any(|window| window == b"notes")
    {
        Some("notes")
    } else {
        None
    }
}

fn space_ids(wires: &[Vec<u8>]) -> BTreeSet<[u8; 32]> {
    let mut ids = BTreeSet::new();
    for wire in wires {
        if let Ok(event) = verify(wire)
            && event.event_type == "space.genesis"
        {
            ids.insert(event.id.0);
        }
    }
    ids
}

fn seen(wires: &[Vec<u8>], id: [u8; 32]) -> bool {
    wires
        .iter()
        .any(|wire| verify(wire).ok().is_some_and(|event| event.id.0 == id))
}

fn append_new(wires: &mut Vec<Vec<u8>>, wire: Vec<u8>) {
    if let Ok(event) = verify(&wire)
        && !seen(wires, event.id.0)
    {
        wires.push(wire);
    }
}

fn identity_public_of(wires: &[Vec<u8>]) -> Option<[u8; 32]> {
    for wire in wires {
        if let Ok(event) = verify(wire)
            && event.event_type == "id.genesis"
        {
            return Some(event.signer);
        }
    }
    None
}

fn push_line(out: &mut String, line: &ChatLine) {
    if let Some(ms) = line.unix_ms {
        out.push_str(&format_unix_ms(ms));
        out.push(' ');
    }
    out.push_str(&line.text);
    out.push('\n');
}

fn now_ms() -> Option<u64> {
    let Ok(elapsed) =
        std::time::SystemTime::now().duration_since(std::time::SystemTime::UNIX_EPOCH)
    else {
        return None;
    };
    u64::try_from(elapsed.as_millis()).ok()
}

/// UTC `YYYY-MM-DD HH:MM` from unix milliseconds. Display only. Not an order.
fn format_unix_ms(ms: u64) -> String {
    let secs = ms / 1000;
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let tod = secs % 86_400;
    let hour = tod / 3_600;
    let minute = (tod % 3_600) / 60;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = u64::try_from(z - era * 146_097).unwrap_or(0);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = i64::try_from(yoe).unwrap_or(0) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    if month <= 2 {
        year += 1;
    }
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}")
}

struct Card {
    signing_seed: [u8; 32],
    encryption_secret: [u8; 32],
    signing_key: [u8; 32],
    encryption_key: [u8; 32],
    mailbox: [u8; 32],
}

impl Card {
    fn encryption_secret(&self) -> &[u8; 32] {
        &self.encryption_secret
    }
}

impl Drop for Card {
    fn drop(&mut self) {
        self.signing_seed.fill(0);
        self.encryption_secret.fill(0);
    }
}

fn create_card_at(path: &Path) -> Result<Card, CliError> {
    let sign = random32()?;
    let enc = random32()?;
    let mailbox = random32()?;
    let device = device_from_seeds(&sign, &enc, &mailbox, "contact")
        .map_err(|_| CliError::message("could not make a contact key"))?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"AMBC");
    bytes.push(1);
    bytes.extend_from_slice(&sign);
    bytes.extend_from_slice(&enc);
    bytes.extend_from_slice(&mailbox);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| CliError::message("could not write card"))?;
    }
    fs::write(path, &bytes).map_err(|_| CliError::message("could not write card"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(Card {
        signing_seed: sign,
        encryption_secret: enc,
        signing_key: device.signing_key,
        encryption_key: device.encryption_key,
        mailbox,
    })
}

fn read_card_file(path: &Path) -> Result<Card, CliError> {
    let bytes = fs::read(path).map_err(|_| CliError::message("missing contact card"))?;
    if bytes.get(..4) != Some(b"AMBC") || bytes.get(4) != Some(&1) || bytes.len() < 5 + 96 {
        return Err(CliError::message("contact card is not version 1"));
    }
    let sign = copy32(&bytes, 5)?;
    let enc = copy32(&bytes, 37)?;
    let mailbox = copy32(&bytes, 69)?;
    let device = device_from_seeds(&sign, &enc, &mailbox, "contact")
        .map_err(|_| CliError::message("contact card rejected"))?;
    Ok(Card {
        signing_seed: sign,
        encryption_secret: enc,
        signing_key: device.signing_key,
        encryption_key: device.encryption_key,
        mailbox,
    })
}

fn card_text(card: &Card) -> Result<String, CliError> {
    let bytes = encode_pair_offer(&card.signing_key, &card.encryption_key, &card.mailbox)
        .map_err(|_| CliError::message("could not encode card"))?;
    Ok(pair_offer_text(&bytes))
}

fn sign_as(
    card: &Card,
    prev: Option<ambient_events::EventId>,
    space: Option<ambient_events::EventId>,
    unix_ms: Option<u64>,
    event_type: &str,
    payload: &[u8],
) -> Result<Vec<u8>, CliError> {
    let unsigned = encode_unsigned(
        &card.signing_key,
        prev.map(|id| id.0),
        unix_ms,
        space.map(|id| id.0),
        event_type,
        payload,
    )
    .map_err(|_| CliError::message("could not encode event"))?;
    sign_wire(&unsigned, &card.signing_seed).map_err(|_| CliError::message("could not sign event"))
}

struct Slot {
    card: Card,
    log_path: PathBuf,
    cursor_path: PathBuf,
    wires: Vec<Vec<u8>>,
    cursor: u64,
}

impl Slot {
    fn space_id(&self) -> Option<String> {
        conversation(&self.wires)
            .ok()
            .map(|view| hex_id(&view.space_id.0))
    }
}

fn load_slots(dir: &Path, key: &[u8; 32]) -> Result<Vec<Slot>, CliError> {
    let mut slots = Vec::new();
    if dir.join("card").is_file() {
        slots.push(open_slot(
            dir.join("card"),
            dir.join("chats"),
            dir.join("cursor-contact"),
            key,
        )?);
    }
    if let Ok(read) = fs::read_dir(dir.join("conversations")) {
        let mut folders: Vec<_> = read
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .collect();
        folders.sort();
        for folder in folders {
            if folder.join("card").is_file() {
                slots.push(open_slot(
                    folder.join("card"),
                    folder.join("log"),
                    folder.join("cursor"),
                    key,
                )?);
            }
        }
    }
    let invite = dir.join("invite");
    if invite.join("card").is_file() {
        slots.push(open_slot(
            invite.join("card"),
            invite.join("log"),
            invite.join("cursor"),
            key,
        )?);
    }
    Ok(slots)
}

fn open_slot(
    card_path: impl AsRef<Path>,
    log_path: impl AsRef<Path>,
    cursor_path: impl AsRef<Path>,
    key: &[u8; 32],
) -> Result<Slot, CliError> {
    let card_path = card_path.as_ref();
    let log_path = log_path.as_ref();
    let cursor_path = cursor_path.as_ref();
    Ok(Slot {
        card: read_card_file(card_path)?,
        wires: read_wires(log_path, key)?,
        cursor: read_cursor(cursor_path)?,
        log_path: log_path.to_path_buf(),
        cursor_path: cursor_path.to_path_buf(),
    })
}

fn ensure_invite(dir: &Path) -> Result<Slot, CliError> {
    let local = keys::open(dir)?;
    let legacy_free = dir.join("card").is_file()
        && !dir.join("chats").is_file()
        && !dir.join("conversations").exists();
    if legacy_free
        || (!dir.join("card").is_file()
            && !dir.join("conversations").exists()
            && !dir.join("invite").exists())
    {
        if !dir.join("card").is_file() {
            create_card_at(&dir.join("card"))?;
        }
        return open_slot(
            dir.join("card"),
            dir.join("chats"),
            dir.join("cursor-contact"),
            &local.store_key,
        );
    }
    let invite = dir.join("invite");
    if !invite.join("card").is_file() {
        fs::create_dir_all(&invite).map_err(|_| CliError::message("could not write card"))?;
        create_card_at(&invite.join("card"))?;
    }
    open_slot(
        invite.join("card"),
        invite.join("log"),
        invite.join("cursor"),
        &local.store_key,
    )
}

fn sole_conversation(dir: &Path) -> Result<String, CliError> {
    let local = keys::open(dir)?;
    let slots = load_slots(dir, &local.store_key)?;
    let mut ids = Vec::new();
    for slot in &slots {
        if let Some(id) = slot.space_id() {
            ids.push(id);
        }
    }
    match ids.as_slice() {
        [id] => Ok(id.clone()),
        [] => Err(CliError::message("no conversation")),
        _ => Err(CliError::message("more than one conversation")),
    }
}

fn accept_chat(wires: &mut Vec<Vec<u8>>, arrived: &[Vec<u8>]) {
    let mut pending: Vec<Vec<u8>> = arrived.to_vec();
    for _ in 0..pending.len().saturating_add(1) {
        let batch = std::mem::take(&mut pending);
        if batch.is_empty() {
            break;
        }
        for wire in batch {
            match place(&wire, &[], wires) {
                Place::Chats => append_new(wires, wire),
                Place::Later => pending.push(wire),
                Place::Notes | Place::Skip => {}
            }
        }
    }
}

fn hex_id(bytes: &[u8]) -> String {
    crate::session::hex(bytes)
}

fn short_id(bytes: &[u8]) -> String {
    crate::session::hex(bytes).chars().take(8).collect()
}

fn read_wires(path: &Path, key: &[u8; 32]) -> Result<Vec<Vec<u8>>, CliError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    read_log(path, key).map_err(|_| CliError::message("could not open notes file"))
}

fn read_cursor(path: &Path) -> Result<u64, CliError> {
    if !path.exists() {
        return Ok(0);
    }
    let bytes = fs::read(path).map_err(|_| CliError::message("could not read cursor"))?;
    let sized: [u8; 8] = bytes
        .get(..8)
        .ok_or_else(|| CliError::message("could not read cursor"))?
        .try_into()
        .map_err(|_| CliError::message("could not read cursor"))?;
    Ok(u64::from_le_bytes(sized))
}

fn write_cursor(path: &Path, cursor: u64) -> Result<(), CliError> {
    fs::write(path, cursor.to_le_bytes()).map_err(|_| CliError::message("could not write cursor"))
}

type PushSet = BTreeSet<([u8; 32], [u8; 32])>;

fn read_pushed(path: &Path) -> Result<PushSet, CliError> {
    if !path.exists() {
        return Ok(BTreeSet::new());
    }
    let bytes = fs::read(path).map_err(|_| CliError::message("could not read push list"))?;
    let mut out = BTreeSet::new();
    let mut offset = 0;
    while offset + 64 <= bytes.len() {
        let mailbox = copy32(&bytes, offset)?;
        let id = copy32(&bytes, offset + 32)?;
        out.insert((mailbox, id));
        offset += 64;
    }
    Ok(out)
}

fn write_pushed(path: &Path, pushed: &PushSet) -> Result<(), CliError> {
    let mut bytes = Vec::new();
    for (mailbox, id) in pushed {
        bytes.extend_from_slice(mailbox);
        bytes.extend_from_slice(id);
    }
    fs::write(path, bytes).map_err(|_| CliError::message("could not write push list"))
}

fn copy32(bytes: &[u8], start: usize) -> Result<[u8; 32], CliError> {
    let slice = bytes
        .get(start..start + 32)
        .ok_or_else(|| CliError::message("truncated"))?;
    let mut out = [0_u8; 32];
    out.copy_from_slice(slice);
    Ok(out)
}

fn random32() -> Result<[u8; 32], CliError> {
    let mut bytes = [0_u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|_| CliError::message("random source failed"))?;
    Ok(bytes)
}

fn blobs_from(line: &str) -> Result<Vec<ambient_transport::Item>, CliError> {
    match decode_response(line) {
        Some(Response::Blobs { items }) => Ok(items),
        Some(Response::Err { code }) => Err(CliError::message(code)),
        _ => Err(CliError::message("relay reply rejected")),
    }
}

fn exchange(url: &str, requests: &[String]) -> Result<Vec<String>, CliError> {
    if requests.is_empty() {
        return Ok(Vec::new());
    }
    let (mut socket, _) =
        tungstenite::connect(url).map_err(|_| CliError::message("relay unreachable"))?;
    let mut replies = Vec::new();
    for request in requests {
        socket
            .send(Message::Text(request.clone().into()))
            .map_err(|_| CliError::message("relay write failed"))?;
        match socket.read() {
            Ok(Message::Text(text)) => replies.push(text.to_string()),
            _ => return Err(CliError::message("relay read failed")),
        }
    }
    let _ = socket.close(None);
    Ok(replies)
}

fn queue_recovery(
    local: &Local,
    notes: &[Vec<u8>],
    pushed: &mut PushSet,
    requests: &mut Vec<String>,
) -> Result<(), CliError> {
    let Some(identity) = &local.identity else {
        return Ok(());
    };
    for wire in notes {
        let Ok(event) = verify(wire) else {
            continue;
        };
        let key = (identity.recovery_mailbox, event.id.0);
        if !pushed.insert(key) {
            continue;
        }
        let mut ikm = [0_u8; 32];
        if getrandom::getrandom(&mut ikm).is_err() {
            return Err(CliError::message("random source failed"));
        }
        match seal_blob(&identity.seal_key, &identity.recovery_mailbox, wire, &ikm) {
            Ok(blob) => {
                ikm.fill(0);
                requests.push(encode_request(&Request::Put {
                    mailbox: identity.recovery_mailbox,
                    blob,
                }));
            }
            Err(_) => {
                ikm.fill(0);
                return Err(CliError::message("could not seal recovery"));
            }
        }
    }
    Ok(())
}

/// Restore from recovery phrase. New device is added after the log is fetched.
pub(crate) fn restore(dir: &Path, phrase: &str, url: &str) -> Result<(), CliError> {
    if dir.join("key").exists() {
        return Err(CliError::message("folder already has a key"));
    }
    fs::create_dir_all(dir).map_err(|_| CliError::message("could not create directory"))?;
    let root = crate::keys::root_from_phrase(phrase)?;
    let identity =
        ambient_identity::derive(&root).map_err(|_| CliError::message("derivation failed"))?;
    let sign = random32()?;
    let enc = random32()?;
    let mailbox = random32()?;
    let label = "restored";
    let _device = ambient_identity::device_from_seeds(&sign, &enc, &mailbox, label)
        .map_err(|_| CliError::message("could not make device"))?;
    let key_bytes = crate::keys::v1_bytes_for_restore(&root, &sign, &enc, &mailbox, label)?;
    crate::keys::write_secret_pub(&dir.join("key"), &key_bytes)?;
    let local = keys::open(dir)?;
    let requests = vec![encode_request(&Request::Get {
        mailbox: identity.recovery_mailbox,
        since: 0,
        limit: 500,
    })];
    let replies = exchange(url, &requests)?;
    let Some(reply) = replies.first() else {
        return Err(CliError::message("relay reply missing"));
    };
    let blobs = blobs_from(reply)?;
    let mut notes = Vec::new();
    for item in blobs {
        if let Ok(plain) = open_blob(
            identity.seal_secret(),
            &identity.recovery_mailbox,
            &item.blob,
        ) {
            notes.push(plain);
        }
    }
    if notes.is_empty() {
        return Err(CliError::message("no recovery found"));
    }
    write_log(&local.notes_path, &local.store_key, &notes)
        .map_err(|_| CliError::message("could not write notes file"))?;
    Notebook::open(dir)?.enroll_self()?;
    Ok(())
}
