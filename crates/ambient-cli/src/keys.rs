use std::fs;
use std::path::{Path, PathBuf};

use ambient_identity::{DeviceKeys, Identity, derive, device_from_seeds};
use hkdf::Hkdf;
use sha2::Sha256;

use crate::notes::CliError;

pub(crate) struct Local {
    pub(crate) identity: Identity,
    pub(crate) device: DeviceKeys,
    pub(crate) store_key: [u8; 32],
    pub(crate) notes_path: PathBuf,
}

pub(crate) fn open(dir: &Path) -> Result<Local, CliError> {
    let key_path = dir.join("key");
    let bytes = fs::read(&key_path).map_err(|_| CliError::message("missing key file"))?;
    if bytes.len() < 4 + 1 + 32 * 4 || bytes.get(..4) != Some(b"AMBK") || bytes.get(4) != Some(&1) {
        return Err(CliError::message("key file is not version 1"));
    }
    let root = copy32(&bytes, 5)?;
    let sign = copy32(&bytes, 37)?;
    let enc = copy32(&bytes, 69)?;
    let mailbox = copy32(&bytes, 101)?;
    let label_len = usize::from(
        *bytes
            .get(133)
            .ok_or_else(|| CliError::message("key file truncated"))?,
    );
    let label_bytes = bytes
        .get(134..134 + label_len)
        .ok_or_else(|| CliError::message("key file truncated"))?;
    let label =
        std::str::from_utf8(label_bytes).map_err(|_| CliError::message("label is not utf-8"))?;
    let identity = derive(&root).map_err(|_| CliError::message("derivation failed"))?;
    let device = device_from_seeds(&sign, &enc, &mailbox, label)
        .map_err(|_| CliError::message("device key file rejected"))?;
    Ok(Local {
        identity,
        device,
        store_key: store_key(&root),
        notes_path: dir.join("notes"),
    })
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
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"AMBK");
    bytes.push(1);
    bytes.extend_from_slice(&root);
    bytes.extend_from_slice(&sign);
    bytes.extend_from_slice(&enc);
    bytes.extend_from_slice(&mailbox);
    let label_len =
        u8::try_from(label.len()).map_err(|_| CliError::message("label must be 1 to 32 bytes"))?;
    bytes.push(label_len);
    bytes.extend_from_slice(label.as_bytes());
    fs::write(&key_path, &bytes).map_err(|_| CliError::message("could not write key file"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600));
    }
    Ok(Local {
        identity,
        device,
        store_key: store_key(&root),
        notes_path: dir.join("notes"),
    })
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
