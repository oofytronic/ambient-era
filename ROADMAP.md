# Roadmap

The order of work. [ARCHITECTURE.md](ARCHITECTURE.md) is the shape of the system. [THREAT_MODEL.md](THREAT_MODEL.md) is what each stage is allowed to claim. This file is the sequence.

No dates. A stage starts when the previous one is actually in use, not when the code compiles. The owner declared v0.1 on 2026-11-20 and is daily driving it. v0.2 is the stage that starts now, and its first half is already built.

## Where this is

Specs 01 through 04 are written. `ambient-events` reproduces the event vectors. `ambient-identity` derives the root, signs genesis, and projects device add, inbox rotation, and revoke-by-ancestor. `ambient-spaces` projects a Notes space, including a conflict between two devices, and a conversation. The Linux window creates, edits, and searches notes, pairs a second folder, and exchanges one 1:1 through the loopback relay. The window is Iced, not a webview. The notes file is sealed with ChaCha20-Poly1305. The native client also keeps a SQLCipher FTS5 index beside that file. The key file can be wrapped by a local passphrase. That passphrase is not a platform passkey and not the OS keystore. HPKE seals events to other devices. The recovery seal is implemented: the root device seals the notes log to the identity seal key and the window can restore from the BIP39 phrase. A restored folder enrolls its new device. Contact invites are a link or a QR in the window. A note can be copied into a chat as text. The relay binds `127.0.0.1` and speaks cleartext, so it is not a public deployment. The window turns that relay on when the folder opens, syncs on its own, and shows status on a globe instead of a Sync button. Spec 05 is intentionally unwritten.

Since 2026-11-19, on `test`: a signature is checked once and the loaded notes are shared between `Notebook`s, so a save is about 30 ms at a thousand notes where it was 330. A write never replaces a file it did not read, and a sync, `say`, enroll, pair, and revoke merge again if the file moved. A note from a device that is not enrolled yet waits for its enrollment, where it used to be dropped for good ([ADR-0006](docs/adr/0006-wait-for-enrollment.md), proposed). The page parses and measures only what changed. The marks of the construct under the caret are shown whole. The window has a focus mode, tasks that update at once, a bundled dictionary and a personal one, and no visible scrollbars. `just lint` passes. The story is [docs/session-2026-11-19-to-20.md](docs/session-2026-11-19-to-20.md). What is open is [docs/open-items.md](docs/open-items.md).

## Connection

This is v0.2 work, started beside the notebook before v0.1 was declared. This is how a second computer syncs without anyone typing an address. The steps and the exit test are [docs/network/README.md](docs/network/README.md). It does not start groups, Tor, payments, or a directory. v0.5 is still the later one-command self-host and full export, not this MVP.

## v0.1 — notes on one computer

Declared by the owner on 2026-11-20. Desktop only. One person, one computer. No network. No agent, no groups, no phone push, no public feed.

| Step | Work | Done when |
|:--|:--|:--|
| 1 | `ambient-events` reproduces the event vectors | Done. Canonical CBOR, ids, and signatures match `conformance/vectors/events/` |
| 2 | Identity | Done. A random root with a recovery phrase, an identity log with genesis and a device, and a key file that a passphrase can wrap. |
| 3 | Local store and notes | Done. The `ambient` CLI creates, edits, and searches notes offline. The log is an encrypted file. Native search is a SQLCipher FTS5 index beside it. A save checks each signature once and never replaces a file it did not read. |
| 4 | Desktop app | Done. The Linux window, Aerodash, is Iced over the same folder as the CLI. Notes, tasks, tags, a markdown page that styles as it is typed, search, spelling with a personal dictionary, a focus mode. Not the OS keystore. |
| 5 | Back up | Done. Back up, in settings, copies the folder to another place, by hand. The copy has the key file, leaves out the export folder and the search index, is made whole before it is named, never overwrites an earlier one, and has the key file and the notes log compared with the originals. Nothing reminds the owner or runs it on a schedule. Restoring is copying the folder back. |

**Exit.** The owner uses it daily, on one computer, for real work. A green test run is not the exit. The owner declared it on 2026-11-20 and started that use. This file does not say the use is done.

**Still true at the end of v0.1.** The notes are on one disk until the owner makes a copy with Back up. Nothing makes one for them, and the recovery phrase does not bring the notes back by itself. A copy has the key file, so whoever has the copy can read the notes unless the key file is wrapped. A stolen or seized device whose key file is not wrapped, or that is unlocked, gives up the notes. The key file is not the OS keystore. The product is not anonymous. There is no agent, no sensing, and no public mirror.

## v0.2 — a second computer and a private 1:1, then groups

Everything in steps 6 through 8 is already built and tested, on two folders on one computer. None of it has been used between two machines. Build, and use, in this order. Do not skip ahead.

| Step | Work | Done when |
|:--|:--|:--|
| 6 | Relay and a second device | The window pairs a second folder and a note shows up there. The relay starts with the window and syncs on its own. It stores opaque blobs and binds `127.0.0.1` in the clear, which spec 04 says is not a public deployment. Two folders on one computer, not two machines. |
| 7 | 1:1 | The window opens a conversation, and can open another. Each one has its own contact key. Text is sealed to that key. The invite is a link or a QR. Messages show up after both windows sync. |
| 8 | Devices, activity, copy | Revoke a device from the root window. The note list shows which device wrote the current version. The editor's line for it was taken off the page on 2026-11-20 and comes back with a per-note info icon. Copy a note into a chat as text is in. Recovery restore from the phrase is in the window, and the restored folder enrolls its new device. |

The network track in [docs/network/README.md](docs/network/README.md) is how a second computer reaches the first without an address being typed. It is part of this stage.

**First exit, before any group.** The owner uses the notes on a second computer for real work, and uses 1:1 with at least one person, including an offline gap and one restore from the recovery phrase. A green test run is not the exit.

Then, and only then:

| Step | Work | Done when |
|:--|:--|:--|
| 9 | Small private groups | The same HPKE sealing. A numeric member cap in the spaces spec. Member remove and block. A rate limit on first contact. |
| 10 | Encrypted attachments | A file in a conversation reaches the other member as sealed bytes. Today the citation syncs and the bytes do not. |
| 11 | Content-free push | A phone client exists, and a push carries no content. |

Not in v0.2: rooms of hundreds, a group ratchet, MLS.

**Still true at the end of v0.2.** Concurrent edits of one note are kept as a conflict, not merged. A stolen device key can open what was already sealed to it. The product is not anonymous. Recovery is a phrase, not a threshold of guardians. The owner runs the relay; nobody is paid to keep a node up. There is no agent, no sensing, and no public mirror.

## Not a version yet

These are requirements for the system to stay user-owned after the shell exists. They are not permission to start. They do not add a second membership model. See [ARCHITECTURE.md](ARCHITECTURE.md).

- Social recovery: guardians or devices, a threshold, a timelock, no single party able to seize the root.
- Operators paid for relay, hosting, or compute. Most people stay clients. Reputation is uptime, not capacity.
- Agent grants that start empty, expire, and can be revoked, with a local audit log.
- Sensing that stays on the device unless one task explicitly sends it. No life log.
- Signed mirrors and interchangeable relays, so a sleeping node is normal.
- Bootstrap that survives the loss of the operator's introduction set.
- Client-side filters, shared blocklists, and node-local refusal. No central delete.

## After v0.2

Each row is a usable product. None of them is permission to start early.

| Version | Ships | Does not ship | Why it is next |
|:--|:--|:--|:--|
| **v0.3** | Long-form creations, each its own Space. Spec 05. Versions, links between pieces, Markdown export. | A CRDT, unless a spike has picked Loro or Automerge and a new payload type. Collaborative writing. | The notebook already exists. This is for a work that needs its own members. |
| **v0.4** | Weave. A creation shows the conversations about it. The capability API: a grant that can be narrowed, expired, and revoked. | An agent that uses those grants. A mini-app platform. | Sharing and a later agent must be the same permission, designed once. |
| **v0.5** | One-command self-host. More than one relay. Full export and import of the logs and the blobs. | A hosted service as the only way to sync. | Sovereignty you can check, not a claim in the README. |
| **v0.6** | A separate public identity. Profiles, a follow graph, a pull-based feed of published creations. A Nostr read and cross-post bridge. | An algorithmic feed. Encrypting the public log. Linking the public identity to the private one by default. | The first public thing is a published creation, not a bare post. |
| **v0.8** | Optional Tor. Direct connections when both peers are reachable, behind the transport trait. Relay blinding. | A promise of anonymity before this ships. | T3 in the threat model. |
| **v0.9** | An independent review of the protocol and the implementation. Findings fixed in public. | Calling the review a formality. | v1.0 cannot claim the threat model without this. |
| **v1.0** | A local agent that is a delegated key and a client of the v0.4 grants. Tipping through an external wallet. The protocol version frozen. | An agent in a shared Space without every member's consent. A wallet inside the app. An orchestrator that competes with vendor bots. | The thesis: private conversation, owned writing, and a public presence on one identity you can take elsewhere. |

## After 1.0

Themes only. They are not scheduled, and they do not change v0.1 or v0.2.

| | Theme |
|:--|:--|
| v1.5 | Polish a stranger can recommend. Import from other messengers. Push that stays content-free. |
| v2.0 | Mini-apps on the same grants the agent already uses. |
| v2.5 | Communities, roles, and portable credentials. |
| v3.0 | Calendar, files, and bridges to other networks. Still no embedded wallet unless the evidence says otherwise. |
| v4.0 | The ambient floor: mesh, hardware keys, and moving off a host without asking its permission. |

## Rules that apply at every version

- Spec and vectors before behavior.
- A private Space stays a membership list plus a commutative log, sealed with HPKE, until a new ADR says otherwise.
- The relay stores ciphertext. It is not given a user id.
- Do not call the product anonymous before v0.8.
- Do not run an agent before the grant API exists, and do not put one in a shared Space without every member's consent.
