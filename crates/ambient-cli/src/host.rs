//! What the notebook needs from the machine it runs on, and nothing else.
//!
//! The notes, the sync, and the folder format do not care which computer they are on. A few
//! small things do: where a person's files live, whether the system is in dark mode, how to
//! open a file or a link with the program that owns it, and how to ask for a file. Those are
//! the [`Host`]. Each platform answers them its own way, and everything above the host asks the
//! trait and never looks at an environment variable, a program name, or a path convention.
//!
//! To add a platform, implement [`Host`] and give it to [`set_host`] before anything else runs.
//! A native shell (`SwiftUI` on a Mac or an iPhone, Compose on Android) will do that from its own
//! side, with the system calls it already has. The three here are the ones the Linux window,
//! and a first try on a Mac, use.
//!
//! The host holds no notes, keys, or user text. It is given a path or a link and returns an
//! answer. Do not put anything in it that reads or writes the notebook.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The computer's time zone: the `TZif` file the system keeps for it, and its name when the
/// system says what it is.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimeZone {
    /// `America/Los_Angeles`, or `None` when the system does not say.
    pub name: Option<String>,
    /// The bytes of the zone file, in the format of RFC 8536.
    pub tzif: Vec<u8>,
}

/// The machine, as far as the notebook needs to know.
pub trait Host: Send + Sync {
    /// The person's home folder, when there is one. Used to expand a typed `~/`, and to offer a
    /// place for backups.
    fn home_dir(&self) -> Option<PathBuf>;

    /// The folder this program keeps its own data in, by the platform's convention. The
    /// notebook is not here unless the person chose it to be. Mail for the relay on this
    /// computer is.
    fn data_dir(&self) -> Option<PathBuf>;

    /// Whether the system is set to dark. `None` when this host cannot tell, and the caller
    /// picks.
    fn prefers_dark(&self) -> Option<bool>;

    /// The zone this computer is set to now. Asked again each time it might have changed. `None`
    /// when this host cannot tell, and the caller uses UTC and says so.
    fn time_zone(&self) -> Option<TimeZone>;

    /// The bytes of the zone file for a name such as `America/Los_Angeles`, whether or not the
    /// computer is in it. `None` when the name is not a plain zone name or the system has no such
    /// file. Used to say what time it is somewhere else.
    fn zone_named(&self, name: &str) -> Option<Vec<u8>>;

    /// Open a file with the program the system uses for it.
    ///
    /// # Errors
    ///
    /// Returns a short sentence a person can read when nothing could open it.
    fn open_path(&self, path: &Path) -> Result<(), String>;

    /// Open a link in the person's browser or mail program. The caller has already decided the
    /// link is allowed. The host does not judge it.
    ///
    /// # Errors
    ///
    /// Returns a short sentence a person can read when nothing could open it.
    fn open_url(&self, url: &str) -> Result<(), String>;

    /// Ask the person to choose a file.
    ///
    /// # Errors
    ///
    /// Returns a short sentence a person can read when no file was chosen, or no way to ask
    /// exists.
    fn choose_file(&self) -> Result<PathBuf, String>;
}

static HOST: OnceLock<Box<dyn Host>> = OnceLock::new();

/// The host in use. Until [`set_host`] is called it is the one for the system this was built for.
pub fn host() -> &'static dyn Host {
    HOST.get_or_init(system_host).as_ref()
}

/// Use `host` for everything from now on.
///
/// # Errors
///
/// Returns the host back when one is already in use, which includes the default one once
/// anything has asked for it. Call this first.
pub fn set_host(host: Box<dyn Host>) -> Result<(), Box<dyn Host>> {
    HOST.set(host)
}

/// The host for the system this was built for.
#[must_use]
pub fn system_host() -> Box<dyn Host> {
    if cfg!(target_os = "macos") {
        Box::new(MacHost)
    } else if cfg!(unix) {
        Box::new(UnixHost)
    } else {
        Box::new(BareHost)
    }
}

fn non_empty(value: Option<OsString>) -> Option<PathBuf> {
    value.filter(|value| !value.is_empty()).map(PathBuf::from)
}

fn env_path(name: &str) -> Option<PathBuf> {
    non_empty(std::env::var_os(name))
}

/// Try each program in turn with these arguments and the target last. The first that starts
/// is taken as having worked, because these programs hand off and return at once.
fn first_that_starts(programs: &[(&str, &[&str])], target: &std::ffi::OsStr) -> bool {
    programs.iter().any(|(program, args)| {
        std::process::Command::new(program)
            .args(*args)
            .arg(target)
            .spawn()
            .is_ok()
    })
}

/// Run a program that asks for a file and prints the path. `Ok(None)` means it is not here.
fn ask_for_path(program: &str, args: &[&str]) -> Result<Option<PathBuf>, String> {
    match std::process::Command::new(program).args(args).output() {
        Ok(output) => {
            let text = String::from_utf8_lossy(&output.stdout);
            let path = text.lines().next().unwrap_or("").trim();
            if !output.status.success() || path.is_empty() {
                return Err("No file chosen.".to_owned());
            }
            Ok(Some(PathBuf::from(path)))
        }
        Err(_) => Ok(None),
    }
}

/// Linux, and other Unix systems that follow the freedesktop conventions.
#[derive(Clone, Copy, Debug)]
pub struct UnixHost;

/// `$XDG_DATA_HOME`, or `~/.local/share` when it is not set. Takes the two values so it can be
/// checked without touching the environment.
#[must_use]
fn xdg_data_dir(xdg: Option<OsString>, home: Option<OsString>) -> Option<PathBuf> {
    non_empty(xdg).or_else(|| non_empty(home).map(|home| home.join(".local").join("share")))
}

/// Whether a `gsettings` answer for the GNOME color scheme says dark. `None` when it says
/// neither, which is what a desktop with no such setting gives.
#[must_use]
fn gnome_scheme_is_dark(answer: &str) -> Option<bool> {
    if answer.contains("prefer-dark") {
        Some(true)
    } else if answer.contains("prefer-light") || answer.contains("default") {
        Some(false)
    } else {
        None
    }
}

/// The zone file Unix systems, macOS included, point `/etc/localtime` at, and its name, which is
/// the part of the link's target after `zoneinfo/`.
fn unix_time_zone() -> Option<TimeZone> {
    let path = Path::new("/etc/localtime");
    let tzif = std::fs::read(path).ok()?;
    let name = std::fs::read_link(path)
        .ok()
        .and_then(|target| zone_name_from_link(&target));
    Some(TimeZone { name, tzif })
}

/// `America/Los_Angeles` from `/usr/share/zoneinfo/America/Los_Angeles`, or from the same under
/// any other prefix, as a Mac has.
fn zone_name_from_link(target: &Path) -> Option<String> {
    let text = target.to_str()?;
    let name = text.split_once("zoneinfo/")?.1;
    (!name.is_empty()).then(|| name.to_owned())
}

/// Where Unix systems and Macs keep their zone files.
const ZONE_DIRS: [&str; 3] = [
    "/usr/share/zoneinfo",
    "/var/db/timezone/zoneinfo",
    "/usr/lib/zoneinfo",
];

/// Whether this is a name such as `America/Los_Angeles` or `Etc/GMT+5`: letters, digits, and
/// `_ + - /`, in parts, with nothing that could climb out of the zone folder.
fn is_zone_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('/')
        && name.split('/').all(|part| {
            !part.is_empty()
                && part != ".."
                && part != "."
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_+-".contains(&byte))
        })
}

fn unix_zone_named(name: &str) -> Option<Vec<u8>> {
    if !is_zone_name(name) {
        return None;
    }
    ZONE_DIRS
        .iter()
        .find_map(|dir| std::fs::read(Path::new(dir).join(name)).ok())
}

impl Host for UnixHost {
    fn zone_named(&self, name: &str) -> Option<Vec<u8>> {
        unix_zone_named(name)
    }

    fn time_zone(&self) -> Option<TimeZone> {
        unix_time_zone()
    }

    fn home_dir(&self) -> Option<PathBuf> {
        env_path("HOME")
    }

    fn data_dir(&self) -> Option<PathBuf> {
        xdg_data_dir(std::env::var_os("XDG_DATA_HOME"), std::env::var_os("HOME"))
    }

    fn prefers_dark(&self) -> Option<bool> {
        let output = std::process::Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "color-scheme"])
            .output()
            .ok()?;
        gnome_scheme_is_dark(&String::from_utf8_lossy(&output.stdout))
    }

    fn open_path(&self, path: &Path) -> Result<(), String> {
        let tried: &[(&str, &[&str])] =
            &[("xdg-open", &[]), ("gio", &["open"]), ("kde-open5", &[])];
        if first_that_starts(tried, path.as_os_str()) {
            Ok(())
        } else {
            Err("Could not open that file.".to_owned())
        }
    }

    fn open_url(&self, url: &str) -> Result<(), String> {
        let tried: &[(&str, &[&str])] =
            &[("xdg-open", &[]), ("gio", &["open"]), ("kde-open5", &[])];
        if first_that_starts(tried, std::ffi::OsStr::new(url)) {
            Ok(())
        } else {
            Err("Could not open that link.".to_owned())
        }
    }

    fn choose_file(&self) -> Result<PathBuf, String> {
        let attempts: &[(&str, &[&str])] = &[
            ("zenity", &["--file-selection", "--title=Add a file"]),
            ("kdialog", &["--getopenfilename", ".", "All files (*)"]),
            ("yad", &["--file", "--title=Add a file"]),
        ];
        for (program, args) in attempts {
            if let Some(path) = ask_for_path(program, args)? {
                return Ok(path);
            }
        }
        Err("No file chooser is installed. Drop the file onto the window.".to_owned())
    }
}

/// macOS. Written from the system's documented behavior and not yet run on a Mac.
#[derive(Clone, Copy, Debug)]
pub struct MacHost;

/// `~/Library/Application Support`, where a Mac program keeps its own data.
#[must_use]
fn mac_data_dir(home: Option<OsString>) -> Option<PathBuf> {
    non_empty(home).map(|home| home.join("Library").join("Application Support"))
}

/// What `defaults read -g AppleInterfaceStyle` means. It prints `Dark` in dark mode and fails
/// in light mode, because the key is not set then.
#[must_use]
fn mac_style_is_dark(succeeded: bool, answer: &str) -> bool {
    succeeded && answer.trim().eq_ignore_ascii_case("dark")
}

impl Host for MacHost {
    fn zone_named(&self, name: &str) -> Option<Vec<u8>> {
        unix_zone_named(name)
    }

    fn time_zone(&self) -> Option<TimeZone> {
        unix_time_zone()
    }

    fn home_dir(&self) -> Option<PathBuf> {
        env_path("HOME")
    }

    fn data_dir(&self) -> Option<PathBuf> {
        mac_data_dir(std::env::var_os("HOME"))
    }

    fn prefers_dark(&self) -> Option<bool> {
        let output = std::process::Command::new("defaults")
            .args(["read", "-g", "AppleInterfaceStyle"])
            .output()
            .ok()?;
        Some(mac_style_is_dark(
            output.status.success(),
            &String::from_utf8_lossy(&output.stdout),
        ))
    }

    fn open_path(&self, path: &Path) -> Result<(), String> {
        if first_that_starts(&[("open", &[])], path.as_os_str()) {
            Ok(())
        } else {
            Err("Could not open that file.".to_owned())
        }
    }

    fn open_url(&self, url: &str) -> Result<(), String> {
        if first_that_starts(&[("open", &[])], std::ffi::OsStr::new(url)) {
            Ok(())
        } else {
            Err("Could not open that link.".to_owned())
        }
    }

    fn choose_file(&self) -> Result<PathBuf, String> {
        let script = r#"POSIX path of (choose file with prompt "Add a file")"#;
        ask_for_path("osascript", &["-e", script])?.ok_or_else(|| {
            "No file chooser is available. Drop the file onto the window.".to_owned()
        })
    }
}

/// A system this has no answers for. It gives a home folder if the environment names one and
/// refuses the rest, so the notebook still runs and the caller falls back.
#[derive(Clone, Copy, Debug)]
pub struct BareHost;

impl Host for BareHost {
    fn zone_named(&self, _name: &str) -> Option<Vec<u8>> {
        None
    }

    fn time_zone(&self) -> Option<TimeZone> {
        None
    }

    fn home_dir(&self) -> Option<PathBuf> {
        env_path("HOME").or_else(|| env_path("USERPROFILE"))
    }

    fn data_dir(&self) -> Option<PathBuf> {
        None
    }

    fn prefers_dark(&self) -> Option<bool> {
        None
    }

    fn open_path(&self, _path: &Path) -> Result<(), String> {
        Err("This system cannot open files from here.".to_owned())
    }

    fn open_url(&self, _url: &str) -> Result<(), String> {
        Err("This system cannot open links from here.".to_owned())
    }

    fn choose_file(&self) -> Result<PathBuf, String> {
        Err("This system cannot ask for a file from here.".to_owned())
    }
}

/// Mail for the relay this computer runs: a `relay` folder under the program's own data folder.
/// With no data folder, a folder beside where the program runs.
#[must_use]
pub fn loopback_store_dir(host: &dyn Host) -> PathBuf {
    host.data_dir().map_or_else(
        || PathBuf::from("relay-data"),
        |data| data.join("ambient").join("relay"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_keeps_its_data_under_xdg_or_local_share() {
        assert_eq!(
            xdg_data_dir(
                Some(OsString::from("/data")),
                Some(OsString::from("/home/me"))
            ),
            Some(PathBuf::from("/data"))
        );
        assert_eq!(
            xdg_data_dir(Some(OsString::from("")), Some(OsString::from("/home/me"))),
            Some(PathBuf::from("/home/me/.local/share")),
            "an empty value is not a value"
        );
        assert_eq!(
            xdg_data_dir(None, Some(OsString::from("/home/me"))),
            Some(PathBuf::from("/home/me/.local/share"))
        );
        assert_eq!(xdg_data_dir(None, None), None);
    }

    #[test]
    fn a_mac_keeps_its_data_in_application_support() {
        assert_eq!(
            mac_data_dir(Some(OsString::from("/Users/me"))),
            Some(PathBuf::from("/Users/me/Library/Application Support"))
        );
        assert_eq!(mac_data_dir(Some(OsString::from(""))), None);
        assert_eq!(mac_data_dir(None), None);
    }

    #[test]
    fn the_system_theme_is_read_from_what_each_system_says() {
        assert_eq!(gnome_scheme_is_dark("'prefer-dark'\n"), Some(true));
        assert_eq!(gnome_scheme_is_dark("'prefer-light'\n"), Some(false));
        assert_eq!(gnome_scheme_is_dark("'default'\n"), Some(false));
        assert_eq!(gnome_scheme_is_dark(""), None, "no answer is no answer");
        assert!(mac_style_is_dark(true, "Dark\n"));
        assert!(
            !mac_style_is_dark(false, ""),
            "light mode makes the command fail"
        );
        assert!(!mac_style_is_dark(true, "Light"));
    }

    #[test]
    fn the_relay_mail_goes_under_the_data_folder_whatever_the_system() {
        struct Fixed(Option<PathBuf>);
        impl Host for Fixed {
            fn home_dir(&self) -> Option<PathBuf> {
                None
            }
            fn data_dir(&self) -> Option<PathBuf> {
                self.0.clone()
            }
            fn prefers_dark(&self) -> Option<bool> {
                None
            }
            fn time_zone(&self) -> Option<TimeZone> {
                None
            }
            fn zone_named(&self, _: &str) -> Option<Vec<u8>> {
                None
            }
            fn open_path(&self, _: &Path) -> Result<(), String> {
                Err(String::new())
            }
            fn open_url(&self, _: &str) -> Result<(), String> {
                Err(String::new())
            }
            fn choose_file(&self) -> Result<PathBuf, String> {
                Err(String::new())
            }
        }
        assert_eq!(
            loopback_store_dir(&Fixed(Some(PathBuf::from(
                "/Users/me/Library/Application Support"
            )))),
            PathBuf::from("/Users/me/Library/Application Support/ambient/relay")
        );
        assert_eq!(
            loopback_store_dir(&Fixed(None)),
            PathBuf::from("relay-data")
        );
    }

    #[test]
    fn the_zone_name_is_what_follows_zoneinfo_in_the_link() {
        let name = |text: &str| zone_name_from_link(Path::new(text));
        assert_eq!(
            name("/usr/share/zoneinfo/America/Los_Angeles").as_deref(),
            Some("America/Los_Angeles")
        );
        assert_eq!(
            name("/var/db/timezone/zoneinfo/America/New_York").as_deref(),
            Some("America/New_York"),
            "a Mac keeps them elsewhere"
        );
        assert_eq!(
            name("../usr/share/zoneinfo/Etc/UTC").as_deref(),
            Some("Etc/UTC")
        );
        assert_eq!(name("/etc/some-other-file"), None);
        assert_eq!(name("/usr/share/zoneinfo/"), None);
    }

    #[test]
    fn only_a_plain_zone_name_is_looked_up() {
        for good in [
            "America/Los_Angeles",
            "Etc/GMT+5",
            "UTC",
            "America/Argentina/Buenos_Aires",
            "Asia/Ho_Chi_Minh",
        ] {
            assert!(is_zone_name(good), "{good}");
        }
        for bad in [
            "",
            "/etc/passwd",
            "../../etc/passwd",
            "America/../../x",
            "a//b",
            "America/",
            "a b",
            "a\\b",
            "a\nb",
            ".",
            "..",
            &"x".repeat(65),
        ] {
            assert!(!is_zone_name(bad), "{bad:?}");
        }
        assert!(BareHost.zone_named("America/Los_Angeles").is_none());
    }

    #[test]
    fn a_system_with_no_answers_refuses_and_does_not_panic() {
        let bare = BareHost;
        assert!(bare.data_dir().is_none() && bare.prefers_dark().is_none());
        assert!(bare.time_zone().is_none());
        assert!(bare.open_path(Path::new("/x")).is_err());
        assert!(bare.open_url("https://example.com").is_err());
        assert!(bare.choose_file().is_err());
    }
}
