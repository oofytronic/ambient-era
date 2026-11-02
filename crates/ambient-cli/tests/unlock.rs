//! The key file wrap. The passphrase is not the root.

use ambient_cli::{Notebook, set_folder_unlock, unlock_folder};

#[test]
fn passphrase_wraps_and_unlocks() -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!("ambient-unlock-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    Notebook::create(&dir).map_err(|err| err.to_string())?;
    set_folder_unlock(&dir, "correct horse").map_err(|err| err.to_string())?;
    let bytes = std::fs::read(dir.join("key")).map_err(|err| err.to_string())?;
    if bytes.get(4) != Some(&3) {
        return Err("key file was not wrapped".to_owned());
    }
    let child = std::process::Command::new(env!("CARGO_BIN_EXE_ambient"))
        .args(["list", &dir.to_string_lossy()])
        .output()
        .map_err(|err| err.to_string())?;
    if child.status.success() {
        return Err("a new process opened the wrapped folder".to_owned());
    }
    if unlock_folder(&dir, "wrong phrase").is_ok() {
        return Err("wrong passphrase was accepted".to_owned());
    }
    unlock_folder(&dir, "correct horse").map_err(|err| err.to_string())?;
    Notebook::open(&dir).map_err(|err| err.to_string())?;
    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}
