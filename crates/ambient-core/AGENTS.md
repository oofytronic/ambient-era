# ambient-core

The headless client. All logic a UI needs, none of the UI. Also the capability API used by the agent and mini-apps.

**Public API (planned):** Commands, reactive query subscriptions, event notifications (`docs/STACK.md` section 5).

## Invariants

- No UI types or platform UI dependencies, ever.
- The async runtime is an implementation detail behind the facade.
- Clients must not need to know about the crates beneath this one.

## Dependencies

May depend on: `ambient-events`, `ambient-identity`, `ambient-spaces`, `ambient-sync`, `ambient-store`, `ambient-transport`, `ambient-capabilities`, `ambient-docs`. Must not depend on any crate above it in the layering (see root `AGENTS.md`). Add external dependencies only with a stated reason, then run `just deny`.

## Targets and review

- Must compile for `wasm32-unknown-unknown` (`just wasm-check`).
- Human review: Standard review.

## Test

`cargo nextest run -p ambient-core` and `cargo test -p ambient-core --doc`. Run `just ci` before committing.
