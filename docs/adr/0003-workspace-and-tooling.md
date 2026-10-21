# ADR-0003: Workspace layout, lints, dependency policy, and CI

**Status:** Accepted.

## Context

The codebase will be written largely by agents and must stay trustworthy for years. Guardrails need to be mechanical, because review attention is the scarce resource. Untrusted input (network data, other users' events) is the normal case.

## Decision

**Workspace**
- One Cargo workspace, one crate per concern (`crates/ambient-*`), with dependencies pointing down only (layering in the root [AGENTS.md](../../AGENTS.md)).
- Rust **1.97.0** pinned in `rust-toolchain.toml`; edition 2024; resolver 3; `rust-version` equals the pinned toolchain. Toolchain bumps are their own commit.
- `Cargo.lock` is committed and every CI command uses `--locked`.
- Experiments live in `spikes/`, excluded from the workspace, with their own lockfiles.

**Lints (workspace-wide)**
- `unsafe_code = forbid`.
- Clippy `pedantic` as warnings; `unwrap_used`, `expect_used`, `panic`, `todo`, `unimplemented`, `dbg_macro` denied; `indexing_slicing` and printing warned. Tests are exempt via `clippy.toml`.
- CI runs clippy with `-D warnings`, so every warning is a failure.

**Dependencies**
- Every new external dependency is added to `[workspace.dependencies]` with a stated reason.
- `cargo-deny` enforces an allow-list of licenses, rejects wildcard versions and unknown registries or git sources, and fails on known advisories. A scheduled job re-checks advisories weekly, independent of code changes.
- Dependabot proposes cargo and GitHub Actions updates weekly.

**CI**
- Least-privilege token (`contents: read`), concurrency cancellation, job timeouts.
- Third-party actions are **pinned to commit SHAs** with the version in a comment.
- Checks: format, clippy, tests (Linux and macOS), docs with warnings denied, wasm32 check for the protocol crates, `cargo-deny`.
- Tool versions installed in CI are pinned.
- `just ci` runs the same checks locally.

## Consequences

- Agents get immediate, mechanical feedback; most classes of mistake fail before review.
- Pedantic lints and no-panic rules add friction. Where a rule is wrong for a specific line, `#[allow(...)]` with a reason is acceptable, and is visible in review.
- Pinned SHAs and tool versions need regular bumps; Dependabot covers actions and crates.

## Deferred (planned in STACK.md, not adopted yet because there is no code to apply them to)

- `cargo-vet` (adopt when the first real dependencies land), `cargo-fuzz`, `proptest`, `insta`, Miri (when there is `unsafe` to check), and deterministic network simulation (`turmoil` or `madsim`, with the first sync code).
- A conformance runner crate (with the first vectors).
- Windows in the CI matrix (with the desktop client).
- SBOM and signed, reproducible release builds (with the first release).

## Alternatives considered

- **Floating `stable` toolchain:** simpler, but builds change under you.
- **Pinning actions by tag only:** easier to read, but tags can be moved; SHAs cannot.
- **`#![deny(warnings)]` in code:** breaks builds on compiler upgrades; denying in CI is safer.
