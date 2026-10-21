# ambient-transport

Moves opaque blobs between devices and relays. Transports are untrusted.

**Public API (planned):** `Transport` trait (`send`, `receive`, `discover`), WebSocket implementation.

## Invariants

- Never assume a transport is authenticated or private; confidentiality and authenticity come from the layers above.
- No business logic here. Implementations must be swappable behind the trait.

## Dependencies

May depend on: `ambient-events`. Must not depend on any crate above it in the layering (see root `AGENTS.md`). Add external dependencies only with a stated reason, then run `just deny`.

## Targets and review

- Must compile for `wasm32-unknown-unknown` (`just wasm-check`).
- Human review: Standard review.

## Test

`cargo nextest run -p ambient-transport` and `cargo test -p ambient-transport --doc`. Run `just ci` before committing.
