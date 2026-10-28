use std::collections::{BTreeMap, HashMap};

use ambient_events::{Cbor, EventId, decode_cbor, encode_cbor, verify};

use crate::error::IdentityError;
use crate::identity::Identity;

/// An authorized device, active or revoked.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeviceState {
    /// The key may still receive new seals.
    Active {
        /// X25519 key.
        encryption_key: [u8; 32],
        /// Local label.
        label: String,
        /// Inbox mailbox.
        mailbox: [u8; 32],
    },
    /// Stop sealing to this key. `up_to` is the last accepted event, or null.
    Revoked {
        /// X25519 key.
        encryption_key: [u8; 32],
        /// Local label.
        label: String,
        /// Inbox mailbox.
        mailbox: [u8; 32],
        /// Last accepted event from this signer, if it ever wrote one.
        up_to: Option<EventId>,
    },
}

/// The identity log after fork resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogProjection {
    /// Last applied identity event. The next identity event uses this as `prev`.
    pub head: Option<EventId>,
    /// Devices keyed by signing key.
    pub devices: BTreeMap<[u8; 32], DeviceState>,
    /// Fork losers. They were not applied.
    pub forks: Vec<EventId>,
    /// Events whose predecessor is not in this set.
    pub gaps: Vec<EventId>,
}

/// The public halves a new device shows in a QR code.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairOffer {
    /// Version 1.
    pub version: u64,
    /// Device signing key.
    pub signing_key: [u8; 32],
    /// Device encryption key.
    pub encryption_key: [u8; 32],
    /// Inbox mailbox.
    pub mailbox: [u8; 32],
}

struct Item {
    id: EventId,
    event_type: String,
    payload: Vec<u8>,
}

/// Project identity-log wires signed by `identity_key`.
///
/// # Errors
///
/// A wire that does not verify, a signer other than the identity key, a second
/// genesis in the accepted chain, or a payload that does not match its type.
pub fn project_log(
    identity_key: &[u8; 32],
    wires: &[&[u8]],
) -> Result<LogProjection, IdentityError> {
    let mut grouped: HashMap<Option<EventId>, Vec<Item>> = HashMap::new();
    for wire in wires {
        let event = verify(wire).map_err(IdentityError::Event)?;
        if event.signer != *identity_key {
            return Err(IdentityError::Event(
                ambient_events::EventError::BadSignature,
            ));
        }
        grouped.entry(event.prev).or_default().push(Item {
            id: event.id,
            event_type: event.event_type,
            payload: event.payload,
        });
    }
    let mut forks = Vec::new();
    let mut next: HashMap<Option<EventId>, Item> = HashMap::new();
    for (prev, mut items) in grouped {
        items.sort_by_key(|item| item.id);
        let mut items = items.into_iter();
        let Some(winner) = items.next() else {
            continue;
        };
        forks.extend(items.map(|item| item.id));
        next.insert(prev, winner);
    }
    let mut devices = BTreeMap::new();
    let mut head = None;
    let mut cursor = None;
    if let Some(first) = next.remove(&None) {
        if first.event_type != "id.genesis" {
            return Err(IdentityError::BadPayload);
        }
        check_genesis(&first.payload)?;
        head = Some(first.id);
        cursor = Some(first.id);
    } else if !next.is_empty() {
        return Err(IdentityError::BadPayload);
    }
    while let Some(id) = cursor {
        if let Some(child) = next.remove(&Some(id)) {
            if child.event_type == "id.genesis" {
                return Err(IdentityError::BadPayload);
            }
            apply(&mut devices, &child)?;
            head = Some(child.id);
            cursor = Some(child.id);
        } else {
            break;
        }
    }
    let mut gaps = Vec::new();
    for item in next.into_values() {
        gaps.push(item.id);
    }
    gaps.sort_by_key(|id| *id);
    forks.sort_by_key(|id| *id);
    Ok(LogProjection {
        head,
        devices,
        forks,
        gaps,
    })
}

fn apply(devices: &mut BTreeMap<[u8; 32], DeviceState>, item: &Item) -> Result<(), IdentityError> {
    match item.event_type.as_str() {
        "id.device-add" => apply_add(devices, &item.payload),
        "id.inbox" => apply_inbox(devices, &item.payload),
        "id.device-revoke" => apply_revoke(devices, &item.payload),
        _ => Err(IdentityError::Event(ambient_events::EventError::BadType)),
    }
}

fn apply_add(
    devices: &mut BTreeMap<[u8; 32], DeviceState>,
    payload: &[u8],
) -> Result<(), IdentityError> {
    let added = read_add(payload)?;
    if devices.contains_key(&added.signing) {
        return Err(IdentityError::BadPayload);
    }
    devices.insert(
        added.signing,
        DeviceState::Active {
            encryption_key: added.encryption,
            label: added.label,
            mailbox: added.mailbox,
        },
    );
    Ok(())
}

fn apply_inbox(
    devices: &mut BTreeMap<[u8; 32], DeviceState>,
    payload: &[u8],
) -> Result<(), IdentityError> {
    let (signing, mailbox) = read_inbox(payload)?;
    match devices.get_mut(&signing) {
        Some(DeviceState::Active { mailbox: slot, .. }) => {
            *slot = mailbox;
            Ok(())
        }
        _ => Err(IdentityError::BadPayload),
    }
}

fn apply_revoke(
    devices: &mut BTreeMap<[u8; 32], DeviceState>,
    payload: &[u8],
) -> Result<(), IdentityError> {
    let (signing, up_to) = read_revoke(payload)?;
    let Some(DeviceState::Active {
        encryption_key,
        label,
        mailbox,
    }) = devices.remove(&signing)
    else {
        return Err(IdentityError::BadPayload);
    };
    devices.insert(
        signing,
        DeviceState::Revoked {
            encryption_key,
            label,
            mailbox,
            up_to,
        },
    );
    Ok(())
}

/// Whether `event` is `up_to` or an ancestor reached by walking `prev`.
///
/// `prevs` maps an event id to its `prev`. A missing id stops the walk.
/// Null `up_to` keeps nothing.
#[must_use]
pub fn is_retained(
    up_to: Option<EventId>,
    event: EventId,
    prevs: &BTreeMap<EventId, Option<EventId>>,
) -> bool {
    let Some(mut cursor) = up_to else {
        return false;
    };
    let limit = prevs.len().saturating_add(1);
    for _ in 0..limit {
        if cursor == event {
            return true;
        }
        let Some(prev) = prevs.get(&cursor).copied().flatten() else {
            return false;
        };
        cursor = prev;
    }
    false
}

/// Canonical bytes of a pairing QR payload.
///
/// # Errors
///
/// Returns [`IdentityError::Event`] if the map cannot be encoded.
pub fn encode_pair_offer(
    signing_key: &[u8; 32],
    encryption_key: &[u8; 32],
    mailbox: &[u8; 32],
) -> Result<Vec<u8>, IdentityError> {
    let value = Cbor::Map(vec![
        (Cbor::Uint(1), Cbor::Uint(1)),
        (Cbor::Uint(2), Cbor::Bytes(signing_key.to_vec())),
        (Cbor::Uint(3), Cbor::Bytes(encryption_key.to_vec())),
        (Cbor::Uint(4), Cbor::Bytes(mailbox.to_vec())),
    ]);
    encode_cbor(&value).map_err(IdentityError::Event)
}

/// Unpadded base64url of a pairing offer.
#[must_use]
pub fn pair_offer_text(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    let mut index = 0;
    while index + 3 <= bytes.len() {
        let b0 = u32::from(bytes.get(index).copied().unwrap_or(0));
        let b1 = u32::from(bytes.get(index + 1).copied().unwrap_or(0));
        let b2 = u32::from(bytes.get(index + 2).copied().unwrap_or(0));
        let n = (b0 << 16) | (b1 << 8) | b2;
        push_digit(&mut out, TABLE, (n >> 18) & 0x3f);
        push_digit(&mut out, TABLE, (n >> 12) & 0x3f);
        push_digit(&mut out, TABLE, (n >> 6) & 0x3f);
        push_digit(&mut out, TABLE, n & 0x3f);
        index += 3;
    }
    if let Some(b0) = bytes.get(index).copied() {
        let b1 = bytes.get(index + 1).copied();
        let n = (u32::from(b0) << 16) | (u32::from(b1.unwrap_or(0)) << 8);
        push_digit(&mut out, TABLE, (n >> 18) & 0x3f);
        push_digit(&mut out, TABLE, (n >> 12) & 0x3f);
        if b1.is_some() {
            push_digit(&mut out, TABLE, (n >> 6) & 0x3f);
        }
    }
    out
}

fn push_digit(out: &mut String, table: &[u8; 64], value: u32) {
    let index = usize::try_from(value).unwrap_or(0);
    out.push(char::from(table.get(index).copied().unwrap_or(b'A')));
}

/// Decode a pairing offer. Padding and the standard base64 alphabet are rejected.
///
/// # Errors
///
/// Returns [`IdentityError::BadPayload`] when the text or the map is not a version-1 offer.
pub fn decode_pair_offer_text(text: &str) -> Result<PairOffer, IdentityError> {
    let bytes = decode_base64url(text)?;
    decode_pair_offer(&bytes)
}

/// Decode the canonical CBOR of a pairing offer.
///
/// # Errors
///
/// Returns [`IdentityError::BadPayload`] when the map is not a version-1 offer.
pub fn decode_pair_offer(bytes: &[u8]) -> Result<PairOffer, IdentityError> {
    let value = decode_cbor(bytes).map_err(|_| IdentityError::BadPayload)?;
    let map = uint_map(&value)?;
    let version = map_uint(map.get(&1).ok_or(IdentityError::BadPayload)?)?;
    if version != 1 || map.len() != 4 {
        return Err(IdentityError::BadPayload);
    }
    Ok(PairOffer {
        version,
        signing_key: map_bytes32(map.get(&2).ok_or(IdentityError::BadPayload)?)?,
        encryption_key: map_bytes32(map.get(&3).ok_or(IdentityError::BadPayload)?)?,
        mailbox: map_bytes32(map.get(&4).ok_or(IdentityError::BadPayload)?)?,
    })
}

impl Identity {
    /// Sign an identity-log event. `space` is always null.
    ///
    /// # Errors
    ///
    /// Returns [`IdentityError::Event`] when the envelope is rejected.
    pub fn sign_log(
        &self,
        prev: Option<EventId>,
        event_type: &str,
        payload: &[u8],
    ) -> Result<Vec<u8>, IdentityError> {
        let unsigned = ambient_events::encode_unsigned(
            &self.signing_key,
            prev.map(|id| id.0),
            None,
            None,
            event_type,
            payload,
        )
        .map_err(IdentityError::Event)?;
        ambient_events::sign_wire(&unsigned, self.signing_seed()).map_err(IdentityError::Event)
    }
}

/// Payload for `id.device-add`.
///
/// # Errors
///
/// Returns [`IdentityError::BadPayload`] when the label length is not 1..=32.
pub fn device_add_payload(
    signing: &[u8; 32],
    encryption: &[u8; 32],
    label: &str,
    mailbox: &[u8; 32],
) -> Result<Vec<u8>, IdentityError> {
    if label.is_empty() || label.len() > 32 {
        return Err(IdentityError::BadPayload);
    }
    encode_cbor(&Cbor::Map(vec![
        (Cbor::Uint(1), Cbor::Bytes(signing.to_vec())),
        (Cbor::Uint(2), Cbor::Bytes(encryption.to_vec())),
        (Cbor::Uint(3), Cbor::Text(label.to_owned())),
        (Cbor::Uint(4), Cbor::Bytes(mailbox.to_vec())),
    ]))
    .map_err(IdentityError::Event)
}

/// Payload for `id.device-revoke`.
///
/// # Errors
///
/// Returns [`IdentityError::Event`] when encoding fails.
pub fn revoke_payload(
    signing: &[u8; 32],
    up_to: Option<EventId>,
) -> Result<Vec<u8>, IdentityError> {
    let up_to = match up_to {
        Some(id) => Cbor::Bytes(id.0.to_vec()),
        None => Cbor::Null,
    };
    encode_cbor(&Cbor::Map(vec![
        (Cbor::Uint(1), Cbor::Bytes(signing.to_vec())),
        (Cbor::Uint(2), up_to),
    ]))
    .map_err(IdentityError::Event)
}

/// Payload for `id.inbox`.
///
/// # Errors
///
/// Returns [`IdentityError::Event`] when encoding fails.
pub fn inbox_payload(signing: &[u8; 32], mailbox: &[u8; 32]) -> Result<Vec<u8>, IdentityError> {
    encode_cbor(&Cbor::Map(vec![
        (Cbor::Uint(1), Cbor::Bytes(signing.to_vec())),
        (Cbor::Uint(2), Cbor::Bytes(mailbox.to_vec())),
    ]))
    .map_err(IdentityError::Event)
}

fn check_genesis(payload: &[u8]) -> Result<(), IdentityError> {
    let value = decode_cbor(payload).map_err(|_| IdentityError::BadPayload)?;
    let map = uint_map(&value)?;
    if map.len() != 1 {
        return Err(IdentityError::BadPayload);
    }
    match map.get(&1) {
        Some(Cbor::Text(text)) if text == "ambient-era" => Ok(()),
        _ => Err(IdentityError::BadPayload),
    }
}

struct Added {
    signing: [u8; 32],
    encryption: [u8; 32],
    label: String,
    mailbox: [u8; 32],
}

fn read_add(payload: &[u8]) -> Result<Added, IdentityError> {
    let value = decode_cbor(payload).map_err(|_| IdentityError::BadPayload)?;
    let map = uint_map(&value)?;
    if map.len() != 4 {
        return Err(IdentityError::BadPayload);
    }
    let label = match map.get(&3) {
        Some(Cbor::Text(text)) if (1..=32).contains(&text.len()) => text.clone(),
        _ => return Err(IdentityError::BadPayload),
    };
    Ok(Added {
        signing: map_bytes32(map.get(&1).ok_or(IdentityError::BadPayload)?)?,
        encryption: map_bytes32(map.get(&2).ok_or(IdentityError::BadPayload)?)?,
        label,
        mailbox: map_bytes32(map.get(&4).ok_or(IdentityError::BadPayload)?)?,
    })
}

fn read_inbox(payload: &[u8]) -> Result<([u8; 32], [u8; 32]), IdentityError> {
    let map = uint_map(&decode_cbor(payload).map_err(|_| IdentityError::BadPayload)?)?;
    if map.len() != 2 {
        return Err(IdentityError::BadPayload);
    }
    Ok((
        map_bytes32(map.get(&1).ok_or(IdentityError::BadPayload)?)?,
        map_bytes32(map.get(&2).ok_or(IdentityError::BadPayload)?)?,
    ))
}

fn read_revoke(payload: &[u8]) -> Result<([u8; 32], Option<EventId>), IdentityError> {
    let map = uint_map(&decode_cbor(payload).map_err(|_| IdentityError::BadPayload)?)?;
    if map.len() != 2 {
        return Err(IdentityError::BadPayload);
    }
    let signing = map_bytes32(map.get(&1).ok_or(IdentityError::BadPayload)?)?;
    let up_to = match map.get(&2) {
        Some(Cbor::Null) => None,
        Some(Cbor::Bytes(bytes)) => {
            let id =
                <[u8; 32]>::try_from(bytes.as_slice()).map_err(|_| IdentityError::BadPayload)?;
            Some(EventId(id))
        }
        _ => return Err(IdentityError::BadPayload),
    };
    Ok((signing, up_to))
}

fn uint_map(value: &Cbor) -> Result<BTreeMap<u64, Cbor>, IdentityError> {
    let Cbor::Map(pairs) = value else {
        return Err(IdentityError::BadPayload);
    };
    let mut map = BTreeMap::new();
    for (key, val) in pairs {
        let Cbor::Uint(index) = key else {
            return Err(IdentityError::BadPayload);
        };
        if map.insert(*index, val.clone()).is_some() {
            return Err(IdentityError::BadPayload);
        }
    }
    Ok(map)
}

fn map_bytes32(value: &Cbor) -> Result<[u8; 32], IdentityError> {
    let Cbor::Bytes(bytes) = value else {
        return Err(IdentityError::BadPayload);
    };
    <[u8; 32]>::try_from(bytes.as_slice()).map_err(|_| IdentityError::BadPayload)
}

fn map_uint(value: &Cbor) -> Result<u64, IdentityError> {
    match value {
        Cbor::Uint(n) => Ok(*n),
        _ => Err(IdentityError::BadPayload),
    }
}

fn decode_base64url(text: &str) -> Result<Vec<u8>, IdentityError> {
    if text.is_empty() {
        return Err(IdentityError::BadPayload);
    }
    let mut bits = 0_u32;
    let mut nbits = 0;
    let mut out = Vec::new();
    for byte in text.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => return Err(IdentityError::BadPayload),
        };
        bits = (bits << 6) | u32::from(value);
        nbits += 6;
        if nbits >= 8 {
            nbits -= 8;
            out.push(u8::try_from((bits >> nbits) & 0xff).unwrap_or(0));
        }
    }
    Ok(out)
}
