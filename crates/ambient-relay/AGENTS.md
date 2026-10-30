# ambient-relay

Stores and forwards opaque encrypted blobs between mailbox addresses. See `VISION.md` section 6.

**Public API:** `Relay::handle_line`, `serve`, `serve_listener`. Mailboxes are in memory. The binary binds `127.0.0.1` and speaks cleartext. Spec 04 says that is not a conforming public deployment. There is no TTL, quota, or rate limit. This process does not parse blobs.

## Invariants

- The relay must never need plaintext, and must not be able to link mailboxes to identities.
- Retention is by TTL and quota, enforced without reading content.
- No analytics or telemetry. Logs must not contain user data.
- Self-hosting stays a one-command job.

## Dependencies

May depend on: `ambient-events`, `ambient-transport`. This crate currently depends on transport only, and it must not parse blobs. Must not depend on any crate above it in the layering (see root `AGENTS.md`). Add external dependencies only with a stated reason, then run `just deny`.

## Targets and review

- Native only; excluded from the wasm check.
- Human review: Required (listed in `.github/CODEOWNERS`).

## Test

`cargo nextest run -p ambient-relay` and `cargo test -p ambient-relay --doc`. Run `just ci` before committing.
