use ambient_events::signing_public;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroize;

/// A device or contact keypair generated on this machine. Not derived from the root.
pub struct DeviceKeys {
    signing_seed: [u8; 32],
    encryption_secret: [u8; 32],
    /// Ed25519 public key.
    pub signing_key: [u8; 32],
    /// X25519 public key.
    pub encryption_key: [u8; 32],
    /// Inbox mailbox id.
    pub mailbox: [u8; 32],
    /// Local label. Never sent to a relay as an identity.
    pub label: String,
}

impl DeviceKeys {
    /// Ed25519 seed. Do not log it.
    #[must_use]
    pub const fn signing_seed(&self) -> &[u8; 32] {
        &self.signing_seed
    }

    /// Raw X25519 secret, before clamping. Do not log it.
    #[must_use]
    pub const fn encryption_secret(&self) -> &[u8; 32] {
        &self.encryption_secret
    }
}

impl Drop for DeviceKeys {
    fn drop(&mut self) {
        self.signing_seed.zeroize();
        self.encryption_secret.zeroize();
    }
}

impl std::fmt::Debug for DeviceKeys {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DeviceKeys")
            .field("signing_key", &self.signing_key)
            .field("encryption_key", &self.encryption_key)
            .field("mailbox", &self.mailbox)
            .field("label", &self.label)
            .finish_non_exhaustive()
    }
}

/// Build a device from caller-supplied seeds. This crate does not read the OS random source.
///
/// # Errors
///
/// Returns [`crate::IdentityError::BadPayload`] when the label is empty or longer than 32 bytes.
pub fn device_from_seeds(
    signing_seed: &[u8; 32],
    encryption_secret: &[u8; 32],
    mailbox: &[u8; 32],
    label: &str,
) -> Result<DeviceKeys, crate::IdentityError> {
    if label.is_empty() || label.len() > 32 {
        return Err(crate::IdentityError::BadPayload);
    }
    let signing_key = signing_public(signing_seed);
    let encryption_key = PublicKey::from(&StaticSecret::from(*encryption_secret)).to_bytes();
    Ok(DeviceKeys {
        signing_seed: *signing_seed,
        encryption_secret: *encryption_secret,
        signing_key,
        encryption_key,
        mailbox: *mailbox,
        label: label.to_owned(),
    })
}
