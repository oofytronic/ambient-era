//! A decoder for the CBOR subset in the event spec.
//!
//! It rejects anything that is not already in canonical form, so a later
//! re-encode cannot disagree with the bytes that were signed.

use crate::error::EventError;

pub(crate) enum Value<'a> {
    Uint(u64),
    Bytes(&'a [u8]),
    Text(&'a str),
    Null,
    Array(Vec<Parsed<'a>>),
    Map(Vec<(Parsed<'a>, Parsed<'a>)>),
}

pub(crate) struct Parsed<'a> {
    pub raw: &'a [u8],
    pub value: Value<'a>,
}

pub(crate) fn parse_exact(input: &[u8]) -> Result<Parsed<'_>, EventError> {
    if input.is_empty() {
        return Err(EventError::Truncated);
    }
    let mut cursor = Cursor { input, pos: 0 };
    let parsed = parse_value(&mut cursor)?;
    if cursor.pos != input.len() {
        return Err(EventError::Noncanonical);
    }
    Ok(parsed)
}

struct Cursor<'a> {
    input: &'a [u8],
    pos: usize,
}

fn parse_value<'a>(cursor: &mut Cursor<'a>) -> Result<Parsed<'a>, EventError> {
    let start = cursor.pos;
    let (major, argument, width) = read_header(cursor)?;
    let value = match major {
        0 => Value::Uint(argument),
        2 => Value::Bytes(read_slice(cursor, argument)?),
        3 => {
            let bytes = read_slice(cursor, argument)?;
            let text = std::str::from_utf8(bytes).map_err(|_| EventError::Noncanonical)?;
            Value::Text(text)
        }
        4 => parse_array(cursor, argument)?,
        5 => parse_map(cursor, argument)?,
        7 => parse_simple(argument, width)?,
        _ => return Err(EventError::Noncanonical),
    };
    let raw = cursor
        .input
        .get(start..cursor.pos)
        .ok_or(EventError::Truncated)?;
    Ok(Parsed { raw, value })
}

fn parse_array<'a>(cursor: &mut Cursor<'a>, len: u64) -> Result<Value<'a>, EventError> {
    if len > remaining(cursor)? {
        return Err(EventError::Truncated);
    }
    let mut items = Vec::new();
    for _ in 0..len {
        items.push(parse_value(cursor)?);
    }
    Ok(Value::Array(items))
}

fn parse_map<'a>(cursor: &mut Cursor<'a>, pairs: u64) -> Result<Value<'a>, EventError> {
    if pairs > remaining(cursor)? {
        return Err(EventError::Truncated);
    }
    let mut items = Vec::new();
    let mut previous: Option<(usize, usize)> = None;
    for _ in 0..pairs {
        let key_start = cursor.pos;
        let key = parse_value(cursor)?;
        let key_end = cursor.pos;
        if let Some((start, end)) = previous {
            let prev = cursor.input.get(start..end).ok_or(EventError::Truncated)?;
            let current = cursor
                .input
                .get(key_start..key_end)
                .ok_or(EventError::Truncated)?;
            if current <= prev {
                return Err(EventError::Noncanonical);
            }
        }
        previous = Some((key_start, key_end));
        let value = parse_value(cursor)?;
        items.push((key, value));
    }
    Ok(Value::Map(items))
}

fn parse_simple(argument: u64, width: u8) -> Result<Value<'static>, EventError> {
    // Only null, and only in its one-byte form (major 7, additional info 22).
    if width == 0 && argument == 22 {
        Ok(Value::Null)
    } else {
        Err(EventError::Noncanonical)
    }
}

fn remaining(cursor: &Cursor<'_>) -> Result<u64, EventError> {
    let left = cursor.input.len().saturating_sub(cursor.pos);
    u64::try_from(left).map_err(|_| EventError::Truncated)
}

/// Reads a CBOR head. `width` is the additional-info class that carried the argument.
fn read_header(cursor: &mut Cursor<'_>) -> Result<(u8, u64, u8), EventError> {
    let first = read_byte(cursor)?;
    let major = first >> 5;
    let additional = first & 0x1f;
    if additional == 31 || (28..31).contains(&additional) {
        return Err(EventError::Noncanonical);
    }
    let (argument, width) = read_argument(cursor, additional)?;
    Ok((major, argument, width))
}

fn read_argument(cursor: &mut Cursor<'_>, additional: u8) -> Result<(u64, u8), EventError> {
    if additional < 24 {
        return Ok((u64::from(additional), 0));
    }
    let (value, width) = match additional {
        24 => (u64::from(read_byte(cursor)?), 24),
        25 => (read_be::<2>(cursor)?, 25),
        26 => (read_be::<4>(cursor)?, 26),
        27 => (read_be::<8>(cursor)?, 27),
        _ => return Err(EventError::Noncanonical),
    };
    if !is_minimal(value, width) {
        return Err(EventError::Noncanonical);
    }
    Ok((value, width))
}

fn is_minimal(value: u64, width: u8) -> bool {
    match width {
        24 => value >= 24,
        25 => value >= 256,
        26 => value >= 65_536,
        27 => value >= 65_536_u64 * 65_536,
        _ => false,
    }
}

fn read_byte(cursor: &mut Cursor<'_>) -> Result<u8, EventError> {
    let byte = *cursor.input.get(cursor.pos).ok_or(EventError::Truncated)?;
    cursor.pos += 1;
    Ok(byte)
}

fn read_be<const N: usize>(cursor: &mut Cursor<'_>) -> Result<u64, EventError> {
    let end = cursor.pos.checked_add(N).ok_or(EventError::Truncated)?;
    let slice = cursor
        .input
        .get(cursor.pos..end)
        .ok_or(EventError::Truncated)?;
    cursor.pos = end;
    let mut bytes = [0_u8; 8];
    let dest = bytes.get_mut(8 - N..).ok_or(EventError::Truncated)?;
    dest.copy_from_slice(slice);
    Ok(u64::from_be_bytes(bytes))
}

fn read_slice<'a>(cursor: &mut Cursor<'a>, len: u64) -> Result<&'a [u8], EventError> {
    let len = usize::try_from(len).map_err(|_| EventError::Truncated)?;
    let end = cursor.pos.checked_add(len).ok_or(EventError::Truncated)?;
    let slice = cursor
        .input
        .get(cursor.pos..end)
        .ok_or(EventError::Truncated)?;
    cursor.pos = end;
    Ok(slice)
}
