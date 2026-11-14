//! Files a person adds from this computer.
//!
//! The bytes stay in the notebook folder. A chat message only cites them.
//! The citation can sync. The bytes do not. That is not a new record.

use std::io::Read;
use std::path::{Path, PathBuf};

const MARK: &str = "aerodash-file:";
const MAX_BYTES: u64 = 256 * 1024 * 1024;

pub(super) struct Cite {
    pub name: String,
    pub id: String,
}

#[derive(Debug)]
pub(super) enum Piece {
    Text(String),
    Picture { name: String, path: Option<PathBuf> },
    Video { name: String, path: Option<PathBuf> },
    File { name: String, path: Option<PathBuf> },
}

pub(super) enum NoteMedia {
    Picture(PathBuf),
    Video(PathBuf, String),
    File(PathBuf, String),
}

pub(super) fn store(root: &Path, conversation: &str, source: &Path) -> Result<String, String> {
    if !conversation_ok(conversation) {
        return Err("That conversation cannot hold a file.".to_owned());
    }
    let meta = std::fs::metadata(source).map_err(|_| "That file could not be read.".to_owned())?;
    if !meta.is_file() {
        return Err("Choose a file, not a folder.".to_owned());
    }
    if meta.len() > MAX_BYTES {
        return Err("That file is larger than 256 MB.".to_owned());
    }
    let id = fresh_id()?;
    let name = safe_name(source);
    let dir = root.join("chat-files").join(conversation);
    std::fs::create_dir_all(&dir).map_err(|_| "Could not save the file.".to_owned())?;
    let dest = dir.join(format!("{id}-{name}"));
    std::fs::copy(source, &dest).map_err(|_| "Could not save the file.".to_owned())?;
    Ok(cite_line(&id, &name))
}

/// Copies a file into the notebook and returns markdown that points at it.
pub(super) fn store_for_note(root: &Path, source: &Path) -> Result<String, String> {
    let meta = std::fs::metadata(source).map_err(|_| "That file could not be read.".to_owned())?;
    if !meta.is_file() {
        return Err("Choose a file, not a folder.".to_owned());
    }
    if meta.len() > MAX_BYTES {
        return Err("That file is larger than 256 MB.".to_owned());
    }
    let id = fresh_id()?;
    let name = safe_name(source);
    let dir = root.join("note-files");
    std::fs::create_dir_all(&dir).map_err(|_| "Could not save the file.".to_owned())?;
    let file_name = format!("{id}-{name}");
    let dest = dir.join(&file_name);
    std::fs::copy(source, &dest).map_err(|_| "Could not save the file.".to_owned())?;
    let rel = format!("note-files/{file_name}");
    if is_image(&name) {
        Ok(format!("![{name}]({rel})"))
    } else {
        Ok(format!("[{name}]({rel})"))
    }
}

pub(super) fn pieces(root: &Path, conversation: &str, text: &str) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut rest = text;
    let mut prose = String::new();
    while let Some(found) = find_cite(rest) {
        prose.push_str(rest.get(..found.at).unwrap_or(""));
        flush_prose(&mut prose, &mut out);
        let path = resolve(root, conversation, &found.cite.id);
        let name = found.cite.name;
        out.push(kind_piece(name, path));
        rest = rest.get(found.end..).unwrap_or("");
    }
    prose.push_str(rest);
    flush_prose(&mut prose, &mut out);
    out
}

pub(super) fn note_media(root: &Path, body: &str) -> Vec<NoteMedia> {
    let mut out = Vec::new();
    let mut rest = body;
    while let Some(start) = rest.find("](") {
        let after = start.saturating_add(2);
        let Some(end_rel) = rest.get(after..).and_then(|slice| slice.find(')')) else {
            break;
        };
        let raw = rest.get(after..after.saturating_add(end_rel)).unwrap_or("");
        if let Some(media) = media_at(root, raw) {
            out.push(media);
        }
        rest = rest
            .get(after.saturating_add(end_rel).saturating_add(1)..)
            .unwrap_or("");
    }
    out
}

pub(super) fn choose() -> Result<PathBuf, String> {
    let attempts: &[(&str, &[&str])] = &[
        ("zenity", &["--file-selection", "--title=Add a file"]),
        ("kdialog", &["--getopenfilename", ".", "All files (*)"]),
        ("yad", &["--file", "--title=Add a file"]),
    ];
    for (bin, args) in attempts {
        match std::process::Command::new(bin).args(*args).output() {
            Ok(output) => {
                if !output.status.success() {
                    return Err("No file chosen.".to_owned());
                }
                let text = String::from_utf8_lossy(&output.stdout);
                let path = text.lines().next().unwrap_or("").trim();
                if path.is_empty() {
                    return Err("No file chosen.".to_owned());
                }
                return Ok(PathBuf::from(path));
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => {}
        }
    }
    Err("No file chooser is installed. Drop the file onto the window.".to_owned())
}

pub(super) fn open_url(url: &str) -> Result<(), String> {
    if !link_allowed(url) {
        return Err("That link is not opened from here.".to_owned());
    }
    for (bin, extra) in [
        ("xdg-open", None),
        ("gio", Some("open")),
        ("kde-open5", None),
    ] {
        let mut cmd = std::process::Command::new(bin);
        if let Some(arg) = extra {
            cmd.arg(arg);
        }
        cmd.arg(url);
        if cmd.spawn().is_ok() {
            return Ok(());
        }
    }
    Err("Could not open that link.".to_owned())
}

fn link_allowed(url: &str) -> bool {
    if url.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return false;
    }
    let lower = url.to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://") || lower.starts_with("mailto:")
}

pub(super) fn open_with_system(path: &Path) -> Result<(), String> {
    for (bin, extra) in [
        ("xdg-open", None),
        ("gio", Some("open")),
        ("kde-open5", None),
    ] {
        let mut cmd = std::process::Command::new(bin);
        if let Some(arg) = extra {
            cmd.arg(arg);
        }
        cmd.arg(path);
        if cmd.spawn().is_ok() {
            return Ok(());
        }
    }
    Err("Could not open that file.".to_owned())
}

fn kind_piece(name: String, path: Option<PathBuf>) -> Piece {
    if is_image(&name) {
        Piece::Picture { name, path }
    } else if is_video(&name) {
        Piece::Video { name, path }
    } else {
        Piece::File { name, path }
    }
}

fn flush_prose(prose: &mut String, out: &mut Vec<Piece>) {
    let trimmed = prose.trim();
    if !trimmed.is_empty() {
        out.push(Piece::Text(trimmed.to_owned()));
    }
    prose.clear();
}

struct Found {
    at: usize,
    end: usize,
    cite: Cite,
}

fn find_cite(text: &str) -> Option<Found> {
    let at = text.find("![")?;
    let after_bang = at.saturating_add(2);
    let rest = text.get(after_bang..)?;
    let name_end = rest.find("](")?;
    let name = rest.get(..name_end)?.trim();
    if name.is_empty() || name.contains('\n') {
        return find_cite(text.get(after_bang..)?).map(|mut next| {
            next.at = next.at.saturating_add(after_bang);
            next.end = next.end.saturating_add(after_bang);
            next
        });
    }
    let url_at = after_bang.saturating_add(name_end).saturating_add(2);
    let url_rest = text.get(url_at..)?;
    if !url_rest.starts_with(MARK) {
        return find_cite(text.get(url_at..)?).map(|mut next| {
            next.at = next.at.saturating_add(url_at);
            next.end = next.end.saturating_add(url_at);
            next
        });
    }
    let id_at = MARK.len();
    let id_rest = url_rest.get(id_at..)?;
    let id_len = id_rest.bytes().take_while(u8::is_ascii_hexdigit).count();
    if id_len < 8 || id_rest.as_bytes().get(id_len) != Some(&b')') {
        return None;
    }
    let id = id_rest.get(..id_len)?.to_owned();
    let end = url_at
        .saturating_add(id_at)
        .saturating_add(id_len)
        .saturating_add(1);
    Some(Found {
        at,
        end,
        cite: Cite {
            name: name.to_owned(),
            id,
        },
    })
}

fn resolve(root: &Path, conversation: &str, id: &str) -> Option<PathBuf> {
    if !conversation_ok(conversation) || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let dir = root.join("chat-files").join(conversation);
    let prefix = format!("{id}-");
    let entries = std::fs::read_dir(&dir).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(text) = name.to_str() else {
            continue;
        };
        if text.starts_with(&prefix) && entry.path().is_file() {
            return Some(entry.path());
        }
    }
    None
}

fn media_at(root: &Path, raw: &str) -> Option<NoteMedia> {
    let raw = raw.trim();
    if raw.is_empty()
        || raw.starts_with(MARK)
        || raw.contains("://")
        || raw.contains('\\')
        || raw.split('/').any(|part| part == "..")
    {
        return None;
    }
    let path = if raw.starts_with('/') {
        PathBuf::from(raw)
    } else {
        root.join(raw)
    };
    let root_real = std::fs::canonicalize(root).ok()?;
    let file_real = std::fs::canonicalize(&path).ok()?;
    if !file_real.starts_with(&root_real) || !file_real.is_file() {
        return None;
    }
    let name = file_real
        .file_name()
        .and_then(|text| text.to_str())
        .unwrap_or("file")
        .to_owned();
    if is_image(&name) {
        Some(NoteMedia::Picture(file_real))
    } else if is_video(&name) {
        Some(NoteMedia::Video(file_real, name))
    } else {
        Some(NoteMedia::File(file_real, name))
    }
}

fn cite_line(id: &str, name: &str) -> String {
    format!("![{name}]({MARK}{id})")
}

fn conversation_ok(id: &str) -> bool {
    (16..=64).contains(&id.len()) && id.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn safe_name(path: &Path) -> String {
    let raw = path
        .file_name()
        .and_then(|text| text.to_str())
        .unwrap_or("file");
    let mut out = String::new();
    for char in raw.chars() {
        if char.is_ascii_alphanumeric() || matches!(char, '.' | '-' | '_' | ' ') {
            out.push(char);
        }
    }
    let out = out.trim().trim_start_matches('.').trim();
    let out = if out.is_empty() { "file" } else { out };
    let mut clipped = String::new();
    for char in out.chars().take(60) {
        clipped.push(char);
    }
    if clipped.is_empty() {
        "file".to_owned()
    } else {
        clipped
    }
}

fn is_image(name: &str) -> bool {
    matches!(
        extension(name).as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp"
    )
}

fn is_video(name: &str) -> bool {
    matches!(
        extension(name).as_str(),
        "mp4" | "webm" | "mov" | "mkv" | "m4v" | "avi"
    )
}

fn extension(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default()
}

fn fresh_id() -> Result<String, String> {
    let mut bytes = [0_u8; 8];
    let mut file =
        std::fs::File::open("/dev/urandom").map_err(|_| "Could not name the file.".to_owned())?;
    file.read_exact(&mut bytes)
        .map_err(|_| "Could not name the file.".to_owned())?;
    let mut out = String::with_capacity(16);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{NoteMedia, Piece, note_media, pieces, store, store_for_note};

    fn scratch() -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ambient-files-{}-{}", std::process::id(), fresh()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn fresh() -> String {
        super::fresh_id().unwrap()
    }

    #[test]
    fn a_chat_file_is_cited_and_the_bytes_stay_in_the_folder() {
        let root = scratch();
        let source = root.join("porch.png");
        std::fs::write(&source, b"png").unwrap();
        let conv = "ab".repeat(16);
        let line = store(&root, &conv, &source).unwrap();
        assert!(line.starts_with("![porch.png](aerodash-file:"));
        let parsed = pieces(&root, &conv, &format!("see this\n{line}"));
        assert!(matches!(&parsed[0], Piece::Text(text) if text == "see this"));
        match &parsed[1] {
            Piece::Picture { name, path } => {
                assert_eq!(name, "porch.png");
                let path = path.as_ref().unwrap();
                assert_eq!(std::fs::read(path).unwrap(), b"png");
                assert!(path.starts_with(root.join("chat-files")));
            }
            other => panic!("expected a picture, got another piece {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_note_file_stays_inside_the_notebook() {
        let root = scratch();
        let source = root.join("clip.mp4");
        std::fs::write(&source, b"mp4").unwrap();
        let markdown = store_for_note(&root, &source).unwrap();
        assert!(markdown.starts_with("[clip.mp4](note-files/"));
        let body = format!("watch\n{markdown}");
        let media = note_media(&root, &body);
        assert!(
            matches!(media.first(), Some(NoteMedia::Video(_, name)) if name.ends_with("clip.mp4"))
        );
        assert!(note_media(&root, "![x](https://example.com/a.png)").is_empty());
        assert!(note_media(&root, "![x](../secret.png)").is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }
}
