# ambient-capabilities

The permission model shared by sharing, the local agent, and future mini-apps. Sharing a creation is a grant.

**Public API (planned):** Grant types (UCAN-style), verification, attenuation, expiry, revocation.

## Invariants

- Default deny. Every call through the local API is checked against a grant.
- Grants can only be narrowed (attenuated), never widened, when delegated.
- No ambient authority for agents or extensions.

## Dependencies

May depend on: `ambient-events`, `ambient-identity`. Must not depend on any crate above it in the layering (see root `AGENTS.md`). Add external dependencies only with a stated reason, then run `just deny`.

## Targets and review

- Must compile for `wasm32-unknown-unknown` (`just wasm-check`).
- Human review: Required (listed in `.github/CODEOWNERS`).

## Test

`cargo nextest run -p ambient-capabilities` and `cargo test -p ambient-capabilities --doc`. Run `just ci` before committing.
