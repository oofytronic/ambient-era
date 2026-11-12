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
    let index = std::fs::read(dir.join("search.db")).map_err(|err| err.to_string())?;
    if index.windows(b"milk".len()).any(|window| window == b"milk") {
        return Err("search index stored the note in the clear".to_owned());
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

#[test]
fn a_deleted_snapshot_leaves_the_list() -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!("ambient-cli-del-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.to_string_lossy().to_string();
    run_ok(&["init", &path], "")?;
    let id = run_ok(&["add", &path, "Market"], "buy milk")?;
    let keep = run_ok(&["add", &path, "Keep"], "still here")?;
    run_ok(&["edit", &path, id.trim(), ""], ambient_cli::DELETED_BODY)?;
    let listed = run_ok(&["list", &path], "")?;
    if listed.contains(id.trim()) || listed.contains("Market") {
        return Err(format!("deleted note still listed: {listed}"));
    }
    if !listed.contains(keep.trim()) || !listed.contains("Keep") {
        return Err(format!("the other note left the list: {listed}"));
    }
    let found = run_ok(&["search", &path, "ambient"], "")?;
    if found.contains(id.trim()) {
        return Err(format!("search still returned the deleted note: {found}"));
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

#[test]
fn a_tag_in_the_body_files_the_note() -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!("ambient-cli-tags-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let book = ambient_cli::Notebook::create(&dir).map_err(|err| err.to_string())?;
    book.add(
        "Hours",
        "# Clinic\n\nWaiting #clinic\nAlso #clinic and #follow-up\n`#nope`\n```\n#hidden\n```\n",
    )
    .map_err(|err| err.to_string())?;
    book.add("Plain", "no tag").map_err(|err| err.to_string())?;
    let notes = book.list().map_err(|err| err.to_string())?;
    let hours = notes
        .iter()
        .find(|note| note.title == "Hours")
        .ok_or("missing Hours")?;
    if hours.tags != ["clinic", "follow-up"] {
        return Err(format!("tags were {:?}", hours.tags));
    }
    let plain = notes
        .iter()
        .find(|note| note.title == "Plain")
        .ok_or("missing Plain")?;
    if !plain.tags.is_empty() {
        return Err(format!("plain note was filed as {:?}", plain.tags));
    }
    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

#[test]
fn warm_search_reuses_the_index() -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!("ambient-cli-warm-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let book = ambient_cli::Notebook::create(&dir).map_err(|err| err.to_string())?;
    let body = format!("alpha note #work\n{}", "word ".repeat(80));
    for index in 0..24 {
        book.add(&format!("Note {index}"), &body)
            .map_err(|err| err.to_string())?;
    }
    let started = std::time::Instant::now();
    for _ in 0..20 {
        let found = book.search("word").map_err(|err| err.to_string())?;
        if found.len() != 24 {
            return Err(format!("search returned {}", found.len()));
        }
    }
    let elapsed = started.elapsed();
    if elapsed > std::time::Duration::from_millis(1500) {
        return Err(format!("warm search took {elapsed:?}"));
    }
    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}
