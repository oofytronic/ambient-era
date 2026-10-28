use ambient_events::{encode_unsigned, sign_wire, signing_public};
use hkdf::Hkdf;
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroize;

use crate::error::IdentityError;

const SALT: &[u8] = b"ambient-era/v1";

/// Keys derived from a root seed. Secret fields are wiped on drop and omitted from [`Debug`].
pub struct Identity {
    signing_seed: [u8; 32],
    seal_secret: [u8; 32],
    /// Ed25519 public key. This is the signer of identity-log events.
    pub signing_key: [u8; 32],
    /// X25519 public key of the recovery seal.
    pub seal_key: [u8; 32],
    /// Mailbox id for the recovery copy.
    pub recovery_mailbox: [u8; 32],
}

impl Identity {
    /// The Ed25519 seed. Treat it as the root: do not log it.
    #[must_use]
    pub const fn signing_seed(&self) -> &[u8; 32] {
        &self.signing_seed
    }

    /// The raw X25519 secret from HKDF, before clamping. Treat it as the root.
    #[must_use]
    pub const fn seal_secret(&self) -> &[u8; 32] {
        &self.seal_secret
    }

    /// The signed `id.genesis` wire event.
    ///
    /// # Errors
    ///
    /// Returns [`IdentityError::Event`] if the envelope cannot be signed.
    pub fn genesis_wire(&self) -> Result<Vec<u8>, IdentityError> {
        let unsigned = encode_unsigned(
            &self.signing_key,
            None,
            None,
            None,
            "id.genesis",
            &genesis_payload(),
        )
        .map_err(IdentityError::Event)?;
        sign_wire(&unsigned, &self.signing_seed).map_err(IdentityError::Event)
    }
}

impl Drop for Identity {
    fn drop(&mut self) {
        self.signing_seed.zeroize();
        self.seal_secret.zeroize();
    }
}

impl std::fmt::Debug for Identity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Identity")
            .field("signing_key", &self.signing_key)
            .field("seal_key", &self.seal_key)
            .field("recovery_mailbox", &self.recovery_mailbox)
            .finish_non_exhaustive()
    }
}

/// Derive the identity signing key, the recovery seal key, and the recovery mailbox.
///
/// # Errors
///
/// Returns [`IdentityError::Derivation`] if HKDF rejects a 32-byte output.
pub fn derive(root: &[u8; 32]) -> Result<Identity, IdentityError> {
    let signing_seed = hkdf32(root, b"identity-sign")?;
    let seal_secret = hkdf32(root, b"identity-seal")?;
    let recovery_mailbox = hkdf32(root, b"recovery-mailbox")?;
    let signing_key = signing_public(&signing_seed);
    let seal_key = PublicKey::from(&StaticSecret::from(seal_secret)).to_bytes();
    Ok(Identity {
        signing_seed,
        seal_secret,
        signing_key,
        seal_key,
        recovery_mailbox,
    })
}

fn hkdf32(ikm: &[u8], info: &[u8]) -> Result<[u8; 32], IdentityError> {
    let hkdf = Hkdf::<Sha256>::new(Some(SALT), ikm);
    let mut out = [0_u8; 32];
    hkdf.expand(info, &mut out)
        .map_err(|_| IdentityError::Derivation)?;
    Ok(out)
}

fn genesis_payload() -> Vec<u8> {
    let text = b"ambient-era";
    let mut out = Vec::with_capacity(3 + text.len());
    out.push(0xa1);
    out.push(0x01);
    out.push(0x6b);
    out.extend_from_slice(text);
    out
}
