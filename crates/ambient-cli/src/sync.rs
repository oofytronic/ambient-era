//! Push and pull sealed events through a relay. The notes window does not do this.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

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

pub(crate) fn device(dir: &Path, label: &str) -> Result<String, CliError> {
    let (_local, offer) = keys::create_device(dir, label)?;
    Ok(offer)
}

pub(crate) fn card(dir: &Path) -> Result<String, CliError> {
    let (_contact, text) = load_or_create_card(dir)?;
    Ok(text)
}

pub(crate) fn talk(dir: &Path, card_text: &str) -> Result<(), CliError> {
    let _ = Notebook::open(dir)?;
    let (ours, _) = load_or_create_card(dir)?;
    let theirs =
        decode_pair_offer_text(card_text.trim()).map_err(|_| CliError::message("card rejected"))?;
    let path = dir.join("chats");
    let local = keys::open(dir)?;
    let mut wires = read_wires(&path, &local.store_key)?;
    if !wires.is_empty() {
        return Err(CliError::message("this folder already has a conversation"));
    }
    if ours.signing_key == theirs.signing_key {
        return Err(CliError::message("that card is this device"));
    }
    let payload = conversation_genesis_payload(&[
        GenesisMember {
            signing_key: ours.signing_key,
            encryption_key: ours.encryption_key,
            role: 1,
            mailbox: ours.mailbox,
        },
        GenesisMember {
            signing_key: theirs.signing_key,
            encryption_key: theirs.encryption_key,
            role: 2,
            mailbox: theirs.mailbox,
        },
    ])
    .map_err(|_| CliError::message("could not encode conversation"))?;
    wires.push(sign_as(&ours, None, None, "space.genesis", &payload)?);
    write_log(&path, &local.store_key, &wires)
        .map_err(|_| CliError::message("could not write chats"))?;
    Ok(())
}

pub(crate) fn say(dir: &Path, text: &str) -> Result<(), CliError> {
    let local = keys::open(dir)?;
    let (ours, _) = load_or_create_card(dir)?;
    let path = dir.join("chats");
    let mut wires = read_wires(&path, &local.store_key)?;
    let view = conversation(&wires)?;
    let payload = msg_put_payload(text).map_err(|_| CliError::message("message is too long"))?;
    let prev = view.tip.get(&ours.signing_key).copied();
    wires.push(sign_as(
        &ours,
        prev,
        Some(view.space_id),
        "msg.put",
        &payload,
    )?);
    write_log(&path, &local.store_key, &wires)
        .map_err(|_| CliError::message("could not write chats"))?;
    Ok(())
}

pub(crate) fn hear(dir: &Path) -> Result<String, CliError> {
    let local = keys::open(dir)?;
    let wires = read_wires(&dir.join("chats"), &local.store_key)?;
    if wires.is_empty() {
        return Ok(String::new());
    }
    let view = conversation(&wires)?;
    let mut out = String::new();
    for line in &view.messages {
        push_line(&mut out, line);
    }
    Ok(out)
}

pub(crate) fn sync(dir: &Path, url: &str) -> Result<String, CliError> {
    let mut local = keys::open(dir)?;
    let notes_path = local.notes_path.clone();
    let mut notes = read_wires(&notes_path, &local.store_key)?;
    let chats_path = dir.join("chats");
    let mut chats = read_wires(&chats_path, &local.store_key)?;
    let card = read_card(dir).ok();
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
    let members = chat_members(&chats)?;
    if let Some(card) = &card {
        queue_pushes(&card.mailbox, &chats, &members, &mut pushed, &mut requests)?;
    }
    let device_since = read_cursor(&dir.join("cursor-device"))?;
    requests.push(encode_request(&Request::Get {
        mailbox: local.device.mailbox,
        since: device_since,
        limit: 500,
    }));
    let contact_since = read_cursor(&dir.join("cursor-contact"))?;
    if let Some(card) = &card {
        requests.push(encode_request(&Request::Get {
            mailbox: card.mailbox,
            since: contact_since,
            limit: 500,
        }));
    }
    let replies = exchange(url, &requests)?;
    let gets = usize::from(card.is_some()) + 1;
    if replies.len() < gets {
        return Err(CliError::message("relay reply missing"));
    }
    let device_at = replies.len() - gets;
    for reply in replies.get(..device_at).unwrap_or(&[]) {
        match decode_response(reply) {
            Some(Response::Ok { .. }) => {}
            Some(Response::Err { code }) => return Err(CliError::message(code)),
            _ => return Err(CliError::message("relay reply rejected")),
        }
    }
    let device_blobs = blobs_from(
        replies
            .get(device_at)
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
    let mut contact_cursor = contact_since;
    if let Some(card) = &card {
        let reply = replies
            .last()
            .ok_or_else(|| CliError::message("relay reply missing"))?;
        for item in blobs_from(reply)? {
            contact_cursor = item.cursor;
            if let Ok(plain) = open_blob(card.encryption_secret(), &card.mailbox, &item.blob) {
                arrived.push(plain);
            }
        }
    }
    file_incoming(&mut local, &mut notes, &mut chats, &arrived)?;
    if !notes.is_empty() {
        write_log(&notes_path, &local.store_key, &notes)
            .map_err(|_| CliError::message("could not write notes file"))?;
    }
    if !chats.is_empty() {
        write_log(&chats_path, &local.store_key, &chats)
            .map_err(|_| CliError::message("could not write chats"))?;
    }
    write_cursor(&dir.join("cursor-device"), device_cursor)?;
    write_cursor(&dir.join("cursor-contact"), contact_cursor)?;
    write_pushed(&dir.join("pushed"), &pushed)?;
    Ok(format!("synced {url}"))
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
    out.push_str(&line.text);
    out.push('\n');
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

fn load_or_create_card(dir: &Path) -> Result<(Card, String), CliError> {
    let path = dir.join("card");
    if path.exists() {
        let card = read_card(dir)?;
        let text = card_text(&card)?;
        return Ok((card, text));
    }
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
    fs::write(&path, &bytes).map_err(|_| CliError::message("could not write card"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    let card = Card {
        signing_seed: sign,
        encryption_secret: enc,
        signing_key: device.signing_key,
        encryption_key: device.encryption_key,
        mailbox,
    };
    let text = card_text(&card)?;
    Ok((card, text))
}

fn read_card(dir: &Path) -> Result<Card, CliError> {
    let bytes =
        fs::read(dir.join("card")).map_err(|_| CliError::message("missing contact card"))?;
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
    event_type: &str,
    payload: &[u8],
) -> Result<Vec<u8>, CliError> {
    let unsigned = encode_unsigned(
        &card.signing_key,
        prev.map(|id| id.0),
        None,
        space.map(|id| id.0),
        event_type,
        payload,
    )
    .map_err(|_| CliError::message("could not encode event"))?;
    sign_wire(&unsigned, &card.signing_seed).map_err(|_| CliError::message("could not sign event"))
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
