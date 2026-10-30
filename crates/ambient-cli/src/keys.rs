use std::fs;
use std::path::{Path, PathBuf};

use ambient_identity::{DeviceKeys, Identity, derive, device_from_seeds};
use hkdf::Hkdf;
use sha2::Sha256;

use crate::notes::CliError;

pub(crate) struct Local {
    pub(crate) identity: Option<Identity>,
    pub(crate) identity_public: [u8; 32],
    pub(crate) device: DeviceKeys,
    pub(crate) store_key: [u8; 32],
    pub(crate) notes_path: PathBuf,
    pub(crate) key_path: PathBuf,
    pub(crate) label: String,
}

pub(crate) fn open(dir: &Path) -> Result<Local, CliError> {
    let key_path = dir.join("key");
    let bytes = fs::read(&key_path).map_err(|_| CliError::message("missing key file"))?;
    if bytes.get(..4) != Some(b"AMBK") {
        return Err(CliError::message("key file is not a notebook key"));
    }
    match bytes.get(4).copied() {
        Some(1) => open_v1(dir, &bytes),
        Some(2) => open_v2(dir, &bytes),
        _ => Err(CliError::message("key file is not a known version")),
    }
}

fn open_v1(dir: &Path, bytes: &[u8]) -> Result<Local, CliError> {
    if bytes.len() < 134 {
        return Err(CliError::message("key file truncated"));
    }
    let root = copy32(bytes, 5)?;
    let sign = copy32(bytes, 37)?;
    let enc = copy32(bytes, 69)?;
    let mailbox = copy32(bytes, 101)?;
    let (label, _) = read_label(bytes, 133)?;
    let identity = derive(&root).map_err(|_| CliError::message("derivation failed"))?;
    let identity_public = identity.signing_key;
    let device = device_from_seeds(&sign, &enc, &mailbox, &label)
        .map_err(|_| CliError::message("device key file rejected"))?;
    Ok(Local {
        identity: Some(identity),
        identity_public,
        device,
        store_key: store_key(&root),
        notes_path: dir.join("notes"),
        key_path: dir.join("key"),
        label,
    })
}

fn open_v2(dir: &Path, bytes: &[u8]) -> Result<Local, CliError> {
    // version, known flag, identity public, three device seeds, store key, label.
    if bytes.len() < 5 + 1 + 32 * 5 + 1 {
        return Err(CliError::message("key file truncated"));
    }
    let known = bytes
        .get(5)
        .copied()
        .ok_or_else(|| CliError::message("key file truncated"))?;
    let identity_public = copy32(bytes, 6)?;
    let sign = copy32(bytes, 38)?;
    let enc = copy32(bytes, 70)?;
    let mailbox = copy32(bytes, 102)?;
    let store = copy32(bytes, 134)?;
    let (label, _) = read_label(bytes, 166)?;
    if known > 1 {
        return Err(CliError::message("key file is not a known version"));
    }
    let device = device_from_seeds(&sign, &enc, &mailbox, &label)
        .map_err(|_| CliError::message("device key file rejected"))?;
    Ok(Local {
        identity: None,
        identity_public: if known == 1 { identity_public } else { [0; 32] },
        device,
        store_key: store,
        notes_path: dir.join("notes"),
        key_path: dir.join("key"),
        label,
    })
}

fn read_label(bytes: &[u8], at: usize) -> Result<(String, usize), CliError> {
    let label_len = usize::from(
        *bytes
            .get(at)
            .ok_or_else(|| CliError::message("key file truncated"))?,
    );
    let label_bytes = bytes
        .get(at + 1..at + 1 + label_len)
        .ok_or_else(|| CliError::message("key file truncated"))?;
    let label =
        std::str::from_utf8(label_bytes).map_err(|_| CliError::message("label is not utf-8"))?;
    Ok((label.to_owned(), at + 1 + label_len))
}

pub(crate) fn create(dir: &Path, label: &str) -> Result<Local, CliError> {
    fs::create_dir_all(dir).map_err(|_| CliError::message("could not create directory"))?;
    let key_path = dir.join("key");
    if key_path.exists() {
        return Err(CliError::message("key file already exists"));
    }
    let root = random32()?;
    let sign = random32()?;
    let enc = random32()?;
    let mailbox = random32()?;
    let identity = derive(&root).map_err(|_| CliError::message("derivation failed"))?;
    let device = device_from_seeds(&sign, &enc, &mailbox, label)
        .map_err(|_| CliError::message("label must be 1 to 32 bytes"))?;
    write_secret(&key_path, &v1_bytes(&root, &sign, &enc, &mailbox, label)?)?;
    Ok(Local {
        identity_public: identity.signing_key,
        identity: Some(identity),
        device,
        store_key: store_key(&root),
        notes_path: dir.join("notes"),
        key_path,
        label: label.to_owned(),
    })
}

/// A second device. It does not hold the root. The offer is what the owner pairs.
pub(crate) fn create_device(dir: &Path, label: &str) -> Result<(Local, String), CliError> {
    fs::create_dir_all(dir).map_err(|_| CliError::message("could not create directory"))?;
    let key_path = dir.join("key");
    if key_path.exists() {
        return Err(CliError::message("key file already exists"));
    }
    let sign = random32()?;
    let enc = random32()?;
    let mailbox = random32()?;
    let store = random32()?;
    let device = device_from_seeds(&sign, &enc, &mailbox, label)
        .map_err(|_| CliError::message("label must be 1 to 32 bytes"))?;
    write_secret(
        &key_path,
        &v2_bytes(false, &[0; 32], &sign, &enc, &mailbox, &store, label)?,
    )?;
    let offer = ambient_identity::encode_pair_offer(
        &device.signing_key,
        &device.encryption_key,
        &device.mailbox,
    )
    .map_err(|_| CliError::message("could not encode offer"))?;
    Ok((
        Local {
            identity: None,
            identity_public: [0; 32],
            device,
            store_key: store,
            notes_path: dir.join("notes"),
            key_path,
            label: label.to_owned(),
        },
        ambient_identity::pair_offer_text(&offer),
    ))
}

pub(crate) fn remember_identity(local: &mut Local, public: [u8; 32]) -> Result<(), CliError> {
    if local.identity.is_some() || local.identity_public == public {
        local.identity_public = public;
        return Ok(());
    }
    let bytes = fs::read(&local.key_path).map_err(|_| CliError::message("missing key file"))?;
    if bytes.get(4) != Some(&2) {
        return Err(CliError::message(
            "only a paired device learns the identity",
        ));
    }
    let sign = copy32(&bytes, 38)?;
    let enc = copy32(&bytes, 70)?;
    let mailbox = copy32(&bytes, 102)?;
    let store = copy32(&bytes, 134)?;
    write_secret(
        &local.key_path,
        &v2_bytes(true, &public, &sign, &enc, &mailbox, &store, &local.label)?,
    )?;
    local.identity_public = public;
    Ok(())
}

fn v1_bytes(
    root: &[u8; 32],
    sign: &[u8; 32],
    enc: &[u8; 32],
    mailbox: &[u8; 32],
    label: &str,
) -> Result<Vec<u8>, CliError> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"AMBK");
    bytes.push(1);
    bytes.extend_from_slice(root);
    bytes.extend_from_slice(sign);
    bytes.extend_from_slice(enc);
    bytes.extend_from_slice(mailbox);
    push_label(&mut bytes, label)?;
    Ok(bytes)
}

fn v2_bytes(
    known: bool,
    identity: &[u8; 32],
    sign: &[u8; 32],
    enc: &[u8; 32],
    mailbox: &[u8; 32],
    store: &[u8; 32],
    label: &str,
) -> Result<Vec<u8>, CliError> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"AMBK");
    bytes.push(2);
    bytes.push(u8::from(known));
    bytes.extend_from_slice(identity);
    bytes.extend_from_slice(sign);
    bytes.extend_from_slice(enc);
    bytes.extend_from_slice(mailbox);
    bytes.extend_from_slice(store);
    push_label(&mut bytes, label)?;
    Ok(bytes)
}

fn push_label(bytes: &mut Vec<u8>, label: &str) -> Result<(), CliError> {
    let label_len =
        u8::try_from(label.len()).map_err(|_| CliError::message("label must be 1 to 32 bytes"))?;
    bytes.push(label_len);
    bytes.extend_from_slice(label.as_bytes());
    Ok(())
}

fn write_secret(path: &Path, bytes: &[u8]) -> Result<(), CliError> {
    fs::write(path, bytes).map_err(|_| CliError::message("could not write key file"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn store_key(root: &[u8; 32]) -> [u8; 32] {
    let hkdf = Hkdf::<Sha256>::new(Some(b"ambient-era/v1"), root);
    let mut out = [0_u8; 32];
    let _ = hkdf.expand(b"local-store", &mut out);
    out
}

fn random32() -> Result<[u8; 32], CliError> {
    let mut bytes = [0_u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|_| CliError::message("random source failed"))?;
    Ok(bytes)
}

fn copy32(bytes: &[u8], start: usize) -> Result<[u8; 32], CliError> {
    let end = start + 32;
    let slice = bytes
        .get(start..end)
        .ok_or_else(|| CliError::message("key file truncated"))?;
    let mut out = [0_u8; 32];
    out.copy_from_slice(slice);
    Ok(out)
}
