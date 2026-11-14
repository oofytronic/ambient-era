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
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            device: String::new(),
            name: String::new(),
            appearance: Appearance::Dark,
            seen: true,
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
        "v1\ndevice={}\nname={}\ntheme={}\nseen={}\n",
        one_line(&profile.device),
        one_line(&profile.name),
        profile.appearance.as_str(),
        if profile.seen { "yes" } else { "no" }
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

pub(super) fn system_is_dark() -> bool {
    if let Ok(output) = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "color-scheme"])
        .output()
    {
        let text = String::from_utf8_lossy(&output.stdout);
        if text.contains("prefer-dark") {
            return true;
        }
        if text.contains("prefer-light") || text.contains("default") {
            return false;
        }
    }
    true
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
    use super::{Appearance, parse_names, parse_profile, parse_seen};

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
}
