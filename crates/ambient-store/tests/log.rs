//! The encrypted local log.

use ambient_store::{open_log, read_log, seal_log, write_log};

#[test]
fn seal_roundtrip_and_rejects_a_bad_key() -> Result<(), String> {
    let key = [7_u8; 32];
    let wires = vec![b"one".to_vec(), b"two".to_vec()];
    let sealed = seal_log(&key, 0, &wires).map_err(|err| err.to_string())?;
    let (counter, opened) = open_log(&key, &sealed).map_err(|err| err.to_string())?;
    if counter != 0 || opened != wires {
        return Err("roundtrip mismatch".to_owned());
    }
    let mut wrong = key;
    wrong[0] = 8;
    if open_log(&wrong, &sealed).is_ok() {
        return Err("wrong key opened the log".to_owned());
    }
    let dir = std::env::temp_dir().join(format!("ambient-store-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    let path = dir.join("notes");
    write_log(&path, &key, &wires).map_err(|err| err.to_string())?;
    write_log(&path, &key, &wires).map_err(|err| err.to_string())?;
    let loaded = read_log(&path, &key).map_err(|err| err.to_string())?;
    if loaded != wires {
        return Err("file roundtrip mismatch".to_owned());
    }
    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}
