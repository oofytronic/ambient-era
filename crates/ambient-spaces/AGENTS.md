# ambient-spaces

The one primitive for conversations and creations (ADR-0001). Every private Space is an MLS group.

**Public API (planned):** Space creation, membership and role changes, share Spaces with expiry, role validation.

## Invariants

- Every private Space is an MLS group (RFC 9420); there is no second encryption path.
- Events from members without the required role are ignored by every client. Read-only is enforced by rules, not by cryptography, and the UI must say so.
- A share Space carries a signed expiry; honest clients purge on expiry or revocation.
- Use an audited MLS implementation; do not hand-roll ratchets or key schedules.

## Dependencies

May depend on: `ambient-events`, `ambient-identity`. Must not depend on any crate above it in the layering (see root `AGENTS.md`). Add external dependencies only with a stated reason, then run `just deny`.

## Targets and review

- Must compile for `wasm32-unknown-unknown` (`just wasm-check`).
- Human review: Required (listed in `.github/CODEOWNERS`).

## Test

`cargo nextest run -p ambient-spaces` and `cargo test -p ambient-spaces --doc`. Run `just ci` before committing.
