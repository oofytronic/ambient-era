# History

A log of meaningful decisions. Newest first. This is not a commit changelog.

When a turn changes direction, scope, or a locked design choice, add an entry here in the same change. Link the ADR or spec that records it. Do not relitigate an entry in a later one; supersede it.

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
