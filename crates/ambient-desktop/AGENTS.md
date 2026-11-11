# ambient-desktop

The Linux notes window. The title is Aerodash. Iced draws the controls. There is no webview. The shell is notes and conversations. A note is one editor. Markdown is styled as it is typed: headings, bold, italic, bold-italic, strike, highlight, code, quotes, lists, tasks, tables, links, images, and wikilinks. Opening `**` starts bold and the closing pair ends it, across lines until a blank line. The marks stay in the text, and that text is what is saved. Enter in a list or a task inserts the next marker. A new task is unchecked. Backspace on an empty item leaves the list. A format row lives in the note column, above the editor, when the window is wide, and at the bottom when the window is narrower than 720. It does not sit over the list. The buttons are grouped as Word, Line, and Insert. A word mark (bold, italic, strike, highlight, code) wraps the selection, or the word under the caret, and toggles off if that word is already wrapped. A line mark (heading, list, task, quote) toggles the lines the caret touches. An insert (link, wikilink, image, table, rule, callout, code block) drops that block. The marks stay in the text. That narrow layout is also the list-then-page layout, so the window can sit beside another program. Chat messages are bubbles, yours on the right, in advisory-time order with the latest at the bottom. A file added in chat is copied into the notebook and cited from the message. The citation can sync. The bytes cannot. A video opens with the system. It is not played in the window. A globe at the right of the toolbar is the sync status: gray when nothing is connected, green when a relay is up and caught up, red when a relay is up and something is unsent. On a narrow window the globe is on the bottom bar. A checkmark beside it means the open note is saved on this computer. Advanced, behind the globe, is where a pinned relay or a backup is typed. The default is this computer. A sun or moon beside it switches parchment light and dark. Clicking the globe opens a popover, with the detail and the relay switch. The relay starts when the folder opens. Sync runs off the typing thread. A short pause writes the open note without moving the cursor. A click reuses the open projection until the sealed file changes. Invite, devices, and recovery are screens, not a permanent instruction column. A device list can revoke a key from the root folder. A note shows which device wrote it. An unsaved note stays when you leave it. List rows are not the same control as Save, Send, or New note.

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
