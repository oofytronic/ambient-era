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

## Changes

- Conventional commits, small enough to read.
- Crypto, identity, Spaces, capabilities, and the relay need owner review.
- No new dependency without a stated reason, a `workspace.dependencies` entry, and `just deny`.
- No plaintext, keys, or mailbox ids in logs.
- Experiments go in `spikes/` with their own lockfile. They are not part of the workspace.

## Agents

- Do not add a second encryption path, an agent runtime, or a JavaScript client.
- If a turn changes a locked choice, update `HISTORY.md` in the same change.
- If you are unsure about a security-relevant choice, stop and ask.
