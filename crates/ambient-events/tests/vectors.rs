//! Conformance vectors for the event envelope.

use std::fs;
use std::path::PathBuf;

use ambient_events::{EventError, verify};

#[test]
fn event_vectors_match_the_spec() -> Result<(), String> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/vectors/events");
    let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
        .map_err(|err| err.to_string())?
        .filter_map(|entry| entry.ok().map(|item| item.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();
    if paths.is_empty() {
        return Err(format!("no vectors in {}", dir.display()));
    }
    for path in paths {
        let text = fs::read_to_string(&path).map_err(|err| err.to_string())?;
        let doc: serde_json::Value = serde_json::from_str(&text).map_err(|err| err.to_string())?;
        let bytes = decode_hex(
            doc["input"]["bytes"]
                .as_str()
                .ok_or("vector missing input.bytes")?,
        )?;
        let result = verify(&bytes);
        if let Some(code) = doc["expect"]["error"].as_str() {
            let got = result
                .err()
                .ok_or_else(|| format!("{} was accepted", path.display()))?;
            if got.code() != code {
                return Err(format!(
                    "{}: got {}, want {code}",
                    path.display(),
                    got.code()
                ));
            }
            continue;
        }
        let event = result.map_err(|err| format!("{}: {err}", path.display()))?;
        let expect = &doc["expect"]["ok"];
        if event.id.to_string() != expect["id"].as_str().unwrap_or("") {
            return Err(format!("{}: id mismatch", path.display()));
        }
        if hex(&event.signer) != expect["signer"].as_str().unwrap_or("") {
            return Err(format!("{}: signer mismatch", path.display()));
        }
        if event.event_type != expect["type"].as_str().unwrap_or("") {
            return Err(format!("{}: type mismatch", path.display()));
        }
        let encoded = event
            .encode_unsigned()
            .map_err(|err: EventError| err.to_string())?;
        if blake3::hash(&encoded).to_hex().as_str() != event.id.to_string() {
            return Err(format!("{}: re-encode changed the id", path.display()));
        }
    }
    Ok(())
}

fn decode_hex(text: &str) -> Result<Vec<u8>, String> {
    if !text.len().is_multiple_of(2) {
        return Err("odd hex".to_owned());
    }
    (0..text.len())
        .step_by(2)
        .map(|index| {
            let pair = text.get(index..index + 2).ok_or("hex")?;
            u8::from_str_radix(pair, 16).map_err(|err| err.to_string())
        })
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let hi = usize::from(byte >> 4);
        let lo = usize::from(byte & 0x0f);
        out.push(char::from(HEX.get(hi).copied().unwrap_or(b'0')));
        out.push(char::from(HEX.get(lo).copied().unwrap_or(b'0')));
    }
    out
}
