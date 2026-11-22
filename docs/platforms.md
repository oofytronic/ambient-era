# Platforms

How the same notebook runs on more than one kind of computer, and what a new platform has to supply. Written 2026-11-21 when the first step was taken: the questions the notebook asks of the machine are now one trait, [`Host`](../crates/ambient-cli/src/host.rs). The plan is in [STACK.md](STACK.md): one Rust core, and a thin native UI per platform. Iced is the Linux window only.

## The layers

| Layer | Crates | Knows about the platform? |
|:--|:--|:--|
| Protocol | `ambient-events`, `-identity`, `-spaces`, `-store`, `-transport`, `-relay` | No. They compile for the browser (`just wasm-check`), so they cannot. |
| Notebook | `ambient-cli` (the `Notebook`, sync, search, backup, the folder format) | Only through `Host`. |
| UI | `ambient-desktop` (Iced, Linux). Later SwiftUI on a Mac and iPhone, Compose on Android, WinUI on Windows. | Yes, that is its job. |

A folder made on one platform opens on another. The format, the specs, and the vectors are the same everywhere. A new UI is correct when it reads and writes the same folder and speaks the same wire as the others, whatever it looks like.

## The host

`Host` answers eight things: the home folder, the program's data folder, the time zone this computer is in, the zone file for any named zone, whether the system is dark, open a file, open a link, and choose a file. `host()` returns the one in use. It starts as the host for the system this was built for, and a platform shell replaces it with `set_host` before anything else runs.

| Host | For | State |
|:--|:--|:--|
| `UnixHost` | Linux and other freedesktop systems | What the Linux window ran before. Tested only on Linux. |
| `MacHost` | macOS | Written from documented behavior (`~/Library/Application Support`, `defaults read -g AppleInterfaceStyle`, `open`, `osascript`). Compiles everywhere. **Never run on a Mac.** |
| `BareHost` | anything else | Gives a home folder if the environment names one. Refuses the rest, so the notebook runs and the caller falls back. |

What is tested is the part that does not need a Mac: the path rules and the answers read from what each system prints are plain functions with tests (`xdg_data_dir`, `mac_data_dir`, `gnome_scheme_is_dark`, `mac_style_is_dark`, `loopback_store_dir`). The programs themselves are not run in any test.

The host holds no notes, keys, or text. It is given a path or a link and answers. Policy stays above it: which links the window will open (`files::link_allowed`) is not the host's call.

## Rules for the core, so it stays portable

1. No `std::env::var` for a home or data folder, no `Command::new`, and no program name outside `host.rs`. Ask the host. The one exception is a window starting another copy of itself (`spawn_at`), which uses `current_exe` and is the same on every system.
2. Do not assume a path separator, a home folder, or that a file has Unix permissions. `#[cfg(unix)]` is fine for setting mode `0600` on a key file, because macOS and iOS are Unix. Windows would need its own answer.
3. Anything a protocol crate does must compile for `wasm32-unknown-unknown`. Do not add a dependency to one of them that does not.
4. Do not put UI words, colors, or layout in the notebook layer. A message is a sentence a person can read, not a style.
5. A new platform question is a new method on `Host` with an answer for each host and a test of the part that can be tested. It is not a `cfg!` in the middle of other code.

## What still ties the notebook to Linux, and what is left

- **The time zone** is asked of `Host::time_zone`, which gives the bytes of a `TZif` file and a name. The Unix and Mac hosts read `/etc/localtime`. A host with no such file gives none, and the window uses UTC. A native shell can build the same bytes, or the window could take an offset table, if its system gives no file. The parser is `zone.rs` in the window and does not know where the bytes came from.
- **`/dev/urandom`** names a file in `files.rs` (`fresh_id`). It works on macOS. It would not on Windows, and an iPhone app may not be allowed to open it. It should use `getrandom`, which `ambient-cli` already depends on.
- **`ambient-cli` is a client library, not the core.** [ARCHITECTURE.md](../ARCHITECTURE.md) plans an `ambient-core` facade with `ambient-ffi` (UniFFI) over it for Swift and Kotlin. That crate is an empty scaffold. When it is built, the `Host` trait should move there, and a native shell should implement it as a UniFFI callback interface. Nothing is built for that yet.
- **The window is Linux-only and untouched.** Its fonts are bundled, so those carry over, but nothing in it has been tried on another system.
- **A shared design system** is not started. The plan is that every UI looks alike though built differently, which needs the colors, type sizes, spacing, and icon sizes to be written down once as plain values and read by each UI. Today they are constants in `main.rs`. That is the next thing to decide, not the next thing to build.
- **iOS** has a stricter sandbox: no `Command`, no shared home folder, and file access by system picker only. `Host` was shaped so those answers are possible, but the notebook folder's own location on iOS is not yet decided.
