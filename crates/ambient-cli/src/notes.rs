use std::io::{Read, Write};
use std::path::Path;

use ambient_events::{EventId, encode_unsigned, sign_wire, verify};
use ambient_identity::{DeviceState, device_add_payload, project_log};
use ambient_spaces::{
    GenesisMember, Revocation, note_put_payload, notes_genesis_payload, project_notes,
};
use ambient_store::{read_log, write_log};

use crate::keys::{self, Local};

/// A CLI failure. The text is safe to print. It does not include note bodies.
#[derive(Debug)]
pub struct CliError {
    message: String,
}

impl CliError {
    pub(crate) fn message(text: impl Into<String>) -> Self {
        Self {
            message: text.into(),
        }
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for CliError {}

/// Run one notes command. `args` is everything after the program name.
///
/// # Errors
///
/// Returns a message for unknown commands, a bad directory, or a rejected event.
pub fn run(
    args: &mut impl Iterator<Item = String>,
    stdin: &mut impl Read,
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let command = args.next().ok_or_else(|| {
        CliError::message("usage: ambient <init|add|edit|list|search|show> <dir> ...")
    })?;
    match command.as_str() {
        "init" => init(args, stdout),
        "add" => add(args, stdin, stdout),
        "edit" => edit(args, stdin, stdout),
        "list" => list(args, stdout),
        "search" => search(args, stdout),
        "show" => show(args, stdout),
        _ => Err(CliError::message("unknown command")),
    }
}

fn init(args: &mut impl Iterator<Item = String>, stdout: &mut impl Write) -> Result<(), CliError> {
    let dir = args
        .next()
        .ok_or_else(|| CliError::message("usage: ambient init <dir>"))?;
    let local = keys::create(Path::new(&dir), "this-computer")?;
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
    writeln!(stdout, "initialized {dir}").map_err(|_| CliError::message("stdout"))?;
    Ok(())
}

fn add(
    args: &mut impl Iterator<Item = String>,
    stdin: &mut impl Read,
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let dir = args
        .next()
        .ok_or_else(|| CliError::message("usage: ambient add <dir> <title>"))?;
    let title = args
        .next()
        .ok_or_else(|| CliError::message("usage: ambient add <dir> <title>"))?;
    let body = read_stdin(stdin)?;
    let local = keys::open(Path::new(&dir))?;
    let mut wires = load(&local)?;
    let view = view_notes(&local, &wires)?;
    let note_id = random_note()?;
    let payload = note_put_payload(&note_id, &title, &body, &[])
        .map_err(|_| CliError::message("title must be at most 200 bytes"))?;
    let prev = view.tip.get(&local.device.signing_key).copied();
    let wire = sign_device(&local, prev, Some(view.space_id), "note.put", &payload)?;
    wires.push(wire);
    write_log(&local.notes_path, &local.store_key, &wires)
        .map_err(|_| CliError::message("could not write notes file"))?;
    writeln!(stdout, "{}", hex(&note_id)).map_err(|_| CliError::message("stdout"))?;
    Ok(())
}

fn edit(
    args: &mut impl Iterator<Item = String>,
    stdin: &mut impl Read,
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let dir = args
        .next()
        .ok_or_else(|| CliError::message("usage: ambient edit <dir> <note-id> <title>"))?;
    let note_hex = args
        .next()
        .ok_or_else(|| CliError::message("usage: ambient edit <dir> <note-id> <title>"))?;
    let title = args
        .next()
        .ok_or_else(|| CliError::message("usage: ambient edit <dir> <note-id> <title>"))?;
    let note_id = parse_hex32(&note_hex)?;
    let body = read_stdin(stdin)?;
    let local = keys::open(Path::new(&dir))?;
    let mut wires = load(&local)?;
    let view = view_notes(&local, &wires)?;
    let heads = view
        .notes
        .get(&note_id)
        .ok_or_else(|| CliError::message("no such note"))?;
    let parents: Vec<EventId> = heads.iter().map(|head| head.event_id).collect();
    let payload = note_put_payload(&note_id, &title, &body, &parents)
        .map_err(|_| CliError::message("title must be at most 200 bytes"))?;
    let prev = view.tip.get(&local.device.signing_key).copied();
    let wire = sign_device(&local, prev, Some(view.space_id), "note.put", &payload)?;
    wires.push(wire);
    write_log(&local.notes_path, &local.store_key, &wires)
        .map_err(|_| CliError::message("could not write notes file"))?;
    writeln!(stdout, "updated {}", hex(&note_id)).map_err(|_| CliError::message("stdout"))?;
    Ok(())
}

fn list(args: &mut impl Iterator<Item = String>, stdout: &mut impl Write) -> Result<(), CliError> {
    let dir = args
        .next()
        .ok_or_else(|| CliError::message("usage: ambient list <dir>"))?;
    let local = keys::open(Path::new(&dir))?;
    let wires = load(&local)?;
    let view = view_notes(&local, &wires)?;
    for (id, heads) in &view.notes {
        let title = heads.first().map_or("", |head| head.title.as_str());
        let mark = if heads.len() > 1 { " conflict" } else { "" };
        writeln!(stdout, "{} {title}{mark}", hex(id)).map_err(|_| CliError::message("stdout"))?;
    }
    Ok(())
}

fn search(
    args: &mut impl Iterator<Item = String>,
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let dir = args
        .next()
        .ok_or_else(|| CliError::message("usage: ambient search <dir> <query>"))?;
    let query = args
        .next()
        .ok_or_else(|| CliError::message("usage: ambient search <dir> <query>"))?;
    if query.is_empty() {
        return Err(CliError::message("empty query"));
    }
    let local = keys::open(Path::new(&dir))?;
    let wires = load(&local)?;
    let view = view_notes(&local, &wires)?;
    for (id, heads) in &view.notes {
        if heads
            .iter()
            .any(|head| head.title.contains(&query) || head.body.contains(&query))
        {
            let title = heads.first().map_or("", |head| head.title.as_str());
            writeln!(stdout, "{} {title}", hex(id)).map_err(|_| CliError::message("stdout"))?;
        }
    }
    Ok(())
}

fn show(args: &mut impl Iterator<Item = String>, stdout: &mut impl Write) -> Result<(), CliError> {
    let dir = args
        .next()
        .ok_or_else(|| CliError::message("usage: ambient show <dir> <note-id>"))?;
    let note_id = parse_hex32(
        &args
            .next()
            .ok_or_else(|| CliError::message("usage: ambient show <dir> <note-id>"))?,
    )?;
    let local = keys::open(Path::new(&dir))?;
    let wires = load(&local)?;
    let view = view_notes(&local, &wires)?;
    let heads = view
        .notes
        .get(&note_id)
        .ok_or_else(|| CliError::message("no such note"))?;
    for (index, head) in heads.iter().enumerate() {
        if heads.len() > 1 {
            writeln!(stdout, "# conflict {}", index + 1)
                .map_err(|_| CliError::message("stdout"))?;
        }
        writeln!(stdout, "{}", head.body).map_err(|_| CliError::message("stdout"))?;
    }
    Ok(())
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

fn read_stdin(stdin: &mut impl Read) -> Result<String, CliError> {
    let mut body = String::new();
    stdin
        .read_to_string(&mut body)
        .map_err(|_| CliError::message("could not read stdin"))?;
    Ok(body)
}

fn random_note() -> Result<[u8; 32], CliError> {
    let mut bytes = [0_u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|_| CliError::message("random source failed"))?;
    Ok(bytes)
}

fn parse_hex32(text: &str) -> Result<[u8; 32], CliError> {
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

fn hex(bytes: &[u8]) -> String {
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
