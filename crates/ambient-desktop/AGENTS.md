# ambient-desktop

The offline notes window. It calls [`Notebook`](../ambient-cli/src/session.rs) in `ambient-cli`, which is the same session as the CLI.

## Invariants

- No network. Do not add a relay client here.
- Do not put protocol rules in this crate. If a note can be saved, the CLI can save it too.
- The key file is not the OS keystore. Do not claim that it is.
- Linux builds need WebKitGTK 4.1 (`libwebkit2gtk-4.1-dev`). macOS uses the system webview.

## Dependencies

May depend on: `ambient-cli`. The window is a client, not a protocol crate, so it is excluded from `just wasm-check`.

## Test

The editor decisions are tested in `ambient-cli` (`Draft`). This crate is the window. Run `cargo build -p ambient-desktop` on a machine with WebKitGTK.
