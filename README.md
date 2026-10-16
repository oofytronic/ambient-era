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

Each release is a usable product on its own. Early versions are **TypeScript only**; Rust is added exactly once, in the core, at P2P/MLS.

| Version | Ships | Language |
|:--|:--|:--|
| **v0.1** | Key identity, local store, relay sync, 1:1 E2EE chat | TypeScript |
| **v0.2** | Groups (MLS), media, push | TypeScript |
| **v0.3** | Public profiles, follow graph, feed | TypeScript |
| **v0.5** | Desktop polish, self-hostable relay, data export | TypeScript (+ minimal Rust) |
| **v0.8** | P2P (iroh), Tor, relay blinding | Rust core |
| **v1.0** | Local agent, light docs, payments (tipping) | TypeScript + Rust |

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

**Principle: standards travel; platform APIs don't.** Implement the small, stable specs ourselves; depend only on the irreducible ones; keep crypto, storage, and transport behind interfaces so runtimes can be swapped per platform.

**Shells:** Expo / React Native (iOS, Android) and Tauri (desktop), both consuming **one shared TypeScript core**. Native-first — no PWA for now, but a future web shell must be a swap, not a rewrite.

| Layer | Choice | Standard |
|:--|:--|:--|
| Identity | Passkeys + PRF → Ed25519; mandatory mnemonic fallback | WebAuthn |
| Identifier | `did:key` | W3C DID |
| Crypto | WebCrypto (native on desktop, quick-crypto on mobile) | Ed25519, X25519, AES-GCM, HKDF |
| Data | DAG-CBOR + CID + signed Merkle DAG | IPLD / Multiformats |
| CRDTs | LWW map, OR-Set, RGA (own implementation) | — |
| Storage | SQLite + content-addressed blocks; keys in the OS keychain | — |
| Transport | WebSocket relay now → `iroh` later | QUIC |
| Group E2EE | MLS + HPKE | RFC 9420 / 9180 |
| Payments | Lightning | BOLT11 / LNURL |

**Four irreducible dependencies:** an MLS implementation, a P2P transport, a SQLite driver, and a native passkey module. Everything else is spec-level and self-written.

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

The rule that holds across every phase: **standards travel, platform APIs don't.** Keep the specs (DID, IPLD, MLS, WebAuthn), swap the runtime per platform, and add Rust exactly once — in the core, never in the UI.

## References

- [AnyType protocol](https://tech.anytype.io/any-sync/overview) · [`any-sync`](https://github.com/anyproto/any-sync)
- [Expo](https://expo.dev/) · [Tauri](https://v2.tauri.app/) · [Iroh](https://docs.iroh.computer/about/faq)
- [W3C DID](https://www.w3.org/TR/did-1.1/) · [IPLD](https://ipld.io/) · [MLS (RFC 9420)](https://www.rfc-editor.org/rfc/rfc9420.html)
- [Nostr](https://nostr.com/) · [Macro](https://macro.com/) (landscape)
