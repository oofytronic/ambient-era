# ambient-relay

Stores and forwards opaque encrypted blobs between mailbox addresses. See `VISION.md` section 6.

**Public API (planned):** Put blob, get blobs since cursor, delete, subscribe. Nothing application-specific.

## Invariants

- The relay must never need plaintext, and must not be able to link mailboxes to identities.
- Retention is by TTL and quota, enforced without reading content.
- No analytics or telemetry. Logs must not contain user data.
- Self-hosting stays a one-command job.

## Dependencies

May depend on: `ambient-events`, `ambient-transport`. Must not depend on any crate above it in the layering (see root `AGENTS.md`). Add external dependencies only with a stated reason, then run `just deny`.

## Targets and review

- Native only; excluded from the wasm check.
- Human review: Required (listed in `.github/CODEOWNERS`).

## Test

`cargo nextest run -p ambient-relay` and `cargo test -p ambient-relay --doc`. Run `just ci` before committing.
