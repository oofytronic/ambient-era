# ambient-docs

Document bodies: notes in the personal Notes space, and later books, series, and newsletters. The logic stays in the core so each client's editor stays thin (`docs/STACK.md` section 6).

**Public API (planned):** Document and collection model, versions, Markdown parse to styled spans, edit application, links, `CreationRef`.

## Invariants

- Parsing is total: arbitrary input never panics. Keep a fuzz target for it.
- A comment stores its anchor (version hash and range), never a copy of the text it points at (ADR-0001).
- The Space log is not a CRDT. v0.1 note bodies are `text-snapshot-v1` as specified in `docs/spec/03-spaces.md`: concurrent edits are kept, not merged. A CRDT codec (Loro or Automerge) is a later type string, chosen by a spike, not by this crate in advance. Do not write a CRDT.
- v0.1 notes live in the owner's Notes space. A creation becomes its own Space only when it needs its own membership (ADR-0001). Copying a note into a chat copies the text. It does not grant access to the note.

## Dependencies

May depend on: `ambient-events`. Must not depend on any crate above it in the layering (see root `AGENTS.md`). Add external dependencies only with a stated reason, then run `just deny`.

## Targets and review

- Must compile for `wasm32-unknown-unknown` (`just wasm-check`).
- Human review: Standard review.

## Test

`cargo nextest run -p ambient-docs` and `cargo test -p ambient-docs --doc`. Run `just ci` before committing.
