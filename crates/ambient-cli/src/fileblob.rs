//! A chat file sealed to the other members of a conversation.
//!
//! The message still only cites the file. These bytes are not a log event.
//! A relay blob is at most 4 MiB, so a larger file is several blobs.

use std::fs;
use std::path::Path;

const MAGIC: &[u8] = b"ambient-era/file/v1\n";
/// Plaintext chunk, small enough that the seal stays under the relay limit.
const CHUNK: usize = 3_000_000;
const MAX_FILE: usize = 256 * 1024 * 1024;

pub(crate) struct Piece {
    pub space: [u8; 32],
    pub file_id: [u8; 8],
    pub index: u32,
    pub count: u32,
    pub name: String,
    pub bytes: Vec<u8>,
}

pub(crate) fn encode(piece: &Piece) -> Vec<u8> {
    let name = piece.name.as_bytes();
    let mut out = Vec::with_capacity(
        MAGIC
            .len()
            .saturating_add(64)
            .saturating_add(piece.bytes.len()),
    );
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&piece.space);
    out.extend_from_slice(&piece.file_id);
    out.extend_from_slice(&piece.index.to_be_bytes());
    out.extend_from_slice(&piece.count.to_be_bytes());
    let name_len = u16::try_from(name.len()).unwrap_or(0);
    out.extend_from_slice(&name_len.to_be_bytes());
    out.extend_from_slice(name.get(..usize::from(name_len)).unwrap_or_default());
    out.extend_from_slice(&piece.bytes);
    out
}

pub(crate) fn decode(plain: &[u8]) -> Option<Piece> {
    if !plain.starts_with(MAGIC) {
        return None;
    }
    let rest = plain.get(MAGIC.len()..)?;
    if rest.len() < 32 + 8 + 4 + 4 + 2 {
        return None;
    }
    let mut space = [0_u8; 32];
    space.copy_from_slice(rest.get(..32)?);
    let mut file_id = [0_u8; 8];
    file_id.copy_from_slice(rest.get(32..40)?);
    let index = u32::from_be_bytes(rest.get(40..44)?.try_into().ok()?);
    let count = u32::from_be_bytes(rest.get(44..48)?.try_into().ok()?);
    let name_len = usize::from(u16::from_be_bytes(rest.get(48..50)?.try_into().ok()?));
    let name_bytes = rest.get(50..50 + name_len)?;
    let name = String::from_utf8(name_bytes.to_vec()).ok()?;
    if !name_ok(&name) || count == 0 || index >= count {
        return None;
    }
    if usize::try_from(count).ok()? > MAX_FILE / CHUNK + 1 {
        return None;
    }
    let bytes = rest.get(50 + name_len..)?.to_vec();
    if bytes.len() > CHUNK {
        return None;
    }
    Some(Piece {
        space,
        file_id,
        index,
        count,
        name,
        bytes,
    })
}

/// Write one piece. When every piece is here, the file is the name the cite uses.
pub(crate) fn store(dir: &Path, piece: &Piece) -> Result<(), String> {
    let conversation = hex(&piece.space);
    let id = hex(&piece.file_id);
    let folder = dir.join("chat-files").join(&conversation);
    fs::create_dir_all(&folder).map_err(|_| "could not save the file".to_owned())?;
    let part = folder.join(format!("{id}.part-{}", piece.index));
    let tmp = folder.join(format!("{id}.part-{}.partial", piece.index));
    fs::write(&tmp, &piece.bytes).map_err(|_| "could not save the file".to_owned())?;
    fs::rename(&tmp, &part).map_err(|_| "could not save the file".to_owned())?;
    let mut whole = Vec::new();
    for index in 0..piece.count {
        let path = folder.join(format!("{id}.part-{index}"));
        let Ok(bytes) = fs::read(&path) else {
            return Ok(());
        };
        if whole.len().saturating_add(bytes.len()) > MAX_FILE {
            return Err("that file is larger than 256 MB".to_owned());
        }
        whole.extend(bytes);
    }
    let dest = folder.join(format!("{id}-{}", piece.name));
    let finished = folder.join(format!("{id}.partial"));
    fs::write(&finished, &whole).map_err(|_| "could not save the file".to_owned())?;
    fs::rename(&finished, &dest).map_err(|_| "could not save the file".to_owned())?;
    Ok(())
}

pub(crate) fn pieces(space: [u8; 32], file_id: [u8; 8], name: &str, bytes: &[u8]) -> Vec<Piece> {
    let count = bytes.len().div_ceil(CHUNK).max(1);
    let count = u32::try_from(count).unwrap_or(u32::MAX);
    let mut out = Vec::new();
    let mut index = 0_u32;
    if bytes.is_empty() {
        out.push(Piece {
            space,
            file_id,
            index: 0,
            count: 1,
            name: name.to_owned(),
            bytes: Vec::new(),
        });
        return out;
    }
    for chunk in bytes.chunks(CHUNK) {
        out.push(Piece {
            space,
            file_id,
            index,
            count,
            name: name.to_owned(),
            bytes: chunk.to_vec(),
        });
        index = index.saturating_add(1);
    }
    out
}

/// Identity of one chunk in the per-relay push list. Not an event id.
pub(crate) fn push_id(space: &[u8; 32], file_id: [u8; 8], index: u32) -> [u8; 32] {
    let mut key = [0_u8; 32];
    key[..16].copy_from_slice(space.get(..16).unwrap_or(&[0; 16]));
    key[16..24].copy_from_slice(&file_id);
    key[24..28].copy_from_slice(&index.to_be_bytes());
    key[28..32].copy_from_slice(b"file");
    key
}

pub(crate) fn parse_id(text: &str) -> Option<[u8; 8]> {
    if text.len() != 16 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0_u8; 8];
    for (index, byte) in out.iter_mut().enumerate() {
        let pair = text.get(index * 2..index * 2 + 2)?;
        *byte = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(out)
}

pub(crate) fn cites(wire: &[u8]) -> Vec<[u8; 8]> {
    let mut out = Vec::new();
    let marker = b"aerodash-file:";
    let mut rest = wire;
    while let Some(at) = find_slice(rest, marker) {
        let after = at.saturating_add(marker.len());
        let tail = rest.get(after..).unwrap_or(b"");
        let hex_len = tail
            .iter()
            .take_while(|byte| byte.is_ascii_hexdigit())
            .count();
        if let Some(text) = tail
            .get(..hex_len)
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            && let Some(id) = parse_id(text)
        {
            out.push(id);
        }
        rest = rest.get(after..).unwrap_or(b"");
    }
    out
}

fn find_slice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len())
        .position(|window| window == needle)
}

fn name_ok(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 60
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..")
        && name
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || matches!(char, '.' | '-' | '_' | ' '))
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{CHUNK, decode, encode, parse_id, pieces, store};

    #[test]
    fn a_piece_round_trips() {
        let made = pieces([9; 32], [4; 8], "note.txt", b"hello file");
        assert_eq!(made.len(), 1);
        let back = decode(&encode(&made[0])).expect("decode");
        assert_eq!(back.bytes, b"hello file");
        assert_eq!(back.name, "note.txt");
        assert_eq!(back.file_id, [4; 8]);
    }

    #[test]
    fn an_event_is_not_a_file() {
        assert!(decode(b"not a file").is_none());
        assert!(parse_id("abcd").is_none());
        assert!(parse_id("0123456789abcdef").is_some());
    }

    #[test]
    fn pieces_cover_the_bytes() {
        let bytes = vec![1_u8; CHUNK + 3];
        let made = pieces([0; 32], [1; 8], "a.bin", &bytes);
        assert_eq!(made.len(), 2);
        assert_eq!(made[1].bytes, vec![1, 1, 1]);
        assert_eq!(made[0].count, 2);
    }

    #[test]
    fn store_assembles_when_the_last_piece_arrives() {
        let dir = std::env::temp_dir().join(format!(
            "ambient-fileblob-{}-{}",
            std::process::id(),
            parse_id("0123456789abcdef").is_some()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let first = pieces([3; 32], [4; 8], "note.txt", b"hello file")
            .into_iter()
            .next()
            .unwrap();
        store(&dir, &first).unwrap();
        let path = dir
            .join("chat-files")
            .join("0303030303030303030303030303030303030303030303030303030303030303")
            .join("0404040404040404-note.txt");
        assert_eq!(std::fs::read(&path).unwrap(), b"hello file");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
