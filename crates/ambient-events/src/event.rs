use ed25519_dalek::{Signature, VerifyingKey};

use crate::cbor::{self, Parsed, Value};
use crate::error::EventError;

/// BLAKE3-256 of the canonical unsigned event.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct EventId(pub [u8; 32]);

impl EventId {
    /// The raw 32 bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Display for EventId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// A wire event whose signature and shape have been checked.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedEvent {
    /// BLAKE3 of the canonical unsigned bytes.
    pub id: EventId,
    /// Ed25519 public key that signed the event. Field 2.
    pub signer: [u8; 32],
    /// Previous event id from this signer, if this is not their first.
    pub prev: Option<EventId>,
    /// Advisory unix milliseconds. Not an order.
    pub unix_ms: Option<u64>,
    /// Space id, or `None` for an identity event or `space.genesis`.
    pub space: Option<EventId>,
    /// Type name, such as `id.genesis`.
    pub event_type: String,
    /// Canonical CBOR payload, still opaque to this crate.
    pub payload: Vec<u8>,
}

impl VerifiedEvent {
    /// Canonical unsigned-event bytes. Hashing them yields [`Self::id`].
    ///
    /// # Errors
    ///
    /// Returns [`EventError::BadPayload`] if `payload` is not one canonical CBOR value.
    pub fn encode_unsigned(&self) -> Result<Vec<u8>, EventError> {
        encode_unsigned(
            &self.signer,
            self.prev.map(|id| id.0),
            self.unix_ms,
            self.space.map(|id| id.0),
            &self.event_type,
            &self.payload,
        )
    }
}

/// Encode an unsigned event. The result is what a signature covers.
///
/// # Errors
///
/// Returns a schema error if the type or the payload is not acceptable.
pub fn encode_unsigned(
    signer: &[u8; 32],
    prev: Option<[u8; 32]>,
    unix_ms: Option<u64>,
    space: Option<[u8; 32]>,
    event_type: &str,
    payload: &[u8],
) -> Result<Vec<u8>, EventError> {
    check_type(event_type)?;
    check_payload(payload)?;
    let mut out = Vec::new();
    out.push(0xa7);
    push_uint(&mut out, 1);
    push_uint(&mut out, 1);
    push_uint(&mut out, 2);
    push_bstr(&mut out, signer);
    push_uint(&mut out, 3);
    push_id_or_null(&mut out, prev);
    push_uint(&mut out, 4);
    match unix_ms {
        Some(ms) => push_uint(&mut out, ms),
        None => out.push(0xf6),
    }
    push_uint(&mut out, 5);
    push_id_or_null(&mut out, space);
    push_uint(&mut out, 6);
    push_tstr(&mut out, event_type);
    push_uint(&mut out, 7);
    push_bstr(&mut out, payload);
    Ok(out)
}

/// Verify a wire event: canonical CBOR, then the Ed25519 signature.
///
/// # Errors
///
/// See [`EventError`]. Encoding failures are reported before the signature.
#[must_use = "a failed verify is the difference between accepting and dropping an event"]
pub fn verify(bytes: &[u8]) -> Result<VerifiedEvent, EventError> {
    let parsed = cbor::parse_exact(bytes)?;
    let Value::Array(items) = parsed.value else {
        return Err(EventError::BadPayload);
    };
    let mut items = items.into_iter();
    let Some(unsigned) = items.next() else {
        return Err(EventError::BadPayload);
    };
    let Some(signature) = items.next() else {
        return Err(EventError::BadPayload);
    };
    if items.next().is_some() {
        return Err(EventError::BadPayload);
    }
    let Value::Bytes(sig_bytes) = signature.value else {
        return Err(EventError::BadPayload);
    };
    let Ok(sig_array) = <[u8; 64]>::try_from(sig_bytes) else {
        return Err(EventError::BadPayload);
    };
    let event = interpret_unsigned(&unsigned)?;
    let signature = Signature::from_bytes(&sig_array);
    let key = VerifyingKey::from_bytes(&event.signer).map_err(|_| EventError::BadSignature)?;
    key.verify_strict(unsigned.raw, &signature)
        .map_err(|_| EventError::BadSignature)?;
    let id = EventId(*blake3::hash(unsigned.raw).as_bytes());
    Ok(VerifiedEvent {
        id,
        signer: event.signer,
        prev: event.prev,
        unix_ms: event.unix_ms,
        space: event.space,
        event_type: event.event_type,
        payload: event.payload,
    })
}

struct Unsigned {
    signer: [u8; 32],
    prev: Option<EventId>,
    unix_ms: Option<u64>,
    space: Option<EventId>,
    event_type: String,
    payload: Vec<u8>,
}

fn interpret_unsigned(parsed: &Parsed<'_>) -> Result<Unsigned, EventError> {
    let Value::Map(pairs) = &parsed.value else {
        return Err(EventError::BadPayload);
    };
    let mut fields: [Option<&Parsed<'_>>; 8] = [None, None, None, None, None, None, None, None];
    for (key, value) in pairs {
        let Value::Uint(index) = key.value else {
            return Err(EventError::BadPayload);
        };
        let Some(slot) = fields.get_mut(usize::try_from(index).unwrap_or(0)) else {
            return Err(EventError::BadPayload);
        };
        if !(1..8).contains(&index) || slot.is_some() {
            return Err(EventError::BadPayload);
        }
        *slot = Some(value);
    }
    if fields.iter().skip(1).any(Option::is_none) {
        return Err(EventError::BadPayload);
    }
    let version = uint_field(field(&fields, 1)?)?;
    if version != 1 {
        return Err(EventError::BadVersion);
    }
    let signer = bytes32(field(&fields, 2)?)?;
    let prev = id_or_null(field(&fields, 3)?)?;
    let unix_ms = unix_field(field(&fields, 4)?)?;
    let space = id_or_null(field(&fields, 5)?)?;
    let event_type = text_field(field(&fields, 6)?)?;
    check_type(&event_type)?;
    let payload_bytes = bytes_field(field(&fields, 7)?)?;
    check_payload(payload_bytes)?;
    Ok(Unsigned {
        signer,
        prev,
        unix_ms,
        space,
        event_type,
        payload: payload_bytes.to_vec(),
    })
}

fn field<'a>(
    fields: &[Option<&'a Parsed<'a>>; 8],
    index: usize,
) -> Result<&'a Parsed<'a>, EventError> {
    fields
        .get(index)
        .and_then(|slot| *slot)
        .ok_or(EventError::BadPayload)
}

fn uint_field(parsed: &Parsed<'_>) -> Result<u64, EventError> {
    match parsed.value {
        Value::Uint(value) => Ok(value),
        _ => Err(EventError::BadPayload),
    }
}

fn bytes_field<'a>(parsed: &'a Parsed<'_>) -> Result<&'a [u8], EventError> {
    match parsed.value {
        Value::Bytes(bytes) => Ok(bytes),
        _ => Err(EventError::BadPayload),
    }
}

fn bytes32(parsed: &Parsed<'_>) -> Result<[u8; 32], EventError> {
    <[u8; 32]>::try_from(bytes_field(parsed)?).map_err(|_| EventError::BadPayload)
}

fn text_field(parsed: &Parsed<'_>) -> Result<String, EventError> {
    match &parsed.value {
        Value::Text(text) => Ok((*text).to_owned()),
        _ => Err(EventError::BadPayload),
    }
}

fn id_or_null(parsed: &Parsed<'_>) -> Result<Option<EventId>, EventError> {
    match parsed.value {
        Value::Null => Ok(None),
        Value::Bytes(bytes) => {
            let id = <[u8; 32]>::try_from(bytes).map_err(|_| EventError::BadPayload)?;
            Ok(Some(EventId(id)))
        }
        _ => Err(EventError::BadPayload),
    }
}

fn unix_field(parsed: &Parsed<'_>) -> Result<Option<u64>, EventError> {
    match parsed.value {
        Value::Null => Ok(None),
        Value::Uint(value) => Ok(Some(value)),
        _ => Err(EventError::BadPayload),
    }
}

fn check_type(event_type: &str) -> Result<(), EventError> {
    if event_type.len() > 64 || event_type.is_empty() {
        return Err(EventError::BadPayload);
    }
    let allowed = event_type
        .bytes()
        .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-'));
    if !allowed {
        return Err(EventError::BadType);
    }
    let known = matches!(
        event_type,
        "id.genesis"
            | "id.device-add"
            | "id.device-revoke"
            | "id.inbox"
            | "space.genesis"
            | "space.member-add"
            | "space.member-remove"
            | "space.inbox"
            | "note.put"
            | "msg.put"
    );
    if known {
        Ok(())
    } else {
        Err(EventError::BadType)
    }
}

fn check_payload(payload: &[u8]) -> Result<(), EventError> {
    cbor::parse_exact(payload)
        .map(|_| ())
        .map_err(|_| EventError::BadPayload)
}

fn push_uint(out: &mut Vec<u8>, value: u64) {
    push_head(out, 0, value);
}

fn push_bstr(out: &mut Vec<u8>, bytes: &[u8]) {
    let Ok(len) = u64::try_from(bytes.len()) else {
        return;
    };
    push_head(out, 2, len);
    out.extend_from_slice(bytes);
}

fn push_tstr(out: &mut Vec<u8>, text: &str) {
    let Ok(len) = u64::try_from(text.len()) else {
        return;
    };
    push_head(out, 3, len);
    out.extend_from_slice(text.as_bytes());
}

fn push_id_or_null(out: &mut Vec<u8>, id: Option<[u8; 32]>) {
    match id {
        Some(bytes) => push_bstr(out, &bytes),
        None => out.push(0xf6),
    }
}

fn push_head(out: &mut Vec<u8>, major: u8, len: u64) {
    let class = major << 5;
    if len < 24 {
        out.push(class | fold_u8(len));
    } else if len < 256 {
        out.push(class | 0x18);
        out.push(fold_u8(len));
    } else if len < 65_536 {
        out.push(class | 0x19);
        out.extend_from_slice(&fold_u16(len).to_be_bytes());
    } else if len < 65_536_u64 * 65_536 {
        out.push(class | 0x1a);
        out.extend_from_slice(&fold_u32(len).to_be_bytes());
    } else {
        out.push(class | 0x1b);
        out.extend_from_slice(&len.to_be_bytes());
    }
}

fn fold_u8(value: u64) -> u8 {
    u8::try_from(value).unwrap_or(0)
}

fn fold_u16(value: u64) -> u16 {
    u16::try_from(value).unwrap_or(0)
}

fn fold_u32(value: u64) -> u32 {
    u32::try_from(value).unwrap_or(0)
}
