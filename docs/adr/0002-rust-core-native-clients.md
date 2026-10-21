# ADR-0002: Rust core, native mobile clients, Dioxus for web and desktop

**Status:** Accepted, pending the proof spikes in [docs/STACK.md](../STACK.md) section 12.

## Context

The product must be sovereign and long-lived, run on iOS, Android, desktop, and web, and feel native on mobile. It is built by one person working with AI agents. The long-lived logic (identity, events, Spaces, sync, storage, transport) is where correctness and security matter most.

## Decision

- The core and the relay are written in **Rust**, from day one.
- iOS is **SwiftUI** and Android is **Jetpack Compose**, each calling the core through UniFFI bindings.
- Web and desktop are **Dioxus** (Rust), linking the core directly: WASM on web, native on desktop.
- Clients are thin: the core owns logic and state, clients render query results and send commands.
- Full detail is in [docs/STACK.md](../STACK.md).

## Consequences

- One implementation of the protocol serves every platform; a later web client is a build target, not a rewrite.
- Three client codebases (Dioxus, SwiftUI, Compose). This is affordable only because the clients are thin.
- The UI framework for web and desktop is replaceable (Leptos is the fallback) without touching the core. Native mobile UI is a deliberate commitment.
- Rust, Swift, and Kotlin skills are all needed. Agents write most of the code; review is the human's job.

## Alternatives considered

- **TypeScript core, Expo/React Native plus Tauri:** two cross-platform shells, workarounds to share code, and a later port to Rust for MLS and P2P.
- **Tauri everywhere:** one UI, but WebView-feel on mobile.
- **Dioxus everywhere (including mobile):** the most coherent, but mobile defaults to a WebView today. It remains the fallback if three clients prove too much for one person.
- **Custom-rendered Rust UI toolkits (Makepad, Slint, egui):** non-native widgets, text input, and accessibility are risks for an app built around writing.
