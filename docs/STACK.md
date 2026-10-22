# Stack

**Status:** Proposed, pending proof spikes (see [§12](#12-proof-spikes-before-anything-is-final)). This supersedes the stack and language sections of [VISION.md](../VISION.md) (§2 rows 6–7 and §8). Protocol, identity, and roadmap positions in VISION.md still stand.

**Design goal:** pick once, build for decades. Put the hard, long-lived logic in one Rust core. Keep every client thin and native to its platform. Avoid wrappers and workarounds to share UI code.

---

## 1. One-way doors and two-way doors

| One-way (locked) | Two-way (replaceable later) |
|:--|:--|
| Rust core and relay | Web/desktop UI framework (Dioxus → Leptos or other Rust UI) |
| Wire and data formats (deterministic CBOR, BLAKE3, Ed25519) | Transport implementations (WebSocket → iroh → Tor) |
| Identity key-event log; MLS for every Space | Relay storage backend |
| On-disk store format and migration rules | CI provider, hosting, task runner |
| Native UI on iOS and Android | Linters, formatters, test helpers |
| Licensing | Individual crate dependencies behind traits |

The clients are two-way because they are thin: they render query results and send commands. The core is where the product lives.

## 2. Shape of the system

```
          ┌────────────────────────── Rust core ──────────────────────────┐
          │ identity · events · spaces(MLS) · sync · store · transport     │
          │ capabilities · app facade (commands + reactive queries)        │
          └───────┬──────────────────────┬─────────────────────┬──────────┘
                  │ UniFFI               │ UniFFI              │ direct crate link
            ┌─────▼─────┐          ┌─────▼──────┐        ┌─────▼──────────────┐
            │ iOS        │          │ Android    │        │ Web + Desktop      │
            │ SwiftUI    │          │ Compose    │        │ Dioxus (Rust)      │
            └────────────┘          └────────────┘        │ WASM  | native     │
                                                          └────────────────────┘
                         ▲  untrusted, self-hostable
                         │
                  ┌──────┴───────┐
                  │ Rust relay   │  (axum, SQLite/Postgres)
                  └──────────────┘
```

## 3. Languages

| Where | Language | Notes |
|:--|:--|:--|
| Core, relay, tooling, web/desktop UI | **Rust** (2024 edition, stable toolchain pinned) | The vast majority of the code. |
| iOS UI | **Swift** (SwiftUI) | UI only. |
| Android UI | **Kotlin** (Jetpack Compose) | UI only. |
| Specs and vectors | **CDDL**, plain text, binary vectors | Language-neutral. |

There is no JavaScript or TypeScript in the product. Small amounts of JS glue for the browser (Dioxus/wasm-bindgen generates it) are build artifacts, not source.

## 4. The core (Rust workspace)

| Crate | Responsibility |
|:--|:--|
| `ambient-identity` | Key-event log, device delegation, recovery, pairwise key derivation |
| `ambient-events` | Typed events, deterministic CBOR, BLAKE3 hashing, signatures |
| `ambient-spaces` | Spaces as MLS groups, membership, ACL log |
| `ambient-sync` | Per-device logs, merge, snapshots, cursors |
| `ambient-store` | SQLite, encrypted blobs, FTS5 search |
| `ambient-docs` | Creations: CRDT documents, collections, versions, Markdown parsing and styling spans, links and backlinks, creation references |
| `ambient-transport` | `Transport` trait; WebSocket impl first |
| `ambient-capabilities` | Scoped grants and the local API (agent and mini-apps use this) |
| `ambient-core` | The app facade: commands in, reactive query streams out |
| `ambient-ffi` | UniFFI surface for Swift and Kotlin (thin; wraps `ambient-core`) |
| `ambient-relay` | The relay binary |

Protocol crates use `#![forbid(unsafe_code)]` and have no UI or platform dependencies.

### Technology choices

| Concern | Choice | Notes |
|:--|:--|:--|
| Async runtime | `tokio` (native), `wasm-bindgen-futures` (web) | Runtime sits behind the facade. |
| Signatures / KX | `ed25519-dalek`, `x25519-dalek` | RustCrypto family. |
| AEAD / KDF | `chacha20poly1305`, `aes-gcm`, `hkdf` | |
| Hash | `blake3` | CIDs only at interop edges. |
| Group E2EE | **OpenMLS** (RFC 9420), HPKE (RFC 9180) | Verify audit/maturity in §12. |
| Encoding | Deterministic CBOR (RFC 8949 §4.2) via `ciborium` | Schemas in CDDL. |
| Storage | SQLite via `rusqlite` + SQLCipher (native) | Web storage: see §7. |
| Search | SQLite FTS5 | |
| Transport | WebSocket relay first → `iroh` (QUIC) → Tor | Behind a trait. |
| CRDTs (documents only) | Loro or Automerge, selected by spike | Not hand-rolled. |
| Markdown | `comrak` or `pulldown-cmark`, selected by spike | Parsing and styling spans live in the core, not in each client. |
| Serialization at the FFI | UniFFI records and enums | No bespoke wire format. |
| Errors / logging | `thiserror`, `tracing` | No user data in logs. |

## 5. The facade: how every client talks to the core

The facade is small and the same everywhere:

- **Commands**: `create_identity`, `send_message`, `join_space`, and so on. Async, typed, returning `Result`.
- **Queries**: reactive subscriptions such as `space_timeline(space_id)` that emit updated snapshots.
- **Events**: notifications such as `sync_state_changed` or `device_added`.

Rust types are the single source of truth. Dioxus calls the facade directly as a crate. Swift and Kotlin call it through UniFFI-generated bindings. There are no IPC or typegen layers and no serialization between the UI and the core on web and desktop.

The same facade is the **capability API** that the local agent and, later, mini-apps use, with every call scoped by a grant.

## 6. Clients

### iOS: SwiftUI
- Native navigation, lists, context menus, SF Symbols, system share sheets, Dynamic Type, VoiceOver.
- Core is an XCFramework built from `ambient-ffi`.
- **Notification Service Extension** receives a content-free push, fetches and decrypts via the core locally, and shows the real notification. APNs only ever sees a wake-up. (Extension memory limits are tight, so the core must stay lean; verify in §12.)

### Android: Jetpack Compose
- Material 3 where it fits, with platform conventions otherwise.
- Core is a shared library plus Kotlin bindings built with `cargo-ndk` and a Gradle plugin.
- **UnifiedPush** supported alongside FCM so push doesn't require Google services.

### Web and desktop: Dioxus
- One Rust codebase, two targets: **web** (WASM) and **desktop** (native window, system webview).
- Responsive multi-pane layouts, keyboard-first navigation, real context menus, drag-and-drop, built per desktop/web conventions rather than stretched mobile screens.
- Desktop gets the native core (OS keychain, SQLCipher, background running, tray, notifications, and later iroh P2P). Web gets the WASM core.
- Dioxus Native (the Blitz renderer) is a watch item. If it matures, desktop can adopt it without changing the architecture.

### Design system
Design tokens (color, type scale, spacing, radii, motion) live once in `design/tokens/` as data and generate Rust constants, Swift, and Kotlin. Components are implemented per platform with each platform's idiom; behavior and naming stay aligned through a short written component spec.

## 7. Platform realities

| Topic | Native (iOS, Android, desktop) | Web |
|:--|:--|:--|
| Key storage | Secure Enclave / Keystore / OS keychain | Seed wrapped by a **passkey PRF** and stored in IndexedDB; weaker than a keystore |
| Local database | SQLite + SQLCipher | SQLite WASM on OPFS; encryption approach is an open item (below) |
| Transport | WebSocket → iroh → Tor | WebSocket (WebTransport/WebRTC later); **no raw iroh** |
| Background sync | Platform push + background tasks | Installed PWA with Web Push only |
| Privacy tier | T0–T3 | T0–T2 |

**Open item, encrypted local search on web.** An FTS index over plaintext would sit unencrypted on disk. Options are SQLCipher compiled to WASM, an in-memory index rebuilt per session, or an encrypted index. Decide in the spike; do not ship web search before it's resolved.

**Web trust story.** The server can change JavaScript on every visit, so web must earn trust differently from a signed binary: reproducible builds with published hashes, a static bundle anyone can self-host and verify, and a version-pinned service worker with a signed manifest. Alongside it, ship non-store installs: an Android APK, F-Droid, and signed desktop binaries. iOS still requires the App Store (or TestFlight or alternative marketplaces where available).

## 8. Relay

- Rust, `axum`, single static binary and container image. SQLite by default (with Litestream-style replication); Postgres via `sqlx` for hosted fleets.
- API: put blob, get blobs since cursor, delete, subscribe. Nothing application-specific.
- Mailbox addressing, sealed-sender submission, padding, rate limits, and capability-gated writes per [VISION.md §6](../VISION.md).
- Self-hosting is a single command and a ten-minute job, or the sovereignty claim is marketing.

## 9. Tooling and quality gates

**Workspace and build:** Cargo workspace, `rust-toolchain.toml`, committed `Cargo.lock`, `just` as the one task entry point. Apple builds in Xcode and Gradle for Android, both driven by `just` recipes.

| Area | Tools |
|:--|:--|
| Rust format / lint | `rustfmt`, `clippy` (strict, warnings denied in CI) |
| Rust tests | `cargo-nextest`, `proptest`, `insta` snapshots, `cargo-fuzz`, Miri for any `unsafe` |
| Supply chain | `cargo-deny`, `cargo-vet`, SBOM on release |
| Sync correctness | Deterministic network simulation (`turmoil` or `madsim`): many devices, seeded drops/reorders/partitions/clock skew |
| Conformance | Golden vectors in `conformance/` checked by every implementation |
| Swift | `swift-format`, Swift Testing/XCTest, XCUITest |
| Kotlin | `ktlint`, `detekt`, JUnit, Compose UI tests |
| Web/desktop UI | `wasm-pack`/`dx` tests, headless-browser end-to-end tests driven from Rust (`thirtyfour` or equivalent) |

**Process**
- Spec first: `docs/spec/` in CDDL plus prose; code conforms to the spec.
- Decisions recorded as ADRs in `docs/adr/`.
- Trunk-based development, short-lived branches, required CI checks, conventional commits.
- Versioned wire format and store schema from day one: formats evolve additively, SQLite migrations are forward-only.
- Releases: signed, reproducible builds; `cargo-dist` for desktop and relay; `fastlane` for store uploads, run from our own CI (no dependence on a hosted build service).
- CI needs macOS runners for Apple builds; everything else runs on Linux.

**Working with agents**
- An `AGENTS.md` in each crate and client states its purpose, public API, invariants, and how to run its tests.
- Small crates with narrow public APIs; strict CI is the guardrail.
- `CODEOWNERS` requires human review for `ambient-identity`, `ambient-events`, `ambient-spaces`, and anything touching keys or the wire format.
- Any custom cryptographic construction goes to an external reviewer before merge; default to standard, audited primitives.

## 10. Licensing

**Deferred.** No license is declared and all rights are reserved until the owner decides (see [ADR-0004](adr/0004-licensing.md) and [LICENSING.md](../LICENSING.md)). The decision will be made before the first user other than the owner (and before any outside contribution), because it is the hardest one to reverse. The dependency policy in `deny.toml` accepts only permissive dependency licenses, which keeps every option open.

## 11. Repository layout

```
crates/
  ambient-identity/   ambient-events/    ambient-spaces/
  ambient-sync/       ambient-store/     ambient-transport/
  ambient-capabilities/  ambient-core/   ambient-ffi/
  ambient-docs/       ambient-relay/
clients/
  web-desktop/        Dioxus (web + desktop targets)
  apple/              Xcode project, SwiftUI
  android/            Gradle project, Compose
design/tokens/        design tokens (generates Rust/Swift/Kotlin)
conformance/          language-neutral test vectors
docs/
  spec/  adr/  STACK.md
```

## 12. Proof spikes before anything is final

Each spike has a pass condition. The architecture is final only when all pass.

| # | Spike | Pass condition |
|:--|:--|:--|
| 1 | Core on iOS | SwiftUI screen sends and receives a message through `ambient-ffi` on a real iPhone, including a Notification Service Extension decrypt. |
| 2 | Core on Android | Compose screen does the same on a real device. |
| 3 | Core on web | Dioxus WASM build runs the core, persists to SQLite on OPFS, survives reload, handles two tabs. |
| 4 | Core on desktop | Dioxus desktop build with SQLCipher and OS keychain. |
| 5 | End to end | Two identities on different platforms exchange messages through one relay, with an offline gap in the middle. |
| 6 | OpenMLS | Interop test, bundle size, memory use inside an iOS extension, maintenance and audit status. **Partially done:** see [docs/spikes/06-openmls.md](spikes/06-openmls.md). |
| 7 | Dioxus maturity | Build the real conversation screen; judge stability, accessibility, text input and IME, and upgrade churn. |
| 8 | Editor | A Markdown editor on all three clients with core-owned text and styling spans: typing, selection, IME composition, undo/redo, and a 50,000-word document that stays responsive. |

**If a spike fails:** iOS/Android/desktop failures change the binding or build approach, not the language choice. A Dioxus failure swaps the web/desktop UI framework (two-way door). An editor failure changes the editor approach on that client (for example, embedding a web editor component on Dioxus), not the core. OpenMLS failure is the only one that threatens a one-way door, so it runs first.

## 13. What we are deliberately not using

- **TypeScript, React, React Native, Expo, Tauri, Electron:** no JS runtime in the product.
- **Flutter, Kotlin Multiplatform:** own renderer or extra bridge, and no gain over native UI with a Rust core.
- **Go:** a second systems language that doesn't fit the Rust core.
- **A custom-rendered UI toolkit everywhere (Makepad, Slint, egui):** non-native widgets, text input, and accessibility are risks for an everything app.
- **Hand-rolled CRDTs, ratchets, or crypto:** use standard libraries and audited primitives.

## 14. Known risks

| Risk | Mitigation |
|:--|:--|
| Dioxus is pre-1.0 and may churn | UI is thin; spike #7; Leptos is the fallback; core is unaffected. |
| Three client codebases for one person | UIs are thin; shared tokens and component spec; agents write most UI; ship order below. |
| OpenMLS maturity and audit status | Run spike #6 first; pin versions; budget for external review. |
| iOS extension memory limits | Spike #1; keep the core's idle footprint small; content-free push as fallback. |
| Web storage and trust limits | Documented in §7; web is a T0–T2 client by design. |
| Rust build times | `sccache`, workspace splits, `cargo check` in agent loops. |

## 15. Suggested ship order

Native feel matters most on mobile, but web is the fastest place to iterate, and agents can test it headlessly. A pragmatic order, consistent with the roadmap in the README:

1. Core, relay, and a CLI client (proves the protocol with no UI).
2. Dioxus web client as the reference UI.
3. iOS and Android clients on the same facade.
4. Dioxus desktop, then iroh P2P (v0.8).

Writing is a desktop and web experience first. The Dioxus client gets the full editor when creations land (v0.3); mobile starts with short notes, reading, commenting, and conversations, and gains long-form editing afterward.

The architecture is declared up front; only the build order is flexible.
