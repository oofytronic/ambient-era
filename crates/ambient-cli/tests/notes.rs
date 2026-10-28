//! The offline notes commands.

use std::io::Cursor;

use ambient_cli::run;

#[test]
fn create_edit_and_search_offline() -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!("ambient-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.to_string_lossy().to_string();
    run_ok(&["init", &path], "")?;
    let id = run_ok(&["add", &path, "Market"], "buy milk")?;
    let listed = run_ok(&["list", &path], "")?;
    if !listed.contains(id.trim()) || !listed.contains("Market") {
        return Err(format!("list missed the note: {listed}"));
    }
    let found = run_ok(&["search", &path, "milk"], "")?;
    if !found.contains(id.trim()) {
        return Err("search missed the body".to_owned());
    }
    run_ok(&["edit", &path, id.trim(), "Market"], "buy oats")?;
    let shown = run_ok(&["show", &path, id.trim()], "")?;
    if shown.trim() != "buy oats" {
        return Err(format!("show got {shown}"));
    }
    let notes = std::fs::read(dir.join("notes")).map_err(|err| err.to_string())?;
    if notes
        .windows(b"buy oats".len())
        .any(|window| window == b"buy oats")
    {
        return Err("note body was written in the clear".to_owned());
    }
    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

fn run_ok(args: &[&str], stdin: &str) -> Result<String, String> {
    let mut input = Cursor::new(stdin.as_bytes());
    let mut output = Vec::new();
    run(
        &mut args.iter().map(|arg| (*arg).to_owned()),
        &mut input,
        &mut output,
    )
    .map_err(|err| err.to_string())?;
    String::from_utf8(output).map_err(|err| err.to_string())
}
