# ambient-spaces

The one primitive for conversations and creations (ADR-0001). A Space is a membership list and a role log. Private Spaces are sealed to current member device keys (ADR-0005). They are not MLS groups.

**Public API:** `project_notes`, `project_conversation`, the notes and conversation payload builders, `seal_blob`, and `open_blob`. Share-expiry is not in spec 03. Do not add it here.

## Invariants

- Every private Space is sealed with HPKE (RFC 9180) to each member device's current encryption key. There is no second encryption path and no MLS dependency.
- Membership and application events stay commutative. Do not add a totally ordered group-commit cipher without a new ADR.
- Events from members without the required role are ignored by every client. Read-only is enforced by rules, not by cryptography, and the UI must say so.
- A share Space carries a signed expiry; honest clients purge on expiry or revocation. Purging does not mean a hostile client forgot.
- Use HPKE from the audited RustCrypto crates. Do not hand-roll ratchets or key schedules.

## Dependencies

May depend on: `ambient-events`, `ambient-identity`. Must not depend on any crate above it in the layering (see root `AGENTS.md`). Add external dependencies only with a stated reason, then run `just deny`.

## Targets and review

- Must compile for `wasm32-unknown-unknown` (`just wasm-check`).
- Human review: Required (listed in `.github/CODEOWNERS`).

## Test

`cargo nextest run -p ambient-spaces` and `cargo test -p ambient-spaces --doc`. Run `just ci` before committing.
