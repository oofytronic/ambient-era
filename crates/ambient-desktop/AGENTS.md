# ambient-desktop

The Linux notes window. The title is Aerodash. Iced draws the controls. There is no webview. The shell is notes and conversations. A note is one editor. Markdown is styled as it is typed: opening `**` starts bold and the closing pair ends it, across lines until a blank line. The marks stay in the text, and that text is what is saved. Chat messages are bubbles, yours on the right, in advisory-time order with the latest at the bottom. A globe at the right of the toolbar is the sync status: gray when the relay is off, green when it is on and caught up, red when it is on and something is unsent. A sun or moon beside it switches parchment light and dark. Clicking the globe opens a popover under the icon, over the notes, with the detail and the relay switch. The relay starts when the folder opens. Sync runs off the typing thread. A short pause writes the open note without moving the cursor. A click reuses the open projection until the sealed file changes. Invite, devices, and recovery are screens, not a permanent instruction column. A device list can revoke a key from the root folder. A note shows which device wrote it. An unsaved note stays when you leave it. List rows are not the same control as Save, Send, or New note.

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
