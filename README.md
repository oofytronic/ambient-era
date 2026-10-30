# Ambient Era

**An everything app you actually own.** Decentralized, local-first, and end-to-end encrypted for private Spaces — talk, post, organize, and (eventually) transact on rails no company controls.

> Signal's privacy, X's surface, AnyType's ownership — with an identity you hold as a key.

**Status:** Pre-0.1. The `ambient` CLI does offline notes, a second device, and one 1:1 chat through a loopback relay. The Linux window is still offline notes only. The relay binary binds `127.0.0.1` in the clear and is not a public deployment. The recovery seal is not implemented. Encryption is [ADR-0005](docs/adr/0005-private-space-encryption.md). Promises are [THREAT_MODEL.md](THREAT_MODEL.md). The decision log is [HISTORY.md](HISTORY.md).

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
| **v0.1** | Key identity, a personal Notes space, relay sync across the owner's devices, 1:1 E2EE chat (a Space of two), local search. Desktop first. |
| **v0.2** | Small private groups on the same sealing (not MLS), encrypted media, member management, abuse controls. Content-free push arrives with the first phone client. |
| **v0.3** | **Write:** long-form creations (books, series, newsletters), each its own Space. Markdown editor, media embeds, versions, links and backlinks, export. Short notes already exist from v0.1. |
| **v0.4** | **Weave:** chat about any creation, a creation-to-conversation sidebar, share a version into a conversation, capability-scoped local API |
| **v0.5** | One-command self-hostable relay, multi-relay redundancy, full data export and import |
| **v0.6** | **Publish:** public persona, profiles, follow graph, feed of published creations, Nostr bridge |
| **v0.8** | P2P (iroh), Tor, relay blinding |
| **v0.9** | Independent crypto and protocol review |
| **v1.0** | Local agent as a delegated key, tipping via external wallets, stable versioned protocol |

### v0.1 — notes and a private 1:1

v0.1 is **not** an everything app, and it does not run an agent. It is the owner's daily driver on desktop:

1. Create an identity. A passkey unlocks it. The recovery phrase is the backup.
2. Write notes in one Notes space. They work with no network.
3. Add the owner's other computer. Notes sync through a relay the owner runs.
4. Add one contact by QR code and chat. The chat is a Space of two.

No groups, no feed, no P2P, no phone push yet. The relay is untrusted and self-hostable. What "encrypted" means, and what it does not mean, is [THREAT_MODEL.md](THREAT_MODEL.md).

### v0.3–v0.4 — long-form writing, then weave

The notebook already exists. v0.3 adds creations that need their own membership: books, series, and newsletters, each its own Space, with versions and links between pieces. v0.4 interweaves that with conversation: from any chapter you can start a chat, and the creation shows those conversations in a sidebar. Copying a note into a chat copies the text. It does not hand over the Notes space. Only after that does anything become public (v0.6), starting with publishing creations rather than bare posts.

### v1.0 — the complete product

The point where the thesis is proven: sovereign *and* broad. v1.0 is a private messenger for small Spaces, plus a lightweight public social network, plus an owned second brain. Tipping goes through an external wallet. The app does not hold funds.

- Private groups on the same sealing as 1:1. Not rooms of hundreds, and not MLS.
- Public profiles with a portable identity, follow graph, and feed. Public Spaces are signed, not encrypted.
- Direct P2P sync, so the relay becomes optional when peers are reachable
- Tor / anonymity mode, which is the first time "anonymous" is an honest word
- A local agent that is a delegated key with a grant. It does not see a shared Space unless every member agreed.
- Long-form creations with conversations woven in, and basic tipping

**Explicitly out of scope for v1:** whiteboards, database UIs, algorithmic feeds, and very large encrypted groups. No ads, ever — sustainability comes from relay subscriptions, pro features, and self-hosting.

## Architecture

**Principle: standards travel; platform APIs don't.** One Rust core owns every long-lived concern (identity, events, Spaces, sync, storage, transport). Clients are thin and native to their platform. Full detail, rationale, and proof spikes are in [docs/STACK.md](docs/STACK.md).

| Piece | Choice |
|:--|:--|
| Core and relay | **Rust** |
| iOS | SwiftUI, core via UniFFI |
| Android | Jetpack Compose, core via UniFFI |
| Linux | Iced (Rust), a system window. Not a webview. |

| Layer | Choice | Standard |
|:--|:--|:--|
| Identity | Random root seed + mnemonic; rotatable key-event log; passkeys unlock locally | WebAuthn (unlock only) |
| Data | Deterministic CBOR, BLAKE3, signed per-device logs | RFC 8949, CDDL schemas |
| Crypto | RustCrypto: Ed25519, X25519, ChaCha20-Poly1305, HKDF, HPKE | RFC 9180 for sealing |
| Private Spaces | HPKE to each member device's current key. Not MLS. | [ADR-0005](docs/adr/0005-private-space-encryption.md) |
| Documents | v0.1 note body is a snapshot with an explicit conflict rule. A CRDT library (Loro or Automerge) is a later codec, not a v0.1 dependency. | [03-spaces.md](docs/spec/03-spaces.md) |
| Storage | SQLite + SQLCipher, FTS5 search, keys in the OS keystore | — |
| Transport | WebSocket relay now → `iroh` → Tor, behind a trait | QUIC |
| Payments | Tipping via external wallets (NWC, LNURL, Cashu); no embedded wallet | — |

The wire formats, identity log, Spaces model, and relay design are described in [VISION.md](VISION.md). Where that document still mentions MLS as the cipher, [ADR-0005](docs/adr/0005-private-space-encryption.md) wins.

## Repository and development

The Rust workspace under `crates/` is what `just ci` builds. Behavior that exists: event envelopes, the identity log through revoke, notes and conversation projection, HPKE seals, the relay JSON codec, an in-memory loopback relay, the `ambient` CLI, and the Linux notes window. `ambient-sync`, `ambient-capabilities`, `ambient-docs`, `ambient-core`, and `ambient-ffi` are still scaffolds. A wire-format change still starts in the spec and the vectors, not in a crate.

```
crates/        Rust workspace: ambient-events, -identity, -spaces, -sync, -store,
               -transport, -capabilities, -docs, -core, -ffi, -relay, -cli, -desktop
conformance/   language-neutral test vectors
docs/          STACK.md, spec/, adr/, TESTING.md
spikes/        throwaway experiments (not part of the build)
HISTORY.md     decision log
ROADMAP.md     order of work
THREAT_MODEL.md  what is promised
ARCHITECTURE.md  system map and v0.1 fence
CONTRIBUTING.md  how to change it
SECURITY.md    how to report a vulnerability
```

You need [rustup](https://rustup.rs/) (the compiler version is pinned in `rust-toolchain.toml`), plus three tools:

```
cargo install --locked just cargo-nextest cargo-deny
just ci        # format, lint, test, docs, wasm check, dependency policy
```

Contributor and agent rules are in [AGENTS.md](AGENTS.md). The system map is [ARCHITECTURE.md](ARCHITECTURE.md). Decisions are recorded in [docs/adr/](docs/adr/). How to work is in [CONTRIBUTING.md](CONTRIBUTING.md). Licensing is in [LICENSING.md](LICENSING.md), and security reporting is in [SECURITY.md](SECURITY.md).

## Hard problems

We're not pretending these are solved:

- **Distribution** — the thing that kills sovereign apps.
- **Key management UX** — passkey onboarding, mnemonic fallback, and social recovery, without a custodian.
- **Anonymity** — Tor is optional; until v0.8, relays still see IPs.
- **E2EE vs. agents** — local inference or encrypted compute; data never leaves the device in the clear.
- **Monetization without data** — relay subscriptions, pro features, self-hosting.

## Beyond 1.0

v1.0 is the proof, not the destination. The longer vision, in brief:

| Version | Theme | What it becomes |
|:--|:--|:--|
| **v1.5** | Scale & polish | Fast, everywhere, recommendable to a normal friend: multi-device, relay federation, push infrastructure, Signal/Telegram/WhatsApp import. |
| **v2.0** | Platform | An operating layer: sandboxed, permission-scoped mini-apps and an extension SDK running on your data with your identity. |
| **v2.5** | Communities & credentials | Orgs, roles, moderation, and portable reputation via Verifiable Credentials. |
| **v3.0** | Sovereign internet suite | Calendar, storage, media, forms, wallet, plus bridges to Nostr, ATProto, and IPFS. |
| **v3.5** | Ecosystem & governance | Independent clients, spec stewardship, grants, audits, and governance that doesn't depend on any one founder. |
| **v4.0+** | Ambient sovereignty | Device mesh, hardware keys, offline-first everything, one-click migration off Big Tech. |

The rule that holds across every phase: **standards travel, platform APIs don't.** Keep the specs (events, identity, Spaces, mailboxes), swap the runtime per platform, and keep the logic in the Rust core, never in the UI.

## References

- [AnyType protocol](https://tech.anytype.io/any-sync/overview) · [`any-sync`](https://github.com/anyproto/any-sync)
- [Iced](https://iced.rs/) · [UniFFI](https://mozilla.github.io/uniffi-rs/) · [Iroh](https://docs.iroh.computer/about/faq) · [HPKE (RFC 9180)](https://www.rfc-editor.org/rfc/rfc9180.html)
- [W3C DID](https://www.w3.org/TR/did-1.1/) · [IPLD](https://ipld.io/) · [SimpleX](https://simplex.chat/) (mailbox prior art, not the data model)
- [Nostr](https://nostr.com/) · [Macro](https://macro.com/) (landscape)
