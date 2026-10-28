# ambient-events

The wire and storage format for everything in the system. Defines events, their canonical encoding, content IDs, and signature verification.

**Public API:** `verify`, `sign_wire`, `encode_unsigned`, `signing_public`, `EventId`, `VerifiedEvent`, `EventError`. Schemas live in `docs/spec/01-events.md`; this crate conforms to the vectors in `conformance/vectors/events/`.

## Invariants

- Encoding is deterministic (RFC 8949 section 4.2). Decoding rejects non-canonical input.
- An `EventId` is the BLAKE3 hash of the canonical bytes. Never hash anything else.
- No I/O, no clock, no randomness: callers inject all of them, so behavior is reproducible.
- Every wire format change needs a spec update, new conformance vectors, and an ADR.

## Dependencies

May depend on: none (this is a leaf crate). Must not depend on any crate above it in the layering (see root `AGENTS.md`). Add external dependencies only with a stated reason, then run `just deny`.

## Targets and review

- Must compile for `wasm32-unknown-unknown` (`just wasm-check`).
- Human review: Required (listed in `.github/CODEOWNERS`).

## Test

`cargo nextest run -p ambient-events` and `cargo test -p ambient-events --doc`. Run `just ci` before committing.
