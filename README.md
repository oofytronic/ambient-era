# Ambient Era

**An everything app you actually own.** Decentralized, local-first, and end-to-end encrypted for private Spaces — talk, post, organize, and (eventually) transact on rails no company controls.

> Signal's privacy, X's surface, AnyType's ownership — with an identity you hold as a key.

**Status:** v0.1, declared by the owner on 2026-11-20: notes on one computer. The Linux window, Aerodash, does notes, tasks, tags, search, and spelling with no network. A second folder, one 1:1 through a loopback relay, and device revoke are built and tested, and they are v0.2, not what v0.1 promises. The note list shows which device wrote each note. The relay is cleartext on `127.0.0.1` and is not a public deployment. The recovery seal is implemented. The window shows the BIP39 phrase and can restore from it. Contact invites are a link or a QR. Chat lines show the advisory time from the event envelope. The key file can be wrapped with a passphrase; that is a local unlock, not a platform passkey. Native search is a SQLCipher FTS5 index beside the sealed log. Spelling checks against a bundled English dictionary and a personal one that stays on this computer. A focus mode hides everything but the writing. What changed most recently, and what is still open, is in [docs/session-2026-11-19-to-20.md](docs/session-2026-11-19-to-20.md) and [docs/open-items.md](docs/open-items.md). Encryption is [ADR-0005](docs/adr/0005-private-space-encryption.md). Promises are [THREAT_MODEL.md](THREAT_MODEL.md). The decision log is [HISTORY.md](HISTORY.md).

---

## The idea

The field splits three ways:

| Camp | Examples | What's missing |
|:--|:--|:--|
| Centralized super apps | X, WeChat, Meta | Sovereignty. Cloud-hosted, account- and KYC-bound, not E2EE. |
| Cloud workspaces | Notion, Macro, ClickUp | Sovereignty. No E2EE; the vendor holds your data. |
| Sovereign / local-first | AnyType, AFFiNE, Nostr | Breadth. Narrow (docs, or social only), weak multiplayer, no economy. |

The first two have breadth without sovereignty. The third has sovereignty without breadth. **The empty quadrant is sovereign *and* broad** — that's what Ambient Era is for.

The closest prior art is [AnyType](https://tech.anytype.io/any-sync/overview), and it's the best teacher: BIP39/Ed25519 identity, a local encrypted store, encrypted CRDT sync with spaces and ACLs. But it's PKM-first, pseudonymous rather than anonymous (relays see your IP), and has no global social graph or economic layer. That's the seam. We borrow the ideas and build our own core rather than forking.

**North star:** a person runs their entire digital life — talk, post, organize, transact, create — on rails they own, with an identity that moves anywhere, and no company able to read, rank, or remove them.

The name is the endgame: *ambient sovereignty* (v4.0+), where owning your data and identity stops being a choice you make and becomes the floor you stand on.

## Who it's for first

Crypto natives, privacy-minded people, and Nostr/Farcaster users. They already hold keys, tolerate rough edges, and X will never satisfy them. Distribution is what kills sovereign apps, so the plan is to seed these communities and build bridges from Nostr and Telegram.

## The road to 1.0

Each release is a usable product on its own. The order, the exit test for v0.1, and what each version refuses to ship are in [ROADMAP.md](ROADMAP.md). The table below is the short form.

| Version | Ships |
|:--|:--|
| **v0.1** | Key identity, a personal Notes space, tasks and tags, local search, spelling, a recovery phrase. One computer, no network. Desktop first. |
| **v0.2** | A second computer through a relay, and 1:1 E2EE chat (a Space of two). Then small private groups on the same sealing (not MLS), encrypted media, member management, abuse controls. Content-free push arrives with the first phone client. |
| **v0.3** | **Write:** long-form creations (books, series, newsletters), each its own Space. Markdown editor, media embeds, versions, links and backlinks, export. Short notes already exist from v0.1. |
| **v0.4** | **Weave:** chat about any creation, a creation-to-conversation sidebar, share a version into a conversation, capability-scoped local API |
| **v0.5** | One-command self-hostable relay, multi-relay redundancy, full data export and import |
| **v0.6** | **Publish:** public persona, profiles, follow graph, feed of published creations, Nostr bridge |
| **v0.8** | P2P (iroh), Tor, relay blinding |
| **v0.9** | Independent crypto and protocol review |
| **v1.0** | Local agent as a delegated key, tipping via external wallets, stable versioned protocol |

### v0.1 — notes on one computer

v0.1 is **not** an everything app, and it does not run an agent. It is the owner's daily driver on one desktop:

1. Create an identity. A passphrase can lock the key file on this computer. The recovery phrase is the backup of the identity.
2. Write notes in one Notes space. Tasks are lines in notes. Tags, search, and spelling work. Nothing needs a network.
3. Use it every day. That is the exit.

**There is no copy of the notes off this computer in v0.1.** The recovery phrase brings back the identity. It brings back the notes only from a copy held by a relay that can be reached, and a relay that sends the owner's notes elsewhere is v0.2. Until then a lost disk is lost notes unless the owner copies the folder.

The second computer, the relay, 1:1 chat, device revoke, and recovery restore are built and tested in the app. They are not v0.1. They are the first half of v0.2, and they are not what v0.1 promises.

### v0.2 — a second computer and a private 1:1, then groups

First the second computer and the 1:1: a relay the owner can reach, a second folder paired to the first, one conversation with one contact by link or QR, an offline gap, and a restore from the recovery phrase. Then small private groups on the same sealing, encrypted media, and member management. The groups do not start until the 1:1 is in use. The relay is untrusted and self-hostable. What "encrypted" means, and what it does not mean, is [THREAT_MODEL.md](THREAT_MODEL.md).

No feed, no P2P, no phone push yet.

### v0.3–v0.4 — long-form writing, then weave

The notebook already exists. v0.3 adds creations that need their own membership: books, series, and newsletters, each its own Space, with versions and links between pieces. v0.4 interweaves that with conversation: from any chapter you can start a chat, and the creation shows those conversations in a sidebar. Copying a note into a chat copies the text. It does not hand over the Notes space. Only after that does anything become public (v0.6), starting with publishing creations rather than bare posts.

### v1.0 — the complete product

The agent is a local process that holds a delegated key and uses the same capability grants the owner already uses to share. It does not see the recovery phrase. Tipping goes through an external wallet the owner controls. The protocol version is frozen.

## Why now

The pieces exist. The gap is that no one has put them together under one identity the owner holds, with a private log that never leaves the devices, and a public surface that does not require a company to stay online.

## Status of the code

The Rust workspace under `crates/` is what `just ci` builds. Behavior that exists: event envelopes, the identity log through revoke, notes and conversation projection, HPKE seals, the relay JSON codec, a loopback relay with durable mailboxes, TLS to a relay, recovery from the phrase, the `ambient` CLI, and the Linux notes window. A signature is checked once per load or write, a write never replaces a file it did not read, and a sync merges again if the file moved while it waited. `ambient-sync`, `ambient-capabilities`, `ambient-docs`, `ambient-core`, and `ambient-ffi` are still scaffolds. A wire-format change still starts in the spec and the vectors, not in a crate.

## Layout

```
crates/           the workspace
docs/             specs, ADRs, spikes
conformance/      vectors the crates must match
ROADMAP.md        order of work
HISTORY.md        decision log
ARCHITECTURE.md   shape of the system
THREAT_MODEL.md   what each stage is allowed to claim
```

## Build

```
just ci
```

Or `cargo test --workspace`.

## License

Not decided. See [docs/adr/0004-licensing.md](docs/adr/0004-licensing.md).
