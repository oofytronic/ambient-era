# ambient-desktop

The Linux notes window. Iced draws the controls. There is no webview.

It calls [`Notebook`](../ambient-cli/src/session.rs) and the sync helpers in `ambient-cli`. The folder is the same one the CLI uses.

## Invariants

- Do not put protocol rules in this crate. Sealing, event encoding, and the relay JSON stay in the crates that already own them.
- The only network is the loopback relay that client starts or dials. Cleartext on `127.0.0.1` is not a public deployment. The window must say so.
- Do not claim the key file is the OS keystore, or that a paired device holds the root.
- This is the Linux app. Do not turn it into the Mac or Windows shell.

## Dependencies

May depend on: `ambient-cli`. The window is a client, not a protocol crate, so it is excluded from `just wasm-check`.

## Test

Editor decisions are tested in `ambient-cli` (`Draft`). Sync and pairing are tested there too. This crate is the window.

```
cargo run -p ambient-desktop -- path/to/folder
```

With no path it uses `./notebook`.
