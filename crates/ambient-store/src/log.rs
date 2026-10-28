use std::fs;
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

/// Write a log file, incrementing the counter when the file already exists.
///
/// # Errors
///
/// Returns [`StoreError::Io`] or [`StoreError::Corrupt`] from the previous file.
pub fn write_log(path: &Path, key: &[u8; 32], wires: &[Vec<u8>]) -> Result<(), StoreError> {
    let counter = match fs::read(path) {
        Ok(bytes) => open_log(key, &bytes)?.0.saturating_add(1),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => 0,
        Err(_) => return Err(StoreError::Io),
    };
    let sealed = seal_log(key, counter, wires)?;
    fs::write(path, sealed).map_err(|_| StoreError::Io)
}

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
