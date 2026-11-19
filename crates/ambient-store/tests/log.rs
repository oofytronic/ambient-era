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

fn scratch(name: &str) -> Result<std::path::PathBuf, String> {
    let dir = std::env::temp_dir().join(format!("ambient-store-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    Ok(dir)
}

#[test]
fn a_write_checks_the_stamp_it_was_given() -> Result<(), String> {
    use ambient_store::{StoreError, log_stamp, write_log_if};
    let key = [9_u8; 32];
    let dir = scratch("stamp")?;
    let path = dir.join("notes");
    let one = vec![b"one".to_vec()];
    let two = vec![b"one".to_vec(), b"two".to_vec()];

    // No file yet: only `None` may create it.
    if write_log_if(&path, &key, Some((0, 99)), &one) != Err(StoreError::Stale) {
        return Err("a stamp for a file that is not there was accepted".to_owned());
    }
    if path.exists() {
        return Err("a refused write left a file".to_owned());
    }
    let first = write_log_if(&path, &key, None, &one).map_err(|err| err.to_string())?;
    if log_stamp(&path) != Ok(Some(first)) || first.0 != 0 {
        return Err("the stamp it returned is not the stamp on disk".to_owned());
    }
    let second = write_log_if(&path, &key, Some(first), &two).map_err(|err| err.to_string())?;
    if second.0 != 1 || log_stamp(&path) != Ok(Some(second)) {
        return Err("the counter did not advance by one".to_owned());
    }
    // A writer that still holds the first stamp must not overwrite the second write.
    if write_log_if(&path, &key, Some(first), &one) != Err(StoreError::Stale) {
        return Err("a stale write was accepted".to_owned());
    }
    if read_log(&path, &key).map_err(|err| err.to_string())? != two {
        return Err("a refused write changed the file".to_owned());
    }
    // Replacing leaves no half-written file behind.
    if path.with_extension("part").exists() {
        return Err("the temporary file was left behind".to_owned());
    }
    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[test]
fn concurrent_writers_lose_no_events() -> Result<(), String> {
    use ambient_store::{StoreError, log_stamp, write_log_if};
    let key = [3_u8; 32];
    let dir = scratch("race")?;
    let path = dir.join("notes");
    write_log_if(&path, &key, None, &[b"genesis".to_vec()]).map_err(|err| err.to_string())?;
    let workers: Vec<_> = (0..6_u8)
        .map(|worker| {
            let path = path.clone();
            std::thread::spawn(move || -> Result<(), String> {
                for round in 0..8_u8 {
                    loop {
                        let stamp = log_stamp(&path).map_err(|err| err.to_string())?;
                        let mut wires = read_log(&path, &key).map_err(|err| err.to_string())?;
                        wires.push(vec![worker, round]);
                        match write_log_if(&path, &key, stamp, &wires) {
                            Ok(_) => break,
                            // Someone else wrote first. Read again and retry.
                            Err(StoreError::Stale) => {}
                            Err(err) => return Err(err.to_string()),
                        }
                    }
                }
                Ok(())
            })
        })
        .collect();
    for worker in workers {
        worker
            .join()
            .map_err(|_| "a writer panicked".to_owned())??;
    }
    let wires = read_log(&path, &key).map_err(|err| err.to_string())?;
    let mut seen = std::collections::BTreeSet::new();
    for wire in &wires {
        seen.insert(wire.clone());
    }
    if wires.len() != 1 + 6 * 8 || seen.len() != wires.len() {
        return Err(format!(
            "expected 49 distinct events, found {}",
            wires.len()
        ));
    }
    // One counter per write, so no two writes shared a nonce.
    let counter = log_stamp(&path)
        .map_err(|err| err.to_string())?
        .map(|stamp| stamp.0);
    if counter != Some(6 * 8) {
        return Err(format!("expected counter 48, found {counter:?}"));
    }
    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[test]
fn a_truncated_file_is_corrupt_not_empty() -> Result<(), String> {
    use ambient_store::{StoreError, log_stamp};
    let dir = scratch("short")?;
    let path = dir.join("notes");
    std::fs::write(&path, b"AMBS\x01\x00").map_err(|err| err.to_string())?;
    if log_stamp(&path) != Err(StoreError::Corrupt) {
        return Err("a header cut short was not corrupt".to_owned());
    }
    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}
