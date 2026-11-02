# ambient-store

Local persistence behind traits so each platform can supply its backend (SQLCipher natively, SQLite on OPFS on web).

**Public API:** `seal_log`, `open_log`, `read_log`, `write_log`. The bytes are an encrypted log of wire events. SQLCipher and FTS5 are not this backend: the native client keeps that index beside the log, because this crate must compile for wasm. Search on the native client uses that index. If the index cannot be opened, search scans the decrypted projection.

## Invariants

- Keys never live in the database; they live in the OS keystore.
- Migrations are forward-only and versioned from day one.
- Encrypted-at-rest applies to the search index too. Do not ship web search until that is resolved (`docs/STACK.md` section 7).

## Dependencies

May depend on: `ambient-events`. Must not depend on any crate above it in the layering (see root `AGENTS.md`). Add external dependencies only with a stated reason, then run `just deny`.

## Targets and review

- Must compile for `wasm32-unknown-unknown` (`just wasm-check`).
- Human review: Standard review.

## Test

`cargo nextest run -p ambient-store` and `cargo test -p ambient-store --doc`. Run `just ci` before committing.
