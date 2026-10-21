# ambient-sync

Turns each member's append-only logs into a consistent Space state, and lets new devices catch up without replaying everything.

**Public API (planned):** Log append and merge, checkpoints, cursor handling.

## Invariants

- Merge is commutative, associative, and idempotent. Cover it with property tests.
- Output depends only on inputs: same events in any order give the same state.
- Sync bugs destroy trust: anything touching merge must extend the deterministic simulation tests.

## Dependencies

May depend on: `ambient-events`. Must not depend on any crate above it in the layering (see root `AGENTS.md`). Add external dependencies only with a stated reason, then run `just deny`.

## Targets and review

- Must compile for `wasm32-unknown-unknown` (`just wasm-check`).
- Human review: Standard review.

## Test

`cargo nextest run -p ambient-sync` and `cargo test -p ambient-sync --doc`. Run `just ci` before committing.
