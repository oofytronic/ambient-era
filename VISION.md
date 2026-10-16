# Vision: my take

This is an independent read of the original plan (the thesis, landscape, stack and roadmap that the [README](README.md) summarizes). I agree with the destination and disagree with several of the paths to it. This document is deliberately opinionated: where I diverge I say why, and where I'm unsure I say what experiment would settle it.

**How to read this.** Each section ends with a *Decision* or *Tripwire*. A decision is what I'd do today. A tripwire is the evidence that would make me change it.

---

## 1. What I'd keep

- **The thesis.** "Sovereign *and* broad" is an empty quadrant, and it's worth occupying.
- **Standards over platform APIs.** Specify small, stable formats; swap runtimes underneath.
- **One shared core, thin shells.** The UI must never own protocol logic.
- **Don't fork AnyType; borrow ideas.** Agreed.
- **Relays are untrusted, interchangeable, and self-hostable.**
- **No ads, no data-selling.** Monetize infrastructure, not people.
- **Honesty about hard problems.** The original's "Hard problems" section is its best part.

## 2. Where I'd diverge, in one page

| # | Original | My take | Why |
|:--|:--|:--|:--|
| 1 | `did:key` identity | A **rotatable identity log**; `did:key` only for device keys | `did:key` can't rotate. A lost or leaked key is a lost identity, which contradicts "an identity that moves anywhere". |
| 2 | Passkey PRF → Ed25519 root | Random root seed + mnemonic; passkeys only **unlock** it | PRF support is uneven, and synced passkeys quietly tie identity to an Apple/Google account. |
| 3 | "Anonymous" as a goal | Write a **threat model first**; promise *unlinkability by default, anonymity opt-in* | Anonymity is a property of transport + metadata, not of keys. Don't market what the architecture can't yet deliver. |
| 4 | v0.1 = 1:1 chat, groups in v0.2 | Primitive = **Space** (an MLS group). 1:1 is a space of two | One code path from day one. No rewrite when groups arrive. |
| 5 | Own CRDT implementations | **Signed append-only logs** for v1; adopt a CRDT library for documents | Chat and feeds are logs. Writing RGA yourself is a research project, not a v0.x task. |
| 6 | Expo/RN **and** Tauri | Spike **Tauri 2 on mobile too**; decide in week two | Two shells means two sets of native-module pain, solo. |
| 7 | Rust "exactly once, at v0.8" | Decide the core's language **now**, behind a conformance suite | The plan says TS-only to v0.5 but needs MLS at v0.2. That's a contradiction. |
| 8 | Push in v0.2 | Push is **v0.1** | A messenger without notifications is dead on arrival. |
| 9 | Local agent (v1.0) and mini-apps (v2.0) as separate features | One **capability-scoped local API** from the start | The agent *is* the first mini-app. Design the permission model once. |
| 10 | Payments via embedded Lightning | **Don't build a wallet.** Tipping via NWC / Cashu / LNURL | Custody, regulation, and mobile node UX will eat a solo dev. |
| 11 | Bridges to Nostr/ATProto at v3.0 | Nostr **read/post bridge by v0.3** | Distribution is the thing that kills sovereign apps; borrow an existing network. |
| 12 | Audits at v3.5 | **Independent crypto review before 1.0** | You can't ship E2EE to privacy people unaudited and call it 1.0. |

The rest of this document expands on these.

---

## 3. Start with a threat model, not a feature list

"Decentralized, local-first, anonymous, E2EE" are four different properties with four different costs. I'd write `THREAT_MODEL.md` before any code and tier the promises:

| Tier | Promise | Ships |
|:--|:--|:--|
| **T0 Content** | Relays and network observers cannot read messages. | v0.1 |
| **T1 Unlinkability** | No global user identifier. Contacts see pairwise pseudonyms; relays can't join your conversations into a graph by identity. | v0.1 by design |
| **T2 Metadata minimization** | Sealed-sender-style delivery, content-free push, padded sizes, jittered timing. | v0.2 to v0.5 |
| **T3 Network anonymity** | Relays can't see your IP (Tor / pluggable transport / mixnet). | v0.8, opt-in |
| **T4 Anti-coercion** | Duress PIN, hidden vaults, remote wipe. | Post-1.0 |

**Be explicit about what v1.0 is not**: it protects content and the social graph from the relay operator; it does not defend against a global passive adversary, a compromised device, or a state that blocks Tor. Say so in the README. Privacy users forgive limits; they never forgive overclaiming.

**Decision:** call the product *private and sovereign* until T3 is real. Reserve "anonymous" for the Tor mode.

## 4. Identity

Identity is the product. Everything else (chat, feed, docs) is an app on top of it, so it deserves the most careful design.

### Problems with `did:key` + passkey-PRF

1. **No rotation.** `did:key` *is* the public key. Compromise or upgrade means a new identity, losing every contact and follower.
2. **Passkeys aren't sovereign.** Most users' passkeys sync through iCloud Keychain or Google Password Manager. Deriving your root from one means your account is recoverable by, and revocable by, a company. That's the opposite of the pitch.
3. **PRF is not universal.** Support varies by OS, authenticator and browser/WebView. A design that depends on it needs the fallback to be the primary path anyway.
4. **One key doing everything** (signing, encryption, device auth) violates basic key hygiene.

### What I'd do

```
Root seed (32 random bytes, BIP39 mnemonic backup, stored in Secure Enclave/Keystore)
 ├─ Identity key        signs the key-event log; used rarely
 ├─ Recovery key(s)     pre-rotation commitments; offline / social recovery
 ├─ Device keys (N)     delegated by signed certificates; one per install, revocable
 └─ Per-contact / per-space pseudonymous keys    derived, so no global ID is ever shown
```

- The **identity** is the hash of a genesis event in a small **signed key-event log** (rotation keys, device delegations, relay/mailbox hints, revocations). Think `did:plc` or KERI-style pre-rotation, but self-contained: no registry, no blockchain. Publish a `did:` method for it later; start with a plain, documented format.
- **Passkeys and biometrics unlock the seed locally.** They are a convenience layer, never the root. Use PRF when available as an extra wrap, not as the derivation source.
- **Recovery** is layered: mnemonic, then optional social recovery (Shamir shares to trusted contacts), then optional hardware key. No custodian at any layer.
- **Two personas, unlinkable by default.** The *private* identity (chat, spaces) and the *public* identity (profile, feed) are different keys. Linking them is an explicit act by the user. The original plan promises both "anonymous" and "portable public profile + follow graph"; they only coexist if these are separate.
- Derive a **secp256k1 key** from the same seed for the Nostr bridge, since Nostr doesn't use Ed25519.

**Tripwire:** if a pre-rotation key-event log proves too heavy to specify well, fall back to `did:key` plus a *signed successor pointer* (key A announces "I am now key B") and accept weaker recovery for 0.x.

## 5. Core model: Spaces and logs

One primitive, used everywhere.

> **A Space is a membership-controlled group with an ordered, encrypted event stream.**

- A 1:1 chat is a Space of two. A group is a Space of N. A personal notebook is a Space of one (multi-device). A public profile is a Space with open read.
- **Encryption:** every private Space is an **MLS group** (RFC 9420). One code path; forward secrecy and post-compromise security come free; group size scales with the tree (O(log n) updates).
- **Events** are small, typed, signed objects (deterministic CBOR) addressed by hash. Each device appends to **its own signed log**; a Space is the merge of its members' logs, ordered with causal references. This is the Secure Scuttlebutt / p2panda / Willow lineage, which I'd study closely before inventing anything.
- **Identifiers:** BLAKE3 content hashes internally. Wrap in CIDs only at interoperability edges. Full IPLD/multiformats everywhere is more machinery than a solo dev needs on day one.
- **Documents and notes** (the "second brain") are the *one* place that wants real CRDTs. Use **Loro or Automerge** inside an event, not a hand-rolled RGA. Revisit owning it only if performance or size forces it.
- **Storage:** SQLite (events, indexes, FTS5 for local search) + encrypted blob store for media. Keys never live in SQLite; they live in the OS keystore.

**Why not general CRDTs everywhere?** Chat is append-only. A feed is append-only. Membership is a signed ACL log. Documents are the only genuinely concurrent-edit case. Don't pay CRDT complexity for data that doesn't need it.

## 6. Network: relays that know as little as possible

### The relay as mailbox, not as server

Borrowing from SimpleX's best idea: delivery uses **unidirectional, per-contact (or per-space) mailbox addresses** that are meaningless on their own and rotated over time, rather than "user X's inbox." A relay sees opaque blobs flowing between random addresses, not a social graph.

- Relays implement a tiny API: *put blob, get blobs since cursor, delete, subscribe.* Nothing application-specific.
- Mailbox hints live in the encrypted key-event log / Space state, so **moving relays costs nothing**: portability is real because identity never depended on a relay.
- Add **sealed-sender-style** submission and **padding** early; they're cheap and meaningful.
- **Push** (APNs/FCM) is a metadata leak by default. Send **content-free wake-ups** only, and support **UnifiedPush** on Android. Treat notifications as a first-class design problem, not a v0.2 feature.
- Write a transport interface (`send`, `receive`, `discover`) from day one with WebSocket as impl #1. Iroh/QUIC P2P, Tor, and a store-and-forward over LAN/Bluetooth are later impls, not rewrites.

### Federation and scale

- Relays are **stateless-ish blob stores sharded by mailbox ID**; scale horizontally by adding nodes. SQLite + Litestream is plenty for a single self-hosted relay; Postgres/object storage for a hosted fleet.
- **Federation** is simple when identity isn't bound to a relay: your Space lists several relays; clients publish to some and read from any. No relay-to-relay protocol is needed for v1. It's a client-side redundancy strategy.
- **Retention is a relay policy** (TTL for undelivered blobs, quota per mailbox), enforced without reading content.
- **Abuse** is the unglamorous killer of anonymous systems: spam, CSAM reports, harassment. Plan *now* for: capability-gated mailboxes (you can only write to an address someone gave you), per-mailbox rate limits, proof-of-work or token-gated first contact, and block/report flows that work on pseudonyms. A relay operator needs a defensible story.

## 7. Scaling: by dimension, not by version number

| Dimension | Approach |
|:--|:--|
| **Small groups (≤ ~200)** | MLS in a single Space; relay provides ordering. |
| **Large communities** | A *different primitive*: public, signed, unencrypted channels with moderator roles. E2EE doesn't scale socially past a certain size; admitting that early avoids contorting MLS. |
| **Social feed** | Pull-based. You fetch followed authors' logs from their declared relays. No global timeline index. **Discovery is an optional market** of indexers/curators that users can swap, never a built-in algorithm. |
| **History size** | Periodic signed **snapshots/checkpoints** per Space so new devices don't replay everything; blobs fetched lazily. |
| **Devices** | Device keys are delegated by the identity log; cap at ~5 in v1, lift later. Sync is just "read my other devices' logs." |
| **Operator cost** | Relays store ciphertext with TTLs; the dominant cost is media. Charge for relay quota (see §10), push the rest to the edge/P2P when peers are reachable. |
| **Team/solo scaling** | Spec-first and conformance-tested so that other people can implement clients and relays. This is also how it outlives its founder. |

## 8. Platform and language

### Shells: spike before committing

The original picks **Expo/React Native + Tauri**. For a solo developer this means two runtimes (Hermes vs. system WebView), two crypto/passkey/storage integration surfaces, and two UI codebases or a lowest-common-denominator abstraction.

**Tauri 2 now targets iOS and Android.** A single Tauri shell gives:
- one UI codebase (web tech, which a JavaScript dev ships fast in),
- real WebCrypto on mobile (the original notes RN lacks it),
- a **Rust backend available on every platform**, which is exactly where MLS, iroh and SQLite want to live anyway.

Costs: Tauri mobile is younger than React Native; WebView scrolling and gesture feel can lag native; background execution and push plugins need work.

**Decision:** in week two, build the *same* conversation screen (list, composer, offline send, one push) in both shells and decide with a stopwatch and a feel test. My prior is Tauri-everywhere for 0.x, with the headless core letting RN replace the mobile shell later *if* UX demands it.

### Core language: decide now

The original says "TypeScript until v0.5, Rust once at v0.8", yet needs MLS in v0.2 and a P2P transport at v0.8. In practice you'll hit a Rust boundary much earlier or adopt a TS MLS library (e.g. `ts-mls`; verify its maturity and audit status before betting on it).

My plan:

1. **v0.x core in TypeScript** behind narrow interfaces (`Identity`, `Log`, `Store`, `Transport`, `Group`).
2. **Language-neutral conformance vectors** from the first commit: golden bytes for events, signatures, key-log transitions, MLS interop transcripts. These are the real spec.
3. **Port to Rust when a tripwire trips**, not on a calendar: MLS lib unmaintained or unaudited; iroh needed; performance on low-end Android. The vectors make the port mechanical and de-risk the "only add Rust once" rule.

If the shell spike favors Tauri, the Rust side can host the crypto/transport sooner with zero extra distribution cost, so the tripwire may trip early. That's fine.

### Relay

TypeScript (Bun or Node), single binary/Docker image, SQLite by default. Self-hosting must be a one-command, ten-minute affair, or "sovereign" is marketing.

## 9. Revised roadmap to 1.0

Same destination, different slicing. Principle: **ship the primitive that everything else reuses, earlier.**

| Phase | Goal | Contents |
|:--|:--|:--|
| **0: Foundations** (weeks) | De-risk | Threat model; identity/key-log spec; event + wire formats; conformance vectors; shell spike; MLS library evaluation; minimal relay. |
| **0.1: Sovereign messenger** | A stranger can use it in 30 seconds | Seed identity + mnemonic, device pairing, **Spaces (MLS)** with 1:1 UI, relay mailboxes, **content-free push**, offline-first, two devices, local search. |
| **0.2: Groups & media** | Communities of friends | Group UI on the same Space primitive, encrypted media/blobs, member management, block/report, abuse controls. |
| **0.3: Public persona** | Surface area | Separate public identity, profiles, follow graph, pull-based feed, **Nostr bridge** (read + cross-post). |
| **0.4: Notes & capabilities** | The "second brain" begins | Document objects (CRDT lib), FTS search across Spaces, **capability-scoped local API** (UCAN-style grants). The permission model future mini-apps and the agent share. |
| **0.5: Own your infrastructure** | Sovereignty you can verify | Self-host relay (one command), multi-relay redundancy, full export/import, desktop polish. |
| **0.8: Transport hardening** | Anonymity becomes real | Iroh P2P, Tor/pluggable transports, relay blinding, token-gated anonymous relay payments. |
| **0.9: Review** | Earn trust | Independent crypto/protocol audit; fix findings; publish spec + threat model. |
| **1.0: Complete product** | Thesis proven | Local agent (a capability-scoped client of the 0.4 API), tipping via external wallets, docs, and a stable, versioned protocol. |

Notable shifts: groups ride on v0.1's primitive instead of arriving as a rebuild; capabilities land before the agent so the agent isn't a special case; the audit gates 1.0; payments shrink to tipping.

## 10. Monetization and sustainability

- **Relay subscriptions** are the right model, but naive billing deanonymizes users. Use **blind-signed tokens** (Privacy Pass-style): pay once with any method, receive unlinkable tokens redeemable for relay quota. The relay can't connect payment to mailbox.
- **Pro features** run locally and are cheap to gate (larger vaults, more devices, priority relay).
- **Self-hosting stays first-class** and free. That's both the ethos and the trust story.
- **Grants** (NLnet, OTF and similar) fit this work well and don't dilute control. For a solo developer, they may be the best runway for the audit.
- Consider a **foundation or stewardship structure** by 1.0: spec under permissive terms (CC0/MIT), reference implementations under a copyleft license (AGPL) so hosted forks can't quietly close it. Decide before outside contributors arrive; relicensing later is painful.

## 11. Local AI agent

The original frames the tension correctly: E2EE vs. agents. My resolution:

- **Local inference by default.** On-device models for summarize/search/draft over your own data; the plaintext never leaves the device.
- The agent is **a client of the capability API**, with explicit, revocable, scoped grants ("read this Space, no network, expires in an hour"). No ambient access.
- **No agent in a shared Space without every member's consent**, since an agent reading a group chat is a disclosure to a third party. Make this a protocol-level rule, not UI etiquette.
- Remote inference, if ever offered, is opt-in per request, with the cost made visible.
- Because mini-apps (v2.0) use the same grants, the agent proves the platform's security model *before* third parties arrive.

## 12. Payments

Don't build a wallet. A wallet means custody decisions, regulatory exposure, node management, and a security surface comparable to the whole rest of the app.

- v1.0: **tip links**: LNURL-pay, **Nostr Wallet Connect** to the user's own wallet, and optionally **Cashu** ecash (bearer tokens, good privacy fit). The app asks a wallet to pay; it never holds funds.
- Revisit an integrated wallet only after the platform (v2.0) exists and there's evidence users want it inside the app.

## 13. Engineering practices that matter for one person

- **Spec in the repo**, written before or alongside code: `docs/spec/` (identity, events, spaces, relay API), `THREAT_MODEL.md`, `docs/adr/` for decisions.
- **Deterministic simulation testing** for sync: a seeded network simulator (drops, reorders, partitions, clock skew) running many devices. Sync bugs are the ones that destroy trust, and they only show up under adversity.
- **Property-based tests** for event validation and log merge; **golden vectors** for every wire format.
- **Boring crypto only**: audited primitives (libsodium-class), no novel constructions, MLS rather than a bespoke ratchet. Anything custom goes to the reviewer first.
- **Dependency budget**: the four irreducible dependencies from the original, plus a CRDT lib and a CBOR lib. Everything else is a justification in an ADR.
- **Reproducible builds and signed releases** early. Sovereignty claims need verifiable binaries.

Suggested layout:

```
packages/
  identity/    key-event log, device delegation, recovery
  events/      typed events, CBOR, hashing, signatures
  spaces/      MLS groups, membership, ACLs
  sync/        log merge, snapshots, cursors
  store/       SQLite, blobs, FTS
  transport/   interface + websocket (later iroh, tor)
  capabilities/ grants, local API (agent & mini-apps use this)
  app-core/    the headless client facade the shells call
apps/
  client/      Tauri (and/or RN) shell, UI only
  relay/       the mailbox server
docs/ spec/ adr/
conformance/   language-neutral test vectors
```

## 14. Where I could be wrong

| My bet | What would prove me wrong |
|:--|:--|
| Tauri on mobile is good enough | The week-two spike feels laggy, or push/background work needs more native code than RN would. |
| MLS for every Space, including 1:1 | The MLS library is immature, or the per-message overhead/complexity hurts offline 1:1 UX. |
| A key-event log is worth its complexity | The spec balloons; users never use rotation. (Fallback in §4.) |
| Logs over CRDTs | Real usage needs concurrent editing of structured data well beyond notes. |
| Unlinkable public/private personas | Users find two identities confusing and want one profile. (Then make linking a one-tap default-off choice.) |
| Nostr bridge helps distribution | Cross-posting just leaks the audience out and nobody comes back. Measure retention, not signups. |
| Messenger-first wedge | Signal, SimpleX and Session already satisfy this audience. If so, the *identity + Spaces + notes* combination is the wedge, and v0.4 should move up. |

That last row is the one I'd watch most. A sovereign 1:1 messenger on its own competes with excellent existing tools. The thing nobody else offers is **one portable identity across private chat, public presence and owned notes**. v0.1 is a means of getting people onto that identity, not the pitch itself.

## 15. Prior art the landscape should add

The original landscape omits the projects closest to the *messaging and sync* layer:

- **SimpleX**: no user identifiers, per-contact unidirectional queues. The strongest metadata design in this space.
- **Signal**: the usability and crypto-engineering bar (sealed sender, sender keys, usernames).
- **Session / Briar / Bitchat**: onion routing, Tor-first and mesh approaches to anonymity and offline.
- **Matrix/Element**: federation and MLS migration lessons, including what it costs.
- **Secure Scuttlebutt, p2panda, Willow/Earthstar, Veilid, Holepunch/Keet**: signed-log and P2P sync designs worth reading before writing a line of `sync/`.
- **Bluesky `did:plc`, KERI, UCAN, Privacy Pass, Cashu**: identity portability, capabilities, anonymous payments.

## 16. First two weeks

1. `THREAT_MODEL.md` and ADR-0001 (Spaces + logs + MLS).
2. Draft identity key-event log spec with test vectors.
3. Event format, hashing and signing, with golden vectors.
4. MLS library bake-off: interop test, bundle size, maintenance, audit status.
5. Shell spike: the same chat screen in Tauri-mobile and RN.
6. A 200-line relay (put/get/subscribe) and a CLI client that exchanges one message between two identities.

If step 6 works end to end over a real network, with two devices and an offline gap in the middle, the rest is execution.

---

*This document is a position, not a mandate. Disagree with any row of §2 and I'll update the plan around your choice.*
