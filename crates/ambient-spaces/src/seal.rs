//! HPKE base-mode seals for a private Space. See `docs/spec/03-spaces.md`.

use ambient_events::{Cbor, decode_cbor, encode_cbor};
use hpke::aead::ChaCha20Poly1305;
use hpke::kdf::HkdfSha256;
use hpke::kem::X25519HkdfSha256;
use hpke::rand_core::{TryCryptoRng, TryRng};
use hpke::{
    Deserializable, Kem as KemTrait, OpModeR, OpModeS, Serializable, setup_receiver,
    setup_sender_with_rng,
};

use crate::error::SpaceError;

type Kem = X25519HkdfSha256;
type Aead = ChaCha20Poly1305;
type Kdf = HkdfSha256;

const INFO: &[u8] = b"ambient-era/hpke/v1";
const AAD_PREFIX: &[u8] = b"ambient-era/aad/v1";
const HINT_PREFIX: &[u8] = b"ambient-era/hint/v1";

struct IkmRng<'a> {
    rest: &'a [u8],
}

impl TryRng for IkmRng<'_> {
    type Error = core::convert::Infallible;

    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        let mut bytes = [0_u8; 4];
        let _ = self.try_fill_bytes(&mut bytes);
        Ok(u32::from_le_bytes(bytes))
    }

    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        let mut bytes = [0_u8; 8];
        let _ = self.try_fill_bytes(&mut bytes);
        Ok(u64::from_le_bytes(bytes))
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Self::Error> {
        for slot in dest.iter_mut() {
            let (byte, rest) = split_first(self.rest);
            *slot = byte;
            self.rest = rest;
        }
        Ok(())
    }
}

impl TryCryptoRng for IkmRng<'_> {}

fn split_first(bytes: &[u8]) -> (u8, &[u8]) {
    match bytes.split_first() {
        Some((first, rest)) => (*first, rest),
        None => (0, &[]),
    }
}

fn hint(mailbox: &[u8; 32], recipient: &[u8; 32]) -> [u8; 8] {
    let mut input = Vec::with_capacity(HINT_PREFIX.len().saturating_add(64));
    input.extend_from_slice(HINT_PREFIX);
    input.extend_from_slice(mailbox);
    input.extend_from_slice(recipient);
    let digest = blake3::hash(&input);
    let mut out = [0_u8; 8];
    for (index, slot) in out.iter_mut().enumerate() {
        *slot = digest.as_bytes().get(index).copied().unwrap_or(0);
    }
    out
}

fn aad(mailbox: &[u8; 32], recipient: &[u8; 32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(AAD_PREFIX.len().saturating_add(64));
    out.extend_from_slice(AAD_PREFIX);
    out.extend_from_slice(mailbox);
    out.extend_from_slice(recipient);
    out
}

fn copy32(bytes: &[u8]) -> Result<[u8; 32], SpaceError> {
    let mut out = [0_u8; 32];
    if bytes.len() != 32 {
        return Err(SpaceError::BadPayload);
    }
    out.copy_from_slice(bytes);
    Ok(out)
}

/// Seal `plaintext` to one recipient. `ikm` is 32 fresh bytes. HPKE derives the ephemeral key.
///
/// One box, for one mailbox. The caller puts the blob in that mailbox.
///
/// # Errors
///
/// Returns [`SpaceError::Seal`] when the recipient key is rejected or the seal fails.
pub fn seal_blob(
    recipient_public: &[u8; 32],
    mailbox: &[u8; 32],
    plaintext: &[u8],
    ikm: &[u8; 32],
) -> Result<Vec<u8>, SpaceError> {
    let public =
        <Kem as KemTrait>::PublicKey::from_bytes(recipient_public).map_err(|_| SpaceError::Seal)?;
    let mut rng = IkmRng { rest: ikm };
    let (enc, mut ctx) =
        setup_sender_with_rng::<Aead, Kdf, Kem>(&OpModeS::Base, &public, INFO, &mut rng)
            .map_err(|_| SpaceError::Seal)?;
    let ciphertext = ctx
        .seal(plaintext, &aad(mailbox, recipient_public))
        .map_err(|_| SpaceError::Seal)?;
    let enc_bytes = enc.to_bytes();
    let hint = hint(mailbox, recipient_public);
    encode_cbor(&Cbor::Map(vec![
        (Cbor::Uint(1), Cbor::Uint(1)),
        (
            Cbor::Uint(2),
            Cbor::Array(vec![Cbor::Map(vec![
                (Cbor::Uint(1), Cbor::Bytes(hint.to_vec())),
                (Cbor::Uint(2), Cbor::Bytes(enc_bytes.to_vec())),
                (Cbor::Uint(3), Cbor::Bytes(ciphertext)),
            ])]),
        ),
    ]))
    .map_err(SpaceError::Event)
}

/// Open a blob sealed to `recipient_secret`. A hint miss is not a successful open.
///
/// # Errors
///
/// Returns [`SpaceError::Seal`] when no box opens. A malformed blob is [`SpaceError::BadPayload`].
pub fn open_blob(
    recipient_secret: &[u8; 32],
    mailbox: &[u8; 32],
    blob: &[u8],
) -> Result<Vec<u8>, SpaceError> {
    let secret = <Kem as KemTrait>::PrivateKey::from_bytes(recipient_secret)
        .map_err(|_| SpaceError::Seal)?;
    let public = <Kem as KemTrait>::sk_to_pk(&secret).to_bytes();
    let recipient = copy32(public.as_slice())?;
    let expected = hint(mailbox, &recipient);
    let associated = aad(mailbox, &recipient);
    let value = decode_cbor(blob).map_err(|_| SpaceError::BadPayload)?;
    let Cbor::Map(pairs) = value else {
        return Err(SpaceError::BadPayload);
    };
    let mut version = None;
    let mut boxes = None;
    for (key, val) in pairs {
        match key {
            Cbor::Uint(1) => version = Some(val),
            Cbor::Uint(2) => boxes = Some(val),
            _ => return Err(SpaceError::BadPayload),
        }
    }
    let Some(Cbor::Uint(1)) = version else {
        return Err(SpaceError::BadPayload);
    };
    let Some(Cbor::Array(boxes)) = boxes else {
        return Err(SpaceError::BadPayload);
    };
    for item in boxes {
        let Cbor::Map(fields) = item else {
            return Err(SpaceError::BadPayload);
        };
        let mut found_hint = None;
        let mut enc = None;
        let mut ciphertext = None;
        for (key, val) in fields {
            match key {
                Cbor::Uint(1) => found_hint = Some(val),
                Cbor::Uint(2) => enc = Some(val),
                Cbor::Uint(3) => ciphertext = Some(val),
                _ => return Err(SpaceError::BadPayload),
            }
        }
        let Some(Cbor::Bytes(found_hint)) = found_hint else {
            return Err(SpaceError::BadPayload);
        };
        if found_hint.as_slice() != expected {
            continue;
        }
        let Some(Cbor::Bytes(enc)) = enc else {
            return Err(SpaceError::BadPayload);
        };
        let Some(Cbor::Bytes(ciphertext)) = ciphertext else {
            return Err(SpaceError::BadPayload);
        };
        let encapped =
            <Kem as KemTrait>::EncappedKey::from_bytes(&enc).map_err(|_| SpaceError::Seal)?;
        let mut ctx = setup_receiver::<Aead, Kdf, Kem>(&OpModeR::Base, &secret, &encapped, INFO)
            .map_err(|_| SpaceError::Seal)?;
        if let Ok(plain) = ctx.open(&ciphertext, &associated) {
            return Ok(plain);
        }
    }
    Err(SpaceError::Seal)
}
