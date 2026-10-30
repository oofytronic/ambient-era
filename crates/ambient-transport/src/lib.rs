//! The relay's newline-delimited JSON, without a socket.
//!
//! Spec: `docs/spec/04-relay-api.md`. A mailbox id is 32 opaque bytes.
//! Binary fields are unpadded base64url. This crate does not open blobs.

use serde_json::{Map, Value};

/// A stored blob and the cursor the relay assigned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Item {
    /// Positive cursor. Not a Space order.
    pub cursor: u64,
    /// Opaque bytes. The relay does not parse them.
    pub blob: Vec<u8>,
}

/// One client request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Request {
    /// Store one blob.
    Put {
        /// Mailbox id.
        mailbox: [u8; 32],
        /// Opaque blob.
        blob: Vec<u8>,
    },
    /// Blobs with cursor greater than `since`.
    Get {
        /// Mailbox id.
        mailbox: [u8; 32],
        /// Exclusive lower bound.
        since: u64,
        /// Caller limit. The relay caps it.
        limit: u64,
    },
    /// Drop blobs at or below `up_to`.
    Delete {
        /// Mailbox id.
        mailbox: [u8; 32],
        /// Inclusive upper bound.
        up_to: u64,
    },
    /// Ask for future blobs. A client that only polls is still conforming.
    Subscribe {
        /// Mailbox id.
        mailbox: [u8; 32],
        /// Exclusive lower bound for the first batch.
        since: u64,
    },
}

/// One relay response.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Response {
    /// Put, or delete.
    Ok {
        /// Present on put. Absent on delete.
        cursor: Option<u64>,
    },
    /// A get or a subscribe batch.
    Blobs {
        /// Cursor order.
        items: Vec<Item>,
    },
    /// `bad_request` or `too_large`.
    Err {
        /// Spec error code.
        code: String,
    },
}

/// Encode one request as a single JSON line without the newline.
#[must_use]
pub fn encode_request(request: &Request) -> String {
    let value = match request {
        Request::Put { mailbox, blob } => serde_json::json!({
            "op": "put",
            "mailbox": b64(mailbox),
            "blob": b64(blob),
        }),
        Request::Get {
            mailbox,
            since,
            limit,
        } => serde_json::json!({
            "op": "get",
            "mailbox": b64(mailbox),
            "since": since,
            "limit": limit,
        }),
        Request::Delete { mailbox, up_to } => serde_json::json!({
            "op": "delete",
            "mailbox": b64(mailbox),
            "up_to": up_to,
        }),
        Request::Subscribe { mailbox, since } => serde_json::json!({
            "op": "subscribe",
            "mailbox": b64(mailbox),
            "since": since,
        }),
    };
    value.to_string()
}

/// Encode one response as a single JSON line without the newline.
#[must_use]
pub fn encode_response(response: &Response) -> String {
    let value = match response {
        Response::Ok {
            cursor: Some(cursor),
        } => serde_json::json!({ "op": "ok", "cursor": cursor }),
        Response::Ok { cursor: None } => serde_json::json!({ "op": "ok" }),
        Response::Blobs { items } => {
            let items: Vec<Value> = items
                .iter()
                .map(|item| serde_json::json!({ "cursor": item.cursor, "blob": b64(&item.blob) }))
                .collect();
            serde_json::json!({ "op": "blobs", "items": items })
        }
        Response::Err { code } => serde_json::json!({ "op": "err", "code": code }),
    };
    value.to_string()
}

/// Decode one request line.
///
/// # Errors
///
/// Returns `None` when the line is not a known request. The relay answers `bad_request`.
#[must_use]
pub fn decode_request(line: &str) -> Option<Request> {
    let value: Value = serde_json::from_str(line).ok()?;
    let map = value.as_object()?;
    match text(map, "op")? {
        "put" => Some(Request::Put {
            mailbox: mailbox(map)?,
            blob: bytes(map, "blob")?,
        }),
        "get" => Some(Request::Get {
            mailbox: mailbox(map)?,
            since: number(map, "since").unwrap_or(0),
            limit: number(map, "limit").unwrap_or(100),
        }),
        "delete" => Some(Request::Delete {
            mailbox: mailbox(map)?,
            up_to: number(map, "up_to")?,
        }),
        "subscribe" => Some(Request::Subscribe {
            mailbox: mailbox(map)?,
            since: number(map, "since").unwrap_or(0),
        }),
        _ => None,
    }
}

/// Decode one response line.
///
/// # Errors
///
/// Returns `None` when the line is not a response this client understands.
#[must_use]
pub fn decode_response(line: &str) -> Option<Response> {
    let value: Value = serde_json::from_str(line).ok()?;
    let map = value.as_object()?;
    match text(map, "op")? {
        "ok" => Some(Response::Ok {
            cursor: number(map, "cursor"),
        }),
        "err" => Some(Response::Err {
            code: text(map, "code")?.to_owned(),
        }),
        "blobs" => {
            let items = map.get("items")?.as_array()?;
            let mut out = Vec::new();
            for item in items {
                let item = item.as_object()?;
                out.push(Item {
                    cursor: number(item, "cursor")?,
                    blob: bytes(item, "blob")?,
                });
            }
            Some(Response::Blobs { items: out })
        }
        _ => None,
    }
}

fn text<'a>(map: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    map.get(key)?.as_str()
}

fn number(map: &Map<String, Value>, key: &str) -> Option<u64> {
    map.get(key)?.as_u64()
}

fn mailbox(map: &Map<String, Value>) -> Option<[u8; 32]> {
    let bytes = bytes(map, "mailbox")?;
    if bytes.len() != 32 {
        return None;
    }
    let mut out = [0_u8; 32];
    out.copy_from_slice(&bytes);
    Some(out)
}

fn bytes(map: &Map<String, Value>, key: &str) -> Option<Vec<u8>> {
    b64_decode(text(map, key)?)
}

fn b64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    let mut index = 0;
    while index + 3 <= bytes.len() {
        let b0 = u32::from(bytes.get(index).copied().unwrap_or(0));
        let b1 = u32::from(bytes.get(index + 1).copied().unwrap_or(0));
        let b2 = u32::from(bytes.get(index + 2).copied().unwrap_or(0));
        let n = (b0 << 16) | (b1 << 8) | b2;
        push(&mut out, TABLE, (n >> 18) & 0x3f);
        push(&mut out, TABLE, (n >> 12) & 0x3f);
        push(&mut out, TABLE, (n >> 6) & 0x3f);
        push(&mut out, TABLE, n & 0x3f);
        index += 3;
    }
    let left = bytes.len().saturating_sub(index);
    if left == 1 {
        let b0 = u32::from(bytes.get(index).copied().unwrap_or(0));
        let n = b0 << 16;
        push(&mut out, TABLE, (n >> 18) & 0x3f);
        push(&mut out, TABLE, (n >> 12) & 0x3f);
    } else if left == 2 {
        let b0 = u32::from(bytes.get(index).copied().unwrap_or(0));
        let b1 = u32::from(bytes.get(index + 1).copied().unwrap_or(0));
        let n = (b0 << 16) | (b1 << 8);
        push(&mut out, TABLE, (n >> 18) & 0x3f);
        push(&mut out, TABLE, (n >> 12) & 0x3f);
        push(&mut out, TABLE, (n >> 6) & 0x3f);
    }
    out
}

fn push(out: &mut String, table: &[u8; 64], value: u32) {
    let index = usize::try_from(value).unwrap_or(0);
    out.push(char::from(table.get(index).copied().unwrap_or(b'A')));
}

fn b64_decode(text: &str) -> Option<Vec<u8>> {
    if text
        .bytes()
        .any(|byte| byte == b'=' || byte == b'+' || byte == b'/')
    {
        return None;
    }
    let mut out = Vec::new();
    let mut buf = 0_u32;
    let mut bits = 0;
    for byte in text.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => return None,
        };
        buf = (buf << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            let shift = u32::try_from(bits).unwrap_or(0);
            out.push(u8::try_from((buf >> shift) & 0xff).unwrap_or(0));
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::{
        Item, Request, Response, decode_request, decode_response, encode_request, encode_response,
    };

    #[test]
    fn request_roundtrip() {
        let request = Request::Put {
            mailbox: [7; 32],
            blob: b"cipher".to_vec(),
        };
        let line = encode_request(&request);
        assert_eq!(decode_request(&line), Some(request));
    }

    #[test]
    fn blobs_roundtrip() {
        let response = Response::Blobs {
            items: vec![Item {
                cursor: 3,
                blob: vec![1, 2, 3],
            }],
        };
        let line = encode_response(&response);
        assert_eq!(decode_response(&line), Some(response));
    }
}
