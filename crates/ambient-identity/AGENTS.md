# ambient-identity

Owns the root seed derivation tree and the rotatable identity log described in `VISION.md` section 4.

**Public API:** `derive` and `Identity::genesis_wire`. Device add, revoke, and pairing are not implemented yet. The derivation vector is `conformance/vectors/identity/0001-genesis.json`.

## Invariants

- The root seed and private keys never leave the keystore abstraction; secret types zeroize on drop and never implement `Debug` with contents.
- Log validation is total and deterministic: any byte string either validates or yields a typed error, never a panic.
- No global identifier is ever shown to a contact; contacts see pairwise keys only.
- Passkeys and biometrics only unlock the seed; they are never the root.

## Dependencies

May depend on: `ambient-events`. Must not depend on any crate above it in the layering (see root `AGENTS.md`). Add external dependencies only with a stated reason, then run `just deny`.

## Targets and review

- Must compile for `wasm32-unknown-unknown` (`just wasm-check`).
- Human review: Required (listed in `.github/CODEOWNERS`).

## Test

`cargo nextest run -p ambient-identity` and `cargo test -p ambient-identity --doc`. Run `just ci` before committing.
