# Contributing

Humans and agents follow the same rules.

## Read first

[AGENTS.md](AGENTS.md), [ARCHITECTURE.md](ARCHITECTURE.md), [THREAT_MODEL.md](THREAT_MODEL.md), and the ADRs. Do not re-open a decision that already has an ADR. Supersede it with a new one.

## Before writing code

Wire formats and protocol behavior start in `docs/spec/`, with vectors in `conformance/`. Code follows. A crate that implements a rule with no vector is not done.

v0.1 scope is the fence in [ARCHITECTURE.md](ARCHITECTURE.md). A change outside that fence needs the owner to say so, and an entry in [HISTORY.md](HISTORY.md).

## Checks

The toolchain is pinned in `rust-toolchain.toml`.

```
cargo install --locked just cargo-nextest cargo-deny
just ci
```

`just ci` is format, lint, test, docs, the wasm check, and the dependency policy. Do not weaken a check to make it pass.

`just lint` passes on the whole workspace and all targets. Keep it that way: run it before the commit, not after, and gate the commit on it. If `just` and `cargo-nextest` are not installed, the same checks are `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked`, and `cargo check` for `wasm32-unknown-unknown` on the protocol crates. `cargo deny check` has no substitute.

In a dev build, dependencies and the crates on the typing, saving, and syncing path are optimized (`[profile.dev.package]` in `Cargo.toml`). The first build after pulling it is slower. Stepping through those crates in a debugger is harder. See [ADR-0003](docs/adr/0003-workspace-and-tooling.md).

## Changes

- Conventional commits, small enough to read.
- Crypto, identity, Spaces, capabilities, and the relay need owner review.
- No new dependency without a stated reason, a `workspace.dependencies` entry, and `just deny`.
- No plaintext, keys, or mailbox ids in logs.
- Experiments go in `spikes/` with their own lockfile. They are not part of the workspace.

## Agents

- Do not add a second encryption path, an agent runtime, or a JavaScript client.
- If a turn changes a locked choice, update `HISTORY.md` in the same change.
- A change to which events a projection applies is a protocol-behavior change. It needs a spec edit and an ADR, and it needs owner review. A change to how a log is written, locked, or retried is a change to data safety. Say so in the commit.
- Check the branch before describing the app. `git fetch` and `git branch -a`. A stale checkout was once read as the whole project.
- If you are unsure about a security-relevant choice, stop and ask.
