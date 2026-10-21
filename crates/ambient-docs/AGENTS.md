# ambient-docs

Everything needed to write books, series, newsletters, and notes, with the logic in the core so each client's editor stays thin (`docs/STACK.md` section 6).

**Public API (planned):** Document and collection model, versions, Markdown parse to styled spans, edit application, links, `CreationRef`.

## Invariants

- Parsing is total: arbitrary input never panics. Keep a fuzz target for it.
- A comment stores its anchor (version hash and range), never a copy of the text it points at (ADR-0001).
- Use an existing CRDT library (Loro or Automerge); do not write one.

## Dependencies

May depend on: `ambient-events`. Must not depend on any crate above it in the layering (see root `AGENTS.md`). Add external dependencies only with a stated reason, then run `just deny`.

## Targets and review

- Must compile for `wasm32-unknown-unknown` (`just wasm-check`).
- Human review: Standard review.

## Test

`cargo nextest run -p ambient-docs` and `cargo test -p ambient-docs --doc`. Run `just ci` before committing.
