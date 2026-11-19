use std::fs;
use std::io::{Read, Write};
use std::path::Path;

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};

use crate::error::StoreError;

const MAGIC: &[u8; 4] = b"AMBS";
const AAD_PREFIX: &[u8] = b"ambient-store/v1";

/// Seal wires under `key`. `counter` is the nonce and must never repeat for this key.
///
/// # Errors
///
/// Returns [`StoreError::BadKey`] if a length does not fit, or if the AEAD rejects the seal.
pub fn seal_log(key: &[u8; 32], counter: u64, wires: &[Vec<u8>]) -> Result<Vec<u8>, StoreError> {
    let mut plain = Vec::new();
    for wire in wires {
        let len = u32::try_from(wire.len()).map_err(|_| StoreError::BadKey)?;
        plain.extend_from_slice(&len.to_be_bytes());
        plain.extend_from_slice(wire);
    }
    let cipher = ChaCha20Poly1305::new(key.into());
    let nonce = nonce(counter);
    let mut aad = Vec::from(AAD_PREFIX);
    aad.extend_from_slice(&counter.to_be_bytes());
    let ciphertext = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: &plain,
                aad: &aad,
            },
        )
        .map_err(|_| StoreError::BadKey)?;
    let mut out = Vec::with_capacity(13 + ciphertext.len());
    out.extend_from_slice(MAGIC);
    out.push(1);
    out.extend_from_slice(&counter.to_be_bytes());
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Open a sealed log. The returned counter is the one that was used, not the next one.
///
/// # Errors
///
/// Returns [`StoreError::Corrupt`] when the header, tag, or framing is wrong.
pub fn open_log(key: &[u8; 32], bytes: &[u8]) -> Result<(u64, Vec<Vec<u8>>), StoreError> {
    if bytes.len() < 13 || bytes.get(..4) != Some(MAGIC) || bytes.get(4) != Some(&1) {
        return Err(StoreError::Corrupt);
    }
    let counter_bytes: [u8; 8] = bytes
        .get(5..13)
        .ok_or(StoreError::Corrupt)?
        .try_into()
        .map_err(|_| StoreError::Corrupt)?;
    let counter = u64::from_be_bytes(counter_bytes);
    let cipher = ChaCha20Poly1305::new(key.into());
    let nonce = nonce(counter);
    let mut aad = Vec::from(AAD_PREFIX);
    aad.extend_from_slice(&counter.to_be_bytes());
    let ciphertext = bytes.get(13..).ok_or(StoreError::Corrupt)?;
    let plain = cipher
        .decrypt(
            &nonce,
            Payload {
                msg: ciphertext,
                aad: &aad,
            },
        )
        .map_err(|_| StoreError::Corrupt)?;
    Ok((counter, split_wires(&plain)?))
}

/// Read a log file.
///
/// # Errors
///
/// Returns [`StoreError::Io`] when the file cannot be read.
pub fn read_log(path: &Path, key: &[u8; 32]) -> Result<Vec<Vec<u8>>, StoreError> {
    let bytes = fs::read(path).map_err(|_| StoreError::Io)?;
    Ok(open_log(key, &bytes)?.1)
}

/// The counter and length in a log header, without decrypting.
///
/// `None` when the file is not there yet. Readers use this to skip a decrypt
/// when the file has not changed.
///
/// # Errors
///
/// Returns [`StoreError::Io`] or [`StoreError::Corrupt`] when the header cannot be read.
pub fn log_stamp(path: &Path) -> Result<Option<(u64, u64)>, StoreError> {
    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(StoreError::Io),
    };
    let len = file.metadata().map_err(|_| StoreError::Io)?.len();
    let mut header = [0_u8; 13];
    file.read_exact(&mut header).map_err(|err| {
        if err.kind() == std::io::ErrorKind::UnexpectedEof {
            StoreError::Corrupt
        } else {
            StoreError::Io
        }
    })?;
    if header.get(..4) != Some(MAGIC) || header.get(4) != Some(&1) {
        return Err(StoreError::Corrupt);
    }
    let counter_bytes: [u8; 8] = header
        .get(5..13)
        .ok_or(StoreError::Corrupt)?
        .try_into()
        .map_err(|_| StoreError::Corrupt)?;
    Ok(Some((u64::from_be_bytes(counter_bytes), len)))
}

/// Write a log file, incrementing the counter when the file already exists.
///
/// The previous file is opened first, so a file this key cannot open is never overwritten.
/// The write is durable and atomic: see [`write_log_if`]. Prefer that function when the caller
/// already read the file, because it does not decrypt the old bytes again and it refuses to
/// overwrite a write it never saw.
///
/// # Errors
///
/// Returns [`StoreError::Io`] or [`StoreError::Corrupt`] from the previous file.
pub fn write_log(path: &Path, key: &[u8; 32], wires: &[Vec<u8>]) -> Result<(), StoreError> {
    let _lock = lock(path)?;
    let counter = match fs::read(path) {
        Ok(bytes) => open_log(key, &bytes)?.0.saturating_add(1),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => 0,
        Err(_) => return Err(StoreError::Io),
    };
    let sealed = seal_log(key, counter, wires)?;
    replace(path, &sealed)
}

/// Write a log file only if it is still the one the caller read.
///
/// `expected` is the [`log_stamp`] seen when the wires were loaded, or `None` when there was no
/// file. If the file has changed since, nothing is written and the result is
/// [`StoreError::Stale`], so a caller can reload and try again instead of overwriting events
/// another writer appended. On success the result is the stamp of the new file.
///
/// An exclusive lock on a sibling `*.lock` file is held from the check to the rename, so two
/// writers cannot pick the same counter, which is the nonce. The bytes go to a sibling `*.part`
/// file, are flushed to disk, and are then renamed over `path`. A reader sees the old file or the
/// new one, and a crash leaves one of them whole.
///
/// # Errors
///
/// [`StoreError::Stale`] when the file changed, [`StoreError::Io`] or [`StoreError::Corrupt`]
/// when it cannot be read or written, and [`StoreError::BadKey`] when the counter would wrap.
pub fn write_log_if(
    path: &Path,
    key: &[u8; 32],
    expected: Option<(u64, u64)>,
    wires: &[Vec<u8>],
) -> Result<(u64, u64), StoreError> {
    let _lock = lock(path)?;
    let current = log_stamp(path)?;
    if current != expected {
        return Err(StoreError::Stale);
    }
    let counter = match current {
        Some((counter, _)) => counter.checked_add(1).ok_or(StoreError::BadKey)?,
        None => 0,
    };
    let sealed = seal_log(key, counter, wires)?;
    replace(path, &sealed)?;
    let len = u64::try_from(sealed.len()).map_err(|_| StoreError::Io)?;
    Ok((counter, len))
}

/// Hold an exclusive lock on `path`'s sibling `*.lock` file until the guard is dropped.
fn lock(path: &Path) -> Result<fs::File, StoreError> {
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path.with_extension("lock"))
        .map_err(|_| StoreError::Io)?;
    file.lock().map_err(|_| StoreError::Io)?;
    Ok(file)
}

/// Put `sealed` at `path` so that a crash leaves the old file or the new one, never a torn one.
fn replace(path: &Path, sealed: &[u8]) -> Result<(), StoreError> {
    let tmp = path.with_extension("part");
    let written = (|| {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(sealed)?;
        file.sync_all()?;
        fs::rename(&tmp, path)
    })();
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
        return Err(StoreError::Io);
    }
    sync_parent(path);
    Ok(())
}

/// Flush the directory entry so the rename itself survives a power cut. Best effort: some
/// filesystems refuse to sync a directory, and the file's own bytes are already on disk.
#[cfg(unix)]
fn sync_parent(path: &Path) {
    let dir = match path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir,
        _ => Path::new("."),
    };
    if let Ok(dir) = fs::File::open(dir) {
        let _ = dir.sync_all();
    }
}

#[cfg(not(unix))]
fn sync_parent(_path: &Path) {}

fn nonce(counter: u64) -> Nonce {
    let mut bytes = [0_u8; 12];
    bytes[4..].copy_from_slice(&counter.to_be_bytes());
    Nonce::from(bytes)
}

fn split_wires(plain: &[u8]) -> Result<Vec<Vec<u8>>, StoreError> {
    let mut wires = Vec::new();
    let mut pos = 0;
    while pos < plain.len() {
        let len_bytes: [u8; 4] = plain
            .get(pos..pos + 4)
            .ok_or(StoreError::Corrupt)?
            .try_into()
            .map_err(|_| StoreError::Corrupt)?;
        pos += 4;
        let len =
            usize::try_from(u32::from_be_bytes(len_bytes)).map_err(|_| StoreError::Corrupt)?;
        let end = pos.checked_add(len).ok_or(StoreError::Corrupt)?;
        wires.push(plain.get(pos..end).ok_or(StoreError::Corrupt)?.to_vec());
        pos = end;
    }
    Ok(wires)
}
