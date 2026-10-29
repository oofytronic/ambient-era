# ambient-desktop

The Linux notes window. Iced draws the controls. There is no webview.

It calls [`Notebook`](../ambient-cli/src/session.rs) in `ambient-cli`, the same session as the CLI.

## Invariants

- No network. Do not add a relay client here.
- Do not put protocol rules in this crate. If a note can be saved, the CLI can save it too.
- The key file is not the OS keystore. Do not claim that it is.
- This is the Linux app. Do not turn it into the Mac or Windows shell.

## Dependencies

May depend on: `ambient-cli`. The window is a client, not a protocol crate, so it is excluded from `just wasm-check`.

## Test

The editor decisions are tested in `ambient-cli` (`Draft`). This crate is the window.

```
cargo run -p ambient-desktop -- path/to/folder
```

With no path it uses `./notebook`.
