# History

A log of meaningful decisions. Newest first. This is not a commit changelog.

When a turn changes direction, scope, or a locked design choice, add an entry here in the same change. Link the ADR or spec that records it. Do not relitigate an entry in a later one; supersede it.

## 2026-11-12 — a plain word stays on the line

The painter only recorded marks. A plain word left the caret and was not drawn, so the next word started at the left edge. The page now draws the unmarked text as well. The saved text is unchanged.

## 2026-11-12 — a settled word leaves the marks

The page draws a word as soon as its format is closed, one word at a time, and the caret stays in the gap after it. A click in a settled word puts the caret in that word and leaves the marks hidden. A heading is a real size, including on the word being typed. A strike is a line through the letters once that word has settled. The saved text is still the markdown, marks included. This supersedes the lines that said a heading is not a larger size and that the editor cannot change the size of one span.

## 2026-11-12 — settings is a set of cards, and the gear is a real icon

The welcome screen sits in the middle of the window. Settings is separate cards, not one tight list. The gear is Lucide's settings icon. The window title is only Aerodash. The small role next to the name is gone.

## 2026-11-11 — names stay on this computer

The window opens on the word Aerodash, one letter at a time, over the two lines. That stays up for at least three seconds and until the folder has opened. A folder with no key asks for a device name, a personal name, and light, dark, or system. An existing key does not. The device name is the key-file label, set once, 1 to 32 bytes. The personal name and the color are a local `profile` file. Nicknames are a local `names` file. Neither is a record. The contact card is still four fields and still has no name, so a preferred username cannot be projected yet. What you call someone is only on this computer, and their key stays visible. Devices, recovery, and erasing the folder live in settings. Erase deletes the folder's children and starts the opening again. Saving a note runs off the typing thread. The editor is not reloaded when the seal finishes.

## 2026-11-11 — a note is filed by a tag in its text

Notes are grouped by `#tag` in the body. A heading is not a tag. Fenced and inline code are not tags. The list is a view: there is no folder record, because a note is a snapshot and a tag syncs with it. `#clinic/billing` is one tag. The note screen no longer keeps a task editor under the page. A task is still the toolbar button, and the Tasks screen is still the same checkbox lines. New, delete, and send are small round buttons at the bottom right of the note. On a narrow window the new button sits on the list the same way. A pause writes the note on this computer even when the relay is off. Scrollbars are a thin thumb with no track.

## 2026-11-11 — a note is deleted by a later snapshot

The notes tab creates, opens, edits, and deletes a note. Delete asks first. The write is still `note.put`: the body is only `%%ambient-deleted%%`, and the list hides the note when every head is that snapshot. The older text stays in the log. It is not a new event type.

Tasks in the open note are the checkbox lines already there. Add, rename, check, and remove write that line. Enter saves a renamed line. Backspace is not involved. The Tasks screen is still the same view across notes.

## 2026-11-11 — enter continues a list, and a heading stays one size

Enter on a list or a task inserts the next marker, and splits the line if the caret is in the middle. A new task is unchecked. Backspace on an empty item removes the marker. This is still the same checkbox line, not a new record.

A heading is not a larger size. The editor's highlight format is only a color and a font. The text engine under it can size one span, but iced 0.14 does not pass a size through, and this window does not fork iced. `#` and `##` use the serif. Smaller headings are bold. A strike is still a color, not a line through the letters.

## 2026-11-11 — the note owns its toolbar, and a loose task is still a line

The format row sits in the note column, above the editor, not above the list. On a narrow window it stays at the bottom. The buttons are labeled Word, Line, and Insert. A word button wraps the word or the selection and toggles off. A line button toggles the prefix. An insert drops a link, table, rule, callout, or code block. The preview also reads comments, math, tags, embeds, callouts, and footnote definitions. The editor still cannot change the size of one span, and a strike is still a color, not a line through the letters.

A task added on the Tasks screen is a `- [ ]` line in a note titled Tasks. That note is created the first time. It is not a new record. Checking a box still writes the line back. Send to chat asks which conversation, then waits for Send. Chat bubbles stay inside the pane as the window gets narrower, and the words wrap. Opening the window draws two lines longer than the frame, drifting across it like a breeze, for a moment.

## 2026-11-10 — the window fits a small frame, and a chat file stays local

The note editor covers the marks people actually type: headings, emphasis, strike, highlight, code, quotes, lists, tasks, tables, links, images, and wikilinks. The marks still stay in the text. A heading is still not a larger font. The editor cannot change the size of one span.

A format row sits under the top bar when the window is wide, and at the bottom when the window is narrower than 720. Notes, Tasks, and Chat move to that bottom bar too. A wide window keeps the list beside the page. A narrow or square window shows the list, then the page.

A file, picture, or video added to a chat is copied into the notebook folder. The message cites it. The citation can sync. The bytes do not. That is not a new record. Encrypted attachment bytes stay later work. The bubble says when the file is not on this computer. A video opens with the system. It is not played inside the window.

## 2026-11-10 — a folder can hold a relay

The window is no longer hard-wired to `127.0.0.1`. The folder file `relays` is this computer, the introduction list, or a pin and a backup. The list is empty until a real relay exists, so the default stays this computer. `ws://` is refused off this machine. `wss` uses rustls. The relay process terminates TLS from a PEM the operator supplies. Pushes and cursors are per relay, so a dead relay is not treated as delivered. A checkmark means the note is saved on this computer. The globe stays gray, red, or green. The address is behind Advanced. The replacement test, with a second computer, is not done.

## 2026-11-09 — the relay keeps its mail

The loopback relay writes a `mailboxes` file and loads it on the next start. Cursors are not reused. A file that does not parse is refused and left alone. The window's file is under `~/.local/share/ambient/relay`. This is still cleartext on this computer. It is not a public relay. See [docs/network/README.md](docs/network/README.md).

## 2026-11-09 — connection is its own track

The relay on this computer is not the network. The protocol already is: identity, Spaces, sealed logs, mailbox ids. The missing piece is a connection a second computer can use without an address.

That work is [docs/network/README.md](docs/network/README.md), inside this repo, not a second project and not a second protocol. Spec 04 still wins on bytes. The owner started it before the v0.1 exit. It does not pull groups, Tor, payments, or a directory forward. v0.5 remains the later self-host and export.

An introduction list run by the owner is a convenience, not an authority. The claim worth making is that the relay can be swapped and cannot read, and that the notebook stays on the device.

## 2026-11-09 — tasks are a view, and the marks have to read

The Tasks screen lists `- [ ]` lines from notes. Checking a box writes that line back into the note. It is not a new record. A note with two versions is left alone until someone saves one.

In the editor, a heading is the serif face, code is a warm monospace, strike is a different color, and a finished task line goes quiet. The marks stay. The editor cannot change the size of one span, so a heading is not a larger font.

## 2026-11-08 — a feature is a room, a record, or a view

Before adding a kind of thing, read "Adding a feature" in [ARCHITECTURE.md](ARCHITECTURE.md). A room is a Space (who can have the log). A record is one signed event (how two edits merge). A view is a local picture of records and is not synced. Most new screens are views over documents. A new record is only for a different merge rule. A new room is only for a different guest list. Not an object graph.

The note body is one editor. Markdown is styled as it is typed: opening `**` starts bold, and the closing pair ends it, including across lines until a blank line. The marks stay in the text. That text is what is saved. This supersedes the click-a-line preview from 2026-11-06.

## 2026-11-06 — crates only when they are the standard

Dependencies stay on a short list. A new crate is allowed only when it is crucial and the standard for that job: ubiquitous, as small as the job allows, and actually fast. Iced, the audited crypto crates, and the protocol libraries already chosen stay. A convenient or merely popular crate does not. Build the feature. The note preview is the example: a small parser in the window, not a markdown crate. See [ADR-0003](docs/adr/0003-workspace-and-tooling.md).

## 2026-11-06 — markdown preview, chat in time order

The note body previews a small markdown set: headings, emphasis, lists, task boxes, quotes, code, links, and rules. A line shows the rendered text until you click it, and then the marks come back. Enter or an arrow leaves that line and renders it. What is saved is still the raw markdown. Chat messages are reading order by the advisory clock, oldest first, so the latest sits at the bottom. The prev chain is unchanged, and the clock is still not a protocol order. Your messages sit on the right. Theirs sit on the left.

## 2026-11-05 — Aerodash, and a click does not re-read the log

The window is named Aerodash. Choosing a note, a chat, or Devices reuses the projection already in memory. A sync that brought nothing does not rewrite the sealed file and does not reload the window. Search keeps the SQLCipher index open instead of deriving the key again on every letter.

## 2026-11-05 — typing, notes on the other folder, parchment

Typing does not wait on sync. The window runs sync off its own thread, and a short pause writes the open note without moving the cursor, so a new note or an edit reaches the paired folder without a separate Save. A note that is already saved refreshes when the other folder's version arrives. The window is parchment: light `#EDEADE` with `#16446F`, dark `#1C1917` with `#66A5E1`. Titles are Crimson Pro. The rest is DM Sans. A sun or moon beside the globe switches them. Notes and chats in the list are rows, not the same control as Save, Send, or New note.

## 2026-11-04 — globe, auto-sync, drafts, restore enrolls

The window no longer has a Sync button or a relay button. A globe at the right of the toolbar is gray when the relay is off, green when it is on and nothing is unsent, and red when it is on and something has not been pushed. Clicking it opens a popover under the icon, over the notes, with the last sync line and a control to start or stop the relay this window started. The relay starts when the folder opens, and the window syncs on its own while that relay is on. The relay is still cleartext on `127.0.0.1`. It is not a public server.

An unsaved note is kept when you switch notes, start a new note, or open chat. Restore from the phrase still does not recreate old device keys. It appends `id.device-add` and one `space.member-add` signed by the identity key, so the new folder can sync. See [spec 02](docs/spec/02-identity-log.md).

## 2026-11-04 — revoke, writers, and more than one conversation

The root window lists devices and can revoke one. Revoke keeps the notes that device already wrote and stops sealing new events to it. The note list and the editor name the device that wrote the current version. A folder can hold more than one conversation. Each conversation has its own contact key. The first one still lives in `chats`. Later ones live under `conversations/`.

## 2026-11-04 — user-owned only if the later problems stay grants

The later system is still user-owned only if recovery, node payment, agent power, sensing, sleeping nodes, bootstrap, and abuse handling stay inside Spaces and grants. A phrase is the v0.1 backup, not the long-term recovery. No single guardian, operator, or agent may seize the root. See [ARCHITECTURE.md](ARCHITECTURE.md). None of this is v0.1 work.

## 2026-11-02 — one primitive, local unlock, message time, search index

Agents must keep one membership model: a new object is a Space of the same kind, a capability grant, or a copy. Not a second sharing protocol. See [AGENTS.md](AGENTS.md) and [ARCHITECTURE.md](ARCHITECTURE.md).

Message events now set envelope field 4 (advisory unix milliseconds). The window shows that time. It is not an order.

The key file can be wrapped with Argon2id and ChaCha20-Poly1305. The passphrase unlocks the file in this process. It is not the root and not a WebAuthn passkey. The Linux window has no platform passkey API.

Native search builds a SQLCipher FTS5 index beside the sealed log. The log is still the source of truth. `ambient-store` stays a sealed file so the protocol crates still compile for wasm.

## 2026-11-01 — contact invite is a link or a QR

The notes window shows a shareable `ambient://contact/` link and a QR of that link. Pasting the link or the raw card starts a conversation. A selected note can be copied into the chat as text. The recovery phrase and restore remain in the window.

## 2026-10-31 — Recovery seal and phrase restore

The root device seals the identity log and Notes-space events to the identity seal key (HKDF `identity-seal`) and puts the blobs in the recovery mailbox (HKDF `recovery-mailbox`). A new folder can be created from the BIP39 phrase of the root seed; it fetches and opens those blobs. Device keys are still not derived from the root. See [spec 02](docs/spec/02-identity-log.md).

## 2026-10-29 — The notes window syncs on loopback

The Linux window stayed offline so protocol rules would not land in `ambient-desktop`. The owner is testing the relay, a second device, and 1:1 in that window, so the window now calls the client in `ambient-cli`. It still does not seal events or parse relay JSON. The relay it can start is cleartext on `127.0.0.1`, which [spec 04](docs/spec/04-relay-api.md) says is not a public deployment. Recovery, a QR invite, revoke, and copying a note into chat are still not done.

## 2026-10-28 — Linux is an Iced window, not a webview

The owner corrected what "native" means for Linux. It means a real desktop window, not a page drawn by WebKit. It does not mean the app has to look as if GNOME or KDE made it.

[ADR-0002](docs/adr/0002-rust-core-native-clients.md) is amended. The Linux client is Iced. Mac, Windows, iPhone, and Android stay SwiftUI, WinUI, SwiftUI, and Compose. The web client is a PWA and only a fallback. Dioxus is not a client. The notes window in `ambient-desktop` is the Iced app over the same folder the CLI uses.

## 2026-10-27 — Roadmap

[ROADMAP.md](ROADMAP.md) is the sequence. v0.1 is specified and not yet implemented. Later versions do not start until the previous one is in use. The README table is the short form of that file.

## 2026-10-27 — v0.1 specs, before any protocol code

Wrote the four v0.1 specs and the first conformance vectors.

- [docs/spec/01-events.md](docs/spec/01-events.md): canonical CBOR envelope, BLAKE3 ids, Ed25519 over the unsigned bytes, per-signer chains.
- [docs/spec/02-identity-log.md](docs/spec/02-identity-log.md): root seed, HKDF to an identity key, a seal key, and a recovery mailbox. Device keys are not derived. Pairing does not copy the root.
- [docs/spec/03-spaces.md](docs/spec/03-spaces.md): Notes and conversation Spaces, HPKE to member keys, `text-snapshot-v1` with an explicit conflict rule. Not a CRDT, and not MLS.
- [docs/spec/04-relay-api.md](docs/spec/04-relay-api.md): mailbox id is the capability. The relay stores opaque blobs.

Also added [ARCHITECTURE.md](ARCHITECTURE.md), [CONTRIBUTING.md](CONTRIBUTING.md), and [docs/TESTING.md](docs/TESTING.md). Rewrote [SECURITY.md](SECURITY.md) so it points at the threat model.

Not done: a second implementation of the vectors, and any crate behavior. Spec 05 (long-form creations) is intentionally unwritten.

## 2026-10-27 — Spaces stay, MLS does not

The owner and Grok reconciled the written plan (README, VISION, STACK, ADR-0001 through ADR-0004, the OpenMLS spike) with a later decision against MLS.

Kept:

- A Space is a membership list plus the merge of each member's signed log.
- SimpleX contributes one-way mailbox addresses only. It is not a replacement for Spaces.
- Rust core, Dioxus on desktop first, native mobile later.
- Key-event log. Passkeys unlock the seed. They are not the root.
- Chat, membership, and feeds are logs. A CRDT is for document bodies only.
- The relay stores ciphertext and must not learn a global identity.
- No wallet. No agent runtime in v0.1. An agent, later, is a delegated key with a grant. A shared Space still needs every member's consent before an agent can read it.

Changed:

- MLS is not the cipher for a private Space. [ADR-0005](docs/adr/0005-private-space-encryption.md) supersedes that sentence wherever VISION, STACK, or a crate rule still said it. The OpenMLS spike is kept as evidence and marked not adopted.
- A private Space seals each event to the members' current device keys with HPKE (RFC 9180). There is no second, hand-rolled ratchet.
- v0.1 is the owner's Notes space and 1:1 conversations, on desktop first. Personal notes are not deferred to v0.3. Phone push waits for the phone client.
- v0.1 promises content confidentiality and unlinkability only. See [THREAT_MODEL.md](THREAT_MODEL.md).

Not done yet:

- The normative specs in `docs/spec/` (events, identity log, Spaces, mailbox API) and their conformance vectors. No crate behavior until those exist.
