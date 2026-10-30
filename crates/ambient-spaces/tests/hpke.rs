//! The published HPKE blob must open to `hello`.

use ambient_spaces::open_blob;

#[test]
fn vector_opens() -> Result<(), String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/vectors/spaces/0001-hpke-blob.json");
    let text = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|err| err.to_string())?;
    let secret = hex32(field(&value, &["input", "recipient_secret"])?)?;
    let mailbox = hex32(field(&value, &["input", "mailbox"])?)?;
    let blob = hex_bytes(field(&value, &["expect", "ok", "blob"])?)?;
    let plain = open_blob(&secret, &mailbox, &blob).map_err(|err| err.to_string())?;
    if plain != b"hello" {
        return Err(format!("opened {}", String::from_utf8_lossy(&plain)));
    }
    Ok(())
}

fn field<'a>(value: &'a serde_json::Value, path: &[&str]) -> Result<&'a str, String> {
    let mut cursor = value;
    for key in path {
        cursor = cursor.get(*key).ok_or_else(|| format!("missing {key}"))?;
    }
    cursor.as_str().ok_or_else(|| "not a string".to_owned())
}

fn hex32(text: &str) -> Result<[u8; 32], String> {
    let bytes = hex_bytes(text)?;
    let mut out = [0_u8; 32];
    if bytes.len() != 32 {
        return Err(format!("expected 32 bytes, got {}", bytes.len()));
    }
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn hex_bytes(text: &str) -> Result<Vec<u8>, String> {
    if !text.len().is_multiple_of(2) {
        return Err("odd hex".to_owned());
    }
    let mut out = Vec::new();
    let mut index = 0;
    while index < text.len() {
        let pair = text.get(index..index + 2).ok_or_else(|| "hex".to_owned())?;
        out.push(u8::from_str_radix(pair, 16).map_err(|err| err.to_string())?);
        index += 2;
    }
    Ok(out)
}
