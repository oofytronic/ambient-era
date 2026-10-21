# ambient-ffi

A thin binding layer over `ambient-core` for iOS and Android.

**Public API (planned):** UniFFI records, enums, and objects mirroring the core facade.

## Invariants

- No logic here: anything beyond type conversion belongs in `ambient-core`.
- Panics must never cross the FFI boundary.
- The workspace forbids `unsafe`. UniFFI-generated code may need that relaxed for this crate only; do it in this crate's `Cargo.toml` with a comment explaining why, and get it reviewed.

## Dependencies

May depend on: `ambient-core`. Must not depend on any crate above it in the layering (see root `AGENTS.md`). Add external dependencies only with a stated reason, then run `just deny`.

## Targets and review

- Native only; excluded from the wasm check.
- Human review: Standard review.

## Test

`cargo nextest run -p ambient-ffi` and `cargo test -p ambient-ffi --doc`. Run `just ci` before committing.
