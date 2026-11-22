//! Local names and appearance. Not a record. Not sealed. Not sent.

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Appearance {
    Dark,
    Light,
    System,
}

impl Appearance {
    fn as_str(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::System => "system",
        }
    }

    fn parse(text: &str) -> Self {
        match text {
            "light" => Self::Light,
            "system" => Self::System,
            _ => Self::Dark,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Profile {
    pub device: String,
    pub name: String,
    pub appearance: Appearance,
    /// False only for a folder that has not finished the first welcome.
    /// Older profiles have no line for this, and those count as finished.
    pub seen: bool,
    /// Where the last backup went, or where the next one will. Empty until one is made.
    pub backup_to: String,
    /// When the last backup finished, in unix milliseconds. Zero when there has not been one.
    pub backup_at: u64,
    /// Times read `3:42 PM` when true and `15:42` when false.
    pub hour12: bool,
    /// Weeks begin on Monday when true and Sunday when false.
    pub week_monday: bool,
    /// The time zone the person lives in, by name, or empty until one is known. Where a trip is
    /// noticed from.
    pub home_zone: String,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            device: String::new(),
            name: String::new(),
            appearance: Appearance::Dark,
            seen: true,
            backup_to: String::new(),
            backup_at: 0,
            hour12: true,
            week_monday: false,
            home_zone: String::new(),
        }
    }
}

pub(super) fn load_profile(dir: &Path) -> Profile {
    let Ok(text) = fs::read_to_string(dir.join("profile")) else {
        return Profile::default();
    };
    parse_profile(&text)
}

pub(super) fn store_profile(dir: &Path, profile: &Profile) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|_| "could not write settings".to_owned())?;
    let text = format!(
        "v1\ndevice={}\nname={}\ntheme={}\nseen={}\nbackup={}\nbackup_at={}\nclock={}\nweek={}\nhome_zone={}\n",
        one_line(&profile.device),
        one_line(&profile.name),
        profile.appearance.as_str(),
        if profile.seen { "yes" } else { "no" },
        one_line(&profile.backup_to),
        profile.backup_at,
        if profile.hour12 { "12" } else { "24" },
        if profile.week_monday { "mon" } else { "sun" },
        one_line(&profile.home_zone),
    );
    fs::write(dir.join("profile"), text).map_err(|_| "could not write settings".to_owned())
}

pub(super) fn load_names(dir: &Path) -> Vec<(String, String)> {
    let Ok(text) = fs::read_to_string(dir.join("names")) else {
        return Vec::new();
    };
    parse_names(&text)
}

pub(super) fn store_names(dir: &Path, names: &[(String, String)]) -> Result<(), String> {
    let mut text = String::from("v1\n");
    for (id, name) in names {
        if id.is_empty() || name.is_empty() {
            continue;
        }
        text.push_str(&one_line(id));
        text.push('\t');
        text.push_str(&one_line(name));
        text.push('\n');
    }
    fs::write(dir.join("names"), text).map_err(|_| "could not write names".to_owned())
}

/// Words this person told the spellchecker are fine. One per line, lowercase letters. Not
/// sealed and not sent. It is a list of words, not notes, and it stays on this computer.
pub(super) fn load_words(dir: &Path) -> Vec<String> {
    let Ok(text) = fs::read_to_string(dir.join("dictionary")) else {
        return Vec::new();
    };
    text.lines()
        .map(str::trim)
        .filter(|word| {
            word.len() >= 3
                && word
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte == b'\'')
        })
        .map(str::to_owned)
        .collect()
}

pub(super) fn store_words(dir: &Path, words: &[String]) -> Result<(), String> {
    let mut text = String::new();
    for word in words {
        text.push_str(word);
        text.push('\n');
    }
    fs::write(dir.join("dictionary"), text)
        .map_err(|_| "could not write your dictionary".to_owned())
}

/// The order the notes of each group were put in by hand: a group's key, then its note ids.
/// Not sealed and not sent, so it stays on this computer. Ids are random, but a group's name is
/// a word from the notes, so this file is plain text beside the sealed log like `names`.
pub(super) fn load_order(dir: &Path) -> std::collections::BTreeMap<String, Vec<String>> {
    let mut found = std::collections::BTreeMap::new();
    let Ok(text) = fs::read_to_string(dir.join("order")) else {
        return found;
    };
    for line in text.lines().skip(1) {
        let Some((key, ids)) = line.split_once('\t') else {
            continue;
        };
        let ids: Vec<String> = ids
            .split(',')
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_owned)
            .collect();
        if !key.is_empty() && !ids.is_empty() {
            found.insert(key.to_owned(), ids);
        }
    }
    found
}

pub(super) fn store_order(
    dir: &Path,
    order: &std::collections::BTreeMap<String, Vec<String>>,
) -> Result<(), String> {
    let mut text = String::from("v1\n");
    for (key, ids) in order {
        if key.is_empty() || ids.is_empty() {
            continue;
        }
        text.push_str(&one_line(key));
        text.push('\t');
        text.push_str(&ids.join(","));
        text.push('\n');
    }
    fs::write(dir.join("order"), text).map_err(|_| "could not write the order".to_owned())
}

/// How far this computer has read each conversation. Not sealed. Not sent.
pub(super) fn load_seen(dir: &Path) -> Vec<(String, usize)> {
    let Ok(text) = fs::read_to_string(dir.join("seen")) else {
        return Vec::new();
    };
    parse_seen(&text)
}

pub(super) fn store_seen(dir: &Path, rows: &[(String, usize)]) -> Result<(), String> {
    let mut text = String::from("v1\n");
    for (id, count) in rows {
        if id.is_empty() {
            continue;
        }
        text.push_str(&one_line(id));
        text.push('\t');
        text.push_str(&count.to_string());
        text.push('\n');
    }
    fs::write(dir.join("seen"), text).map_err(|_| "could not write seen".to_owned())
}

/// Whether the system is set to dark. When the host cannot tell, dark.
pub(super) fn system_is_dark() -> bool {
    ambient_cli::host().prefers_dark().unwrap_or(true)
}

pub(super) fn export_notes(
    dir: &Path,
    notes: &[(String, String, String)],
) -> Result<PathBuf, String> {
    let dest = dir.join("export");
    fs::create_dir_all(&dest).map_err(|_| "could not write the export".to_owned())?;
    let mut used = std::collections::BTreeSet::new();
    for (id, title, body) in notes {
        let mut name = file_name(title);
        if name.is_empty() {
            name = id.chars().take(8).collect();
        }
        if !used.insert(name.clone()) {
            let short: String = id.chars().take(8).collect();
            name = format!("{name}-{short}");
        }
        let path = dest.join(format!("{name}.md"));
        let mut text = String::new();
        if !title.is_empty() {
            text.push_str("# ");
            text.push_str(title);
            text.push_str("\n\n");
        }
        text.push_str(body);
        if !text.ends_with('\n') {
            text.push('\n');
        }
        fs::write(path, text).map_err(|_| "could not write the export".to_owned())?;
    }
    Ok(dest)
}

/// What a backup made.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct BackupReport {
    /// The folder the copy is in.
    pub path: PathBuf,
    pub files: usize,
    pub bytes: u64,
}

/// A place to put copies when none has been chosen: a folder named `Aerodash backups` in the home
/// folder.
pub(super) fn default_backup_dir() -> PathBuf {
    backup_dir_in(ambient_cli::host().home_dir())
}

pub(super) fn backup_dir_in(home: Option<PathBuf>) -> PathBuf {
    home.map_or_else(
        || PathBuf::from("Aerodash backups"),
        |home| home.join("Aerodash backups"),
    )
}

/// A typed place, with a leading `~/` turned into the home folder.
pub(super) fn expand_home(typed: &str) -> PathBuf {
    expand_home_in(typed, ambient_cli::host().home_dir())
}

pub(super) fn expand_home_in(typed: &str, home: Option<PathBuf>) -> PathBuf {
    match (typed.strip_prefix("~/"), home) {
        (Some(rest), Some(home)) => home.join(rest),
        (None, Some(home)) if typed == "~" => home,
        _ => PathBuf::from(typed),
    }
}

/// What a copy of the folder leaves out. `export` holds readable Markdown that a person made on
/// purpose, and putting it in a copy would defeat a passphrase on the key file. The search index
/// is rebuilt from the log. A lock or a write in progress means nothing in a copy.
fn left_out(at_top: bool, name: &str) -> bool {
    let temporary = std::path::Path::new(name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("lock") || ext.eq_ignore_ascii_case("part"));
    temporary || (at_top && (name == "export" || name.starts_with("search.db")))
}

/// Copy the notebook folder into `dest_root/name`.
///
/// The copy is made in `name.partial` and renamed when it is whole, so a copy that was cut short
/// never looks finished. A name that is taken gets `-2`, `-3`, and so on, and nothing already
/// there is touched. A symbolic link is not followed. The key file and the notes log are compared
/// with the originals, byte for byte, before the copy is called done.
///
/// The copy holds the key file. Whoever has it can read the notes unless the key file is wrapped.
pub(super) fn backup_folder(
    src: &Path,
    dest_root: &Path,
    name: &str,
) -> Result<BackupReport, String> {
    if !src.is_dir() {
        return Err("This notebook has no folder to copy.".to_owned());
    }
    if dest_root.as_os_str().is_empty() {
        return Err("Type where to put the copy.".to_owned());
    }
    let from = fs::canonicalize(src).map_err(|_| "Could not read this folder.".to_owned())?;
    // Where the copy would go, worked out before anything is made. A refused place leaves nothing
    // behind, least of all a folder inside the notebook.
    let wanted = resolved(dest_root).ok_or_else(|| "Could not read that folder.".to_owned())?;
    if wanted.starts_with(&from) {
        return Err("Choose a place outside the notebook folder.".to_owned());
    }
    fs::create_dir_all(dest_root).map_err(|_| "Could not make that folder.".to_owned())?;
    let root = fs::canonicalize(dest_root).map_err(|_| "Could not read that folder.".to_owned())?;
    if root.starts_with(&from) {
        return Err("Choose a place outside the notebook folder.".to_owned());
    }
    let mut finished = root.join(name);
    let mut taken = 2_u32;
    while finished.exists() {
        if taken > 999 {
            return Err("Too many copies with that name.".to_owned());
        }
        finished = root.join(format!("{name}-{taken}"));
        taken = taken.saturating_add(1);
    }
    let mut partial_name = finished.as_os_str().to_owned();
    partial_name.push(".partial");
    let partial = PathBuf::from(partial_name);
    if partial.exists() {
        fs::remove_dir_all(&partial)
            .map_err(|_| "Could not clear an unfinished copy.".to_owned())?;
    }
    make_private_dir(&partial)?;
    let mut files = 0_usize;
    let mut bytes = 0_u64;
    let made = copy_tree(&from, &partial, true, &mut files, &mut bytes);
    let checked = made.and_then(|()| check_essentials(&from, &partial));
    if let Err(err) = checked {
        let _ = fs::remove_dir_all(&partial);
        return Err(err);
    }
    if fs::rename(&partial, &finished).is_err() {
        let _ = fs::remove_dir_all(&partial);
        return Err("Could not finish the copy.".to_owned());
    }
    Ok(BackupReport {
        path: finished,
        files,
        bytes,
    })
}

/// The real path of a place that may not exist yet: the nearest part that does exist, resolved,
/// and the rest added on. `None` for a path with `..` in the part that does not exist.
fn resolved(path: &Path) -> Option<PathBuf> {
    let mut at = if path.is_relative() {
        std::env::current_dir().ok()?.join(path)
    } else {
        path.to_path_buf()
    };
    let mut tail = Vec::new();
    loop {
        if let Ok(mut real) = fs::canonicalize(&at) {
            for part in tail.iter().rev() {
                real.push(part);
            }
            return Some(real);
        }
        tail.push(at.file_name()?.to_owned());
        at = at.parent()?.to_path_buf();
    }
}

/// A folder only its owner can open. The copy holds the key file.
fn make_private_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|_| "Could not make the copy.".to_owned())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
    }
    Ok(())
}

fn copy_tree(
    from: &Path,
    to: &Path,
    at_top: bool,
    files: &mut usize,
    bytes: &mut u64,
) -> Result<(), String> {
    let cut = || "Could not copy everything.".to_owned();
    for entry in fs::read_dir(from).map_err(|_| cut())? {
        let entry = entry.map_err(|_| cut())?;
        let kind = entry.file_type().map_err(|_| cut())?;
        let name = entry.file_name();
        if kind.is_symlink() || left_out(at_top, &name.to_string_lossy()) {
            continue;
        }
        let target = to.join(&name);
        if kind.is_dir() {
            make_private_dir(&target)?;
            copy_tree(&entry.path(), &target, false, files, bytes)?;
        } else if kind.is_file() {
            let copied = fs::copy(entry.path(), &target).map_err(|_| cut())?;
            let landed = fs::metadata(&target).map_err(|_| cut())?.len();
            if landed != copied {
                return Err("A file was cut short while copying.".to_owned());
            }
            *files = files.saturating_add(1);
            *bytes = bytes.saturating_add(copied);
        }
    }
    Ok(())
}

/// The key file and the notes log are the two files a copy is useless without. Each is read back
/// and compared with the original. The copy is flushed to disk too, so pulling a drive out right
/// after does not leave half of it.
fn check_essentials(from: &Path, to: &Path) -> Result<(), String> {
    for name in ["key", "notes"] {
        let original = from.join(name);
        if !original.exists() {
            continue;
        }
        let mine = fs::read(&original).map_err(|_| "Could not check the copy.".to_owned())?;
        let theirs = fs::read(to.join(name)).map_err(|_| "Could not check the copy.".to_owned())?;
        if mine != theirs {
            return Err("The notes changed while copying. Try again.".to_owned());
        }
        fs::File::open(to.join(name))
            .and_then(|file| file.sync_all())
            .map_err(|_| "Could not save the copy to disk.".to_owned())?;
    }
    Ok(())
}

pub(super) fn wipe_folder(dir: &Path) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir).map_err(|_| "could not open the folder".to_owned())? {
        let path = entry
            .map_err(|_| "could not open the folder".to_owned())?
            .path();
        if path.is_dir() {
            fs::remove_dir_all(path).map_err(|_| "could not erase the folder".to_owned())?;
        } else {
            fs::remove_file(path).map_err(|_| "could not erase the folder".to_owned())?;
        }
    }
    Ok(())
}

fn parse_profile(text: &str) -> Profile {
    let mut profile = Profile::default();
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("device=") {
            value.trim().clone_into(&mut profile.device);
        } else if let Some(value) = line.strip_prefix("name=") {
            value.trim().clone_into(&mut profile.name);
        } else if let Some(value) = line.strip_prefix("theme=") {
            profile.appearance = Appearance::parse(value.trim());
        } else if let Some(value) = line.strip_prefix("seen=") {
            profile.seen = value.trim() != "no";
        } else if let Some(value) = line.strip_prefix("backup=") {
            value.trim().clone_into(&mut profile.backup_to);
        } else if let Some(value) = line.strip_prefix("backup_at=") {
            profile.backup_at = value.trim().parse().unwrap_or(0);
        } else if let Some(value) = line.strip_prefix("clock=") {
            profile.hour12 = value.trim() != "24";
        } else if let Some(value) = line.strip_prefix("week=") {
            profile.week_monday = value.trim() == "mon";
        } else if let Some(value) = line.strip_prefix("home_zone=") {
            value.trim().clone_into(&mut profile.home_zone);
        }
    }
    profile
}

fn parse_seen(text: &str) -> Vec<(String, usize)> {
    let mut rows = Vec::new();
    for line in text.lines() {
        if line == "v1" || line.is_empty() {
            continue;
        }
        let Some((id, count)) = line.split_once('\t') else {
            continue;
        };
        let id = id.trim();
        let Ok(count) = count.trim().parse::<usize>() else {
            continue;
        };
        if id.is_empty() {
            continue;
        }
        rows.push((id.to_owned(), count));
    }
    rows
}

fn parse_names(text: &str) -> Vec<(String, String)> {
    let mut names = Vec::new();
    for line in text.lines() {
        if line == "v1" || line.is_empty() {
            continue;
        }
        let Some((id, name)) = line.split_once('\t') else {
            continue;
        };
        let id = id.trim();
        let name = name.trim();
        if id.is_empty() || name.is_empty() {
            continue;
        }
        names.push((id.to_owned(), name.to_owned()));
    }
    names
}

fn one_line(text: &str) -> String {
    text.replace(['\n', '\r', '\t'], " ").trim().to_owned()
}

fn file_name(title: &str) -> String {
    let mut out = String::new();
    for char in title.chars() {
        if char.is_ascii_alphanumeric() || char == '-' || char == ' ' {
            out.push(if char == ' ' { '-' } else { char });
        }
    }
    out.trim_matches('-').to_owned()
}

#[cfg(test)]
mod tests {
    use super::{Appearance, backup_folder, left_out, parse_names, parse_profile, parse_seen};

    #[test]
    fn time_settings_default_to_a_twelve_hour_clock_and_are_kept() {
        let older = parse_profile("v1\ndevice=laptop\n");
        assert!(
            older.hour12,
            "twelve hour unless the person chose otherwise"
        );
        assert!(!older.week_monday);
        assert!(older.home_zone.is_empty());
        let chosen = parse_profile("v1\nclock=24\nweek=mon\nhome_zone=America/Los_Angeles\n");
        assert!(!chosen.hour12 && chosen.week_monday);
        assert_eq!(chosen.home_zone, "America/Los_Angeles");
        assert!(parse_profile("v1\nclock=12\n").hour12);
        assert!(
            parse_profile("v1\nclock=garbage\n").hour12,
            "unreadable means the default"
        );
        assert!(!parse_profile("v1\nweek=garbage\n").week_monday);
    }

    #[test]
    fn profile_round_trip() {
        let parsed = parse_profile("v1\ndevice=laptop\nname=Ada\ntheme=system\n");
        assert_eq!(parsed.device, "laptop");
        assert_eq!(parsed.name, "Ada");
        assert_eq!(parsed.appearance, Appearance::System);
        assert!(parsed.seen);
    }

    #[test]
    fn a_missing_seen_line_counts_as_finished() {
        let parsed = parse_profile("v1\ndevice=laptop\nname=Ada\ntheme=dark\n");
        assert!(parsed.seen);
    }

    #[test]
    fn seen_no_is_an_unfinished_welcome() {
        let parsed = parse_profile("v1\nseen=no\n");
        assert!(!parsed.seen);
        assert!(parse_profile("v1\nseen=yes\n").seen);
    }

    #[test]
    fn seen_counts_skip_a_broken_line() {
        let rows = parse_seen("v1\nabc\t3\nnope\n\ndef\t0\n");
        assert_eq!(rows, vec![("abc".to_owned(), 3), ("def".to_owned(), 0)]);
    }

    #[test]
    fn names_skip_a_broken_line() {
        let names = parse_names("v1\nabc\tClinic\nnope\n\ndef\tHome\n");
        assert_eq!(
            names,
            vec![
                ("abc".to_owned(), "Clinic".to_owned()),
                ("def".to_owned(), "Home".to_owned())
            ]
        );
    }

    #[test]
    fn the_backup_place_and_time_are_remembered_and_older_profiles_have_none() {
        let parsed = parse_profile("v1\nbackup=/media/usb\nbackup_at=1700000000000\n");
        assert_eq!(parsed.backup_to, "/media/usb");
        assert_eq!(parsed.backup_at, 1_700_000_000_000);
        let older = parse_profile("v1\ndevice=laptop\nname=Ada\ntheme=dark\n");
        assert_eq!(older.backup_to, "");
        assert_eq!(older.backup_at, 0);
        assert_eq!(parse_profile("v1\nbackup_at=soon\n").backup_at, 0);
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("aerodash-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    fn put(dir: &std::path::Path, name: &str, bytes: &[u8]) {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("parent");
        }
        std::fs::write(path, bytes).expect("write");
    }

    fn folder_with_notes(root: &std::path::Path) -> std::path::PathBuf {
        let src = root.join("notebook");
        put(&src, "key", b"AMBK-key-bytes");
        put(&src, "notes", b"sealed-notes-bytes");
        put(&src, "names", b"v1\n");
        put(&src, "conversations/abc/log", b"sealed-chat");
        put(&src, "files/photo.png", &[7_u8; 5000]);
        // What a copy must leave out.
        put(&src, "export/Plan.md", b"# readable, no lock");
        put(&src, "search.db", b"index");
        put(&src, "search.db-journal", b"index");
        put(&src, "notes.lock", b"");
        put(&src, "notes.part", b"half a write");
        put(&src, "conversations/abc/log.lock", b"");
        src
    }

    #[test]
    fn a_backup_copies_what_is_needed_and_leaves_out_what_is_not() {
        let root = scratch("backup-copy");
        let src = folder_with_notes(&root);
        let dest = root.join("usb");
        let report = backup_folder(&src, &dest, "aerodash-2026-11-20-1200").expect("backup");
        assert!(
            report
                .path
                .starts_with(std::fs::canonicalize(&dest).expect("dest"))
        );
        // Everything a restore needs.
        for name in [
            "key",
            "notes",
            "names",
            "conversations/abc/log",
            "files/photo.png",
        ] {
            let was = std::fs::read(src.join(name)).expect("source");
            let now = std::fs::read(report.path.join(name)).expect("copy");
            assert_eq!(was, now, "{name}");
        }
        // And nothing it should not have.
        for name in [
            "export",
            "search.db",
            "search.db-journal",
            "notes.lock",
            "notes.part",
            "conversations/abc/log.lock",
        ] {
            assert!(!report.path.join(name).exists(), "{name} was copied");
        }
        assert_eq!(report.files, 5);
        assert_eq!(
            report.bytes,
            u64::try_from(14 + 18 + 3 + 11 + 5000).expect("small")
        );
        // The unfinished folder is gone, so only the finished copy is there.
        let names: Vec<_> = std::fs::read_dir(&dest)
            .expect("dest")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(names, ["aerodash-2026-11-20-1200"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_backup_can_be_opened_as_a_notebook() {
        let root = scratch("backup-open");
        let src = root.join("notebook");
        let book = ambient_cli::Notebook::create(&src).expect("create");
        let id = book.add("Plan", "buy oats").expect("add");
        let report = backup_folder(&src, &root.join("usb"), "copy").expect("backup");
        let copy = ambient_cli::Notebook::open(&report.path).expect("open the copy");
        let shown = copy.show(&id).expect("the note is in the copy");
        assert_eq!(shown.heads[0].body, "buy oats");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_backup_refuses_a_place_inside_the_notebook_and_a_missing_one() {
        let root = scratch("backup-refuse");
        let src = folder_with_notes(&root);
        let inside = src.join("backups");
        let refused = backup_folder(&src, &inside, "copy").expect_err("inside the notebook");
        assert!(refused.contains("outside"), "{refused}");
        assert!(
            !src.join("backups").exists(),
            "a refused place left a folder in the notebook"
        );
        // The same when the place is deeper than one level and none of it exists yet.
        let deep = src.join("a").join("b");
        assert!(backup_folder(&src, &deep, "copy").is_err());
        assert!(!src.join("a").exists());
        assert!(backup_folder(&src, std::path::Path::new(""), "copy").is_err());
        assert!(backup_folder(&root.join("missing"), &root.join("usb"), "copy").is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_backup_never_overwrites_an_earlier_one() {
        let root = scratch("backup-twice");
        let src = folder_with_notes(&root);
        let dest = root.join("usb");
        let first = backup_folder(&src, &dest, "same").expect("first");
        put(&first.path, "marker", b"keep me");
        let second = backup_folder(&src, &dest, "same").expect("second");
        let third = backup_folder(&src, &dest, "same").expect("third");
        assert_ne!(first.path, second.path);
        assert_ne!(second.path, third.path);
        assert!(second.path.ends_with("same-2") && third.path.ends_with("same-3"));
        assert_eq!(
            std::fs::read(first.path.join("marker")).expect("marker"),
            b"keep me"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn a_backup_is_private_and_does_not_follow_a_link() {
        use std::os::unix::fs::PermissionsExt;
        let root = scratch("backup-private");
        let src = folder_with_notes(&root);
        put(&root, "outside-secret", b"not part of the notebook");
        std::os::unix::fs::symlink(root.join("outside-secret"), src.join("link")).expect("link");
        let report = backup_folder(&src, &root.join("usb"), "copy").expect("backup");
        assert!(!report.path.join("link").exists(), "a link was followed");
        let mode = std::fs::metadata(&report.path)
            .expect("meta")
            .permissions()
            .mode();
        assert_eq!(
            mode & 0o077,
            0,
            "the copy folder is open to others: {mode:o}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn what_a_copy_leaves_out_is_only_that() {
        assert!(left_out(true, "export"));
        assert!(left_out(true, "search.db"));
        assert!(left_out(true, "search.db-wal"));
        assert!(left_out(false, "log.lock"));
        assert!(left_out(false, "notes.part"));
        // A folder named export deeper down is a user's, and is kept.
        assert!(!left_out(false, "export"));
        assert!(!left_out(true, "notes"));
        assert!(!left_out(true, "key"));
        assert!(!left_out(true, "dictionary"));
    }
}
