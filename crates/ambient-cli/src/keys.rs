use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError};

use ambient_identity::{DeviceKeys, Identity, derive, device_from_seeds};
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
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
    if let Some(inner) = cached_inner(&key_path) {
        return parse_key(dir, &inner);
    }
    let bytes = fs::read(&key_path).map_err(|_| CliError::message("missing key file"))?;
    if bytes.get(4) == Some(&3) {
        return Err(CliError::message("this folder is locked"));
    }
    parse_key(dir, &bytes)
}

fn parse_key(dir: &Path, bytes: &[u8]) -> Result<Local, CliError> {
    if bytes.get(..4) != Some(b"AMBK") {
        return Err(CliError::message("key file is not a notebook key"));
    }
    match bytes.get(4).copied() {
        Some(1) => open_v1(dir, bytes),
        Some(2) => open_v2(dir, bytes),
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
    let bytes = inner_bytes(&local.key_path)?;
    if bytes.get(4) != Some(&2) {
        return Err(CliError::message(
            "only a paired device learns the identity",
        ));
    }
    let sign = copy32(&bytes, 38)?;
    let enc = copy32(&bytes, 70)?;
    let mailbox = copy32(&bytes, 102)?;
    let store = copy32(&bytes, 134)?;
    store_inner(
        &local.key_path,
        v2_bytes(true, &public, &sign, &enc, &mailbox, &store, &local.label)?,
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

/// The BIP39 recovery phrase for this root seed. 24 words.
pub(crate) fn recovery_phrase(root: &[u8; 32]) -> Result<String, CliError> {
    let mnemonic = bip39::Mnemonic::from_entropy(root)
        .map_err(|_| CliError::message("could not encode recovery phrase"))?;
    Ok(mnemonic.to_string())
}

/// Recover the root seed from a BIP39 phrase.
pub(crate) fn root_from_phrase(phrase: &str) -> Result<[u8; 32], CliError> {
    let mnemonic = bip39::Mnemonic::parse(phrase)
        .map_err(|_| CliError::message("recovery phrase rejected"))?;
    let entropy = mnemonic.to_entropy();
    if entropy.len() != 32 {
        return Err(CliError::message("recovery phrase must be 24 words"));
    }
    let mut root = [0_u8; 32];
    root.copy_from_slice(&entropy);
    Ok(root)
}

pub(crate) fn store_key_from_root(root: &[u8; 32]) -> [u8; 32] {
    store_key(root)
}

pub(crate) fn v1_bytes_for_restore(
    root: &[u8; 32],
    sign: &[u8; 32],
    enc: &[u8; 32],
    mailbox: &[u8; 32],
    label: &str,
) -> Result<Vec<u8>, CliError> {
    v1_bytes(root, sign, enc, mailbox, label)
}

pub(crate) fn write_secret_pub(path: &std::path::Path, bytes: &[u8]) -> Result<(), CliError> {
    write_secret(path, bytes)
}

pub(crate) fn copy32_pub(bytes: &[u8], start: usize) -> Result<[u8; 32], CliError> {
    copy32(bytes, start)
}

const UNLOCK_AAD: &[u8] = b"ambient-unlock/v1";

struct Cached {
    wrap: [u8; 32],
    inner: Vec<u8>,
}

impl Drop for Cached {
    fn drop(&mut self) {
        self.wrap.fill(0);
        self.inner.fill(0);
    }
}

fn cache() -> &'static Mutex<HashMap<PathBuf, Cached>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Cached>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cache_key(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn cached_inner(path: &Path) -> Option<Vec<u8>> {
    let guard = cache().lock().unwrap_or_else(PoisonError::into_inner);
    guard.get(&cache_key(path)).map(|item| item.inner.clone())
}

fn cached_wrap(path: &Path) -> Option<[u8; 32]> {
    let guard = cache().lock().unwrap_or_else(PoisonError::into_inner);
    guard.get(&cache_key(path)).map(|item| item.wrap)
}

fn cache_put(path: &Path, wrap: [u8; 32], inner: Vec<u8>) {
    let mut guard = cache().lock().unwrap_or_else(PoisonError::into_inner);
    guard.insert(cache_key(path), Cached { wrap, inner });
}

fn inner_bytes(path: &Path) -> Result<Vec<u8>, CliError> {
    if let Some(inner) = cached_inner(path) {
        return Ok(inner);
    }
    let bytes = fs::read(path).map_err(|_| CliError::message("missing key file"))?;
    if bytes.get(4) == Some(&3) {
        return Err(CliError::message("this folder is locked"));
    }
    Ok(bytes)
}

fn store_inner(path: &Path, inner: Vec<u8>) -> Result<(), CliError> {
    let on_disk = fs::read(path).ok();
    if on_disk.as_ref().and_then(|bytes| bytes.get(4).copied()) == Some(3) {
        let wrap = cached_wrap(path).ok_or_else(|| CliError::message("this folder is locked"))?;
        let bytes = on_disk.ok_or_else(|| CliError::message("missing key file"))?;
        let salt = copy_array::<16>(&bytes, 5)?;
        let nonce = random_n::<12>()?;
        let sealed = seal_inner(&wrap, &nonce, &inner)?;
        write_secret(path, &wrapped_bytes(&salt, &nonce, &sealed))?;
        cache_put(path, wrap, inner);
        return Ok(());
    }
    write_secret(path, &inner)
}

/// Wrap the key file with a passphrase. The passphrase is not the root.
///
/// # Errors
///
/// Returns [`CliError`] when the passphrase is too short or the folder is locked
/// and has not been unlocked in this process.
pub(crate) fn set_unlock(dir: &Path, passphrase: &str) -> Result<(), CliError> {
    let passphrase = check_passphrase(passphrase)?;
    let key_path = dir.join("key");
    let inner = inner_bytes(&key_path)?;
    let salt = random_n::<16>()?;
    let wrap = derive_wrap(passphrase, &salt)?;
    let nonce = random_n::<12>()?;
    let sealed = seal_inner(&wrap, &nonce, &inner)?;
    write_secret(&key_path, &wrapped_bytes(&salt, &nonce, &sealed))?;
    cache_put(&key_path, wrap, inner);
    Ok(())
}

/// Unwrap a locked key file for this process. Does not write the root to disk.
///
/// # Errors
///
/// Returns [`CliError`] when the passphrase is wrong or the folder is not locked.
pub(crate) fn unlock(dir: &Path, passphrase: &str) -> Result<(), CliError> {
    let passphrase = check_passphrase(passphrase)?;
    let key_path = dir.join("key");
    let bytes = fs::read(&key_path).map_err(|_| CliError::message("missing key file"))?;
    if bytes.get(4) != Some(&3) {
        return Err(CliError::message("this folder is not locked"));
    }
    let salt = copy_array::<16>(&bytes, 5)?;
    let nonce = copy_array::<12>(&bytes, 21)?;
    let sealed = bytes
        .get(33..)
        .ok_or_else(|| CliError::message("key file truncated"))?;
    let wrap = derive_wrap(passphrase, &salt)?;
    let inner = open_inner(&wrap, &nonce, sealed)?;
    parse_key(dir, &inner)?;
    cache_put(&key_path, wrap, inner);
    Ok(())
}

fn check_passphrase(passphrase: &str) -> Result<&str, CliError> {
    let passphrase = passphrase.trim();
    if passphrase.chars().count() < 8 {
        return Err(CliError::message(
            "passphrase must be at least 8 characters",
        ));
    }
    Ok(passphrase)
}

fn derive_wrap(passphrase: &str, salt: &[u8]) -> Result<[u8; 32], CliError> {
    let params = Params::new(19_456, 2, 1, Some(32))
        .map_err(|_| CliError::message("could not start unlock"))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut out = [0_u8; 32];
    argon
        .hash_password_into(passphrase.as_bytes(), salt, &mut out)
        .map_err(|_| CliError::message("could not derive unlock key"))?;
    Ok(out)
}

fn seal_inner(wrap: &[u8; 32], nonce_bytes: &[u8; 12], inner: &[u8]) -> Result<Vec<u8>, CliError> {
    let cipher = ChaCha20Poly1305::new(wrap.into());
    cipher
        .encrypt(
            &Nonce::from(*nonce_bytes),
            Payload {
                msg: inner,
                aad: UNLOCK_AAD,
            },
        )
        .map_err(|_| CliError::message("could not wrap key file"))
}

fn open_inner(wrap: &[u8; 32], nonce_bytes: &[u8; 12], sealed: &[u8]) -> Result<Vec<u8>, CliError> {
    let cipher = ChaCha20Poly1305::new(wrap.into());
    cipher
        .decrypt(
            &Nonce::from(*nonce_bytes),
            Payload {
                msg: sealed,
                aad: UNLOCK_AAD,
            },
        )
        .map_err(|_| CliError::message("passphrase rejected"))
}

fn wrapped_bytes(salt: &[u8; 16], nonce: &[u8; 12], sealed: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(33 + sealed.len());
    out.extend_from_slice(b"AMBK");
    out.push(3);
    out.extend_from_slice(salt);
    out.extend_from_slice(nonce);
    out.extend_from_slice(sealed);
    out
}

fn random_n<const N: usize>() -> Result<[u8; N], CliError> {
    let mut bytes = [0_u8; N];
    getrandom::getrandom(&mut bytes).map_err(|_| CliError::message("random source failed"))?;
    Ok(bytes)
}

fn copy_array<const N: usize>(bytes: &[u8], start: usize) -> Result<[u8; N], CliError> {
    let end = start + N;
    let slice = bytes
        .get(start..end)
        .ok_or_else(|| CliError::message("key file truncated"))?;
    let mut out = [0_u8; N];
    out.copy_from_slice(slice);
    Ok(out)
}
