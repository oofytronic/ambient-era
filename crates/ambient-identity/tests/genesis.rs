//! The identity derivation vector.

use std::fs;
use std::path::PathBuf;

use ambient_events::verify;
use ambient_identity::derive;

#[test]
fn genesis_vector_matches_the_spec() -> Result<(), String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/vectors/identity/0001-genesis.json");
    let text = fs::read_to_string(&path).map_err(|err| err.to_string())?;
    let doc: serde_json::Value = serde_json::from_str(&text).map_err(|err| err.to_string())?;
    let root = decode_hex(doc["input"]["root_seed"].as_str().ok_or("missing root")?)?;
    let root: [u8; 32] = root
        .try_into()
        .map_err(|_| "root is not 32 bytes".to_owned())?;
    let identity = derive(&root).map_err(|err| err.to_string())?;
    let expect = &doc["expect"]["ok"];
    expect_hex("identity_seed", identity.signing_seed(), expect)?;
    expect_hex("identity_public_key", &identity.signing_key, expect)?;
    expect_hex("identity_seal_secret", identity.seal_secret(), expect)?;
    expect_hex("identity_seal_public", &identity.seal_key, expect)?;
    expect_hex("recovery_mailbox", &identity.recovery_mailbox, expect)?;
    let wire = identity.genesis_wire().map_err(|err| err.to_string())?;
    if hex(&wire) != expect["wire"].as_str().unwrap_or("") {
        return Err("wire mismatch".to_owned());
    }
    let event = verify(&wire).map_err(|err| err.to_string())?;
    if event.id.to_string() != expect["event_id"].as_str().unwrap_or("") {
        return Err("event id mismatch".to_owned());
    }
    if event.event_type != "id.genesis" {
        return Err("type mismatch".to_owned());
    }
    Ok(())
}

fn expect_hex(name: &str, got: &[u8], expect: &serde_json::Value) -> Result<(), String> {
    let want = expect[name]
        .as_str()
        .ok_or_else(|| format!("{name} missing"))?;
    if hex(got) == want {
        Ok(())
    } else {
        Err(format!("{name} mismatch"))
    }
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
