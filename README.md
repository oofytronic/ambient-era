# Ambient Era

**An everything app you actually own.** Decentralized, local-first, anonymous, and end-to-end encrypted — talk, post, organize, and (eventually) transact on rails no company controls.

> Signal's privacy, X's surface, AnyType's ownership — with an identity you hold as a key.

**Status:** Pre-0.1, planning and scaffolding. **Current focus:** the road to v1.0.

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

Each release is a usable product on its own. The core and relay are Rust from day one; clients are native to each platform (see [Architecture](#architecture)).

| Version | Ships |
|:--|:--|
| **v0.1** | Key identity, local store, relay sync, 1:1 E2EE chat (a Space of two), push |
| **v0.2** | Groups (MLS), media, member management, abuse controls |
| **v0.3** | Public persona, profiles, follow graph, feed, Nostr bridge |
| **v0.4** | Notes and documents, local search, capability-scoped local API |
| **v0.5** | Desktop polish, one-command self-hostable relay, data export |
| **v0.8** | P2P (iroh), Tor, relay blinding |
| **v0.9** | Independent crypto and protocol review |
| **v1.0** | Local agent, tipping via external wallets, stable versioned protocol |

### v0.1 — a sovereign 1:1 messenger

v0.1 is **not** an everything app. It's a private messenger you can start in 30 seconds with no phone number, email, or account:

1. Create an identity with a passkey
2. Save your recovery phrase
3. Add a contact by QR code
4. Chat end-to-end encrypted — offline-capable, synced across two devices

No groups, no feed, no P2P. Relay-only, and the relay is untrusted and self-hostable.

### v1.0 — the complete product

The point where the thesis is proven: sovereign *and* broad. v1.0 is the sovereign alternative to Signal, plus a lightweight public social network, plus an owned second brain, with a wallet.

- Group chat on MLS, scaling to hundreds per group
- Public profiles with a portable identity, follow graph, and feed
- Direct P2P sync, so the relay becomes optional when peers are reachable
- Tor / anonymity mode
- A local AI agent that reads your encrypted data without sending it away
- Light document objects and basic tipping

**Explicitly out of scope for v1:** whiteboards, database UIs, algorithmic feeds, and very large groups. No ads, ever — sustainability comes from relay subscriptions, pro features, and self-hosting.

## Architecture

**Principle: standards travel; platform APIs don't.** One Rust core owns every long-lived concern (identity, events, Spaces, sync, storage, transport). Clients are thin and native to their platform. Full detail, rationale, and proof spikes are in [docs/STACK.md](docs/STACK.md).

| Piece | Choice |
|:--|:--|
| Core and relay | **Rust** |
| iOS | SwiftUI, core via UniFFI |
| Android | Jetpack Compose, core via UniFFI |
| Web and desktop | Dioxus (Rust), core linked directly (WASM on web, native on desktop) |

| Layer | Choice | Standard |
|:--|:--|:--|
| Identity | Random root seed + mnemonic; rotatable key-event log; passkeys unlock locally | WebAuthn (unlock only) |
| Data | Deterministic CBOR, BLAKE3, signed per-device logs | RFC 8949, CDDL schemas |
| Crypto | RustCrypto: Ed25519, X25519, ChaCha20-Poly1305/AES-GCM, HKDF | — |
| Group E2EE | MLS (OpenMLS) + HPKE; every conversation is a Space | RFC 9420 / 9180 |
| Documents | CRDT library (Loro or Automerge) for notes only | — |
| Storage | SQLite + SQLCipher, FTS5 search, keys in the OS keystore | — |
| Transport | WebSocket relay now → `iroh` → Tor, behind a trait | QUIC |
| Payments | Tipping via external wallets (NWC, LNURL, Cashu); no embedded wallet | — |

The wire formats, identity log, Spaces model, and relay design are described in [VISION.md](VISION.md).

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

The rule that holds across every phase: **standards travel, platform APIs don't.** Keep the specs (DID, IPLD, MLS, WebAuthn), swap the runtime per platform, and keep the logic in the Rust core, never in the UI.

## References

- [AnyType protocol](https://tech.anytype.io/any-sync/overview) · [`any-sync`](https://github.com/anyproto/any-sync)
- [Dioxus](https://dioxuslabs.com/) · [UniFFI](https://mozilla.github.io/uniffi-rs/) · [Iroh](https://docs.iroh.computer/about/faq) · [OpenMLS](https://openmls.tech/)
- [W3C DID](https://www.w3.org/TR/did-1.1/) · [IPLD](https://ipld.io/) · [MLS (RFC 9420)](https://www.rfc-editor.org/rfc/rfc9420.html)
- [Nostr](https://nostr.com/) · [Macro](https://macro.com/) (landscape)
