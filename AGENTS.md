# AGENTS.md

Guidance for AI coding agents and human contributors. Each crate under `crates/` has its own `AGENTS.md` with local rules; this file covers the whole repository.

## Read first

1. [README.md](README.md): what this is.
2. [THREAT_MODEL.md](THREAT_MODEL.md): what is actually promised.
3. [docs/adr/](docs/adr/): decisions already made. Do not re-litigate them in a code change. [ADR-0005](docs/adr/0005-private-space-encryption.md) wins over any older sentence that says a Space is an MLS group.
4. [docs/STACK.md](docs/STACK.md): the stack and why.
5. [HISTORY.md](HISTORY.md): why the direction changed. Add an entry when a turn changes a locked choice.
6. [VISION.md](VISION.md): background positions. Where it disagrees with an ADR or with STACK, the ADR or STACK wins.

## Commands

```
just ci          # everything CI runs; run before every commit
just fmt         # format
just lint        # clippy, warnings denied
just test        # nextest + doc tests
just wasm-check  # protocol crates must compile for the browser
just deny        # licenses, advisories, banned crates, sources
```

The toolchain is pinned in `rust-toolchain.toml`. Do not change it as a side effect of another change.

## Layering

Dependencies point down only. A crate may depend only on the crates listed in its own `AGENTS.md`.

```
ambient-events                      (leaf: formats, hashing, signatures)
 ├─ ambient-identity
 │   ├─ ambient-spaces
 │   └─ ambient-capabilities
 ├─ ambient-sync
 ├─ ambient-store
 ├─ ambient-transport
 └─ ambient-docs
        all of the above ─▶ ambient-core ─▶ ambient-ffi
ambient-relay  depends on ambient-events and ambient-transport only
```

The relay never depends on the core, the store, or anything that holds a user's plaintext.

## Rules

- **Spec first.** A wire-format or protocol change needs a spec update in `docs/spec/`, new vectors in `conformance/`, and an ADR. Code follows the spec, not the other way around.
- **No panics on untrusted input.** `unwrap`, `expect`, `panic!`, `todo!`, and unchecked indexing are denied in production code by workspace lints. Return typed errors. Tests may use them.
- **No `unsafe`.** It is forbidden workspace-wide. If a crate truly needs it (for example UniFFI output), change that crate's lint setting explicitly, explain why in the PR, and expect review.
- **Crypto.** Use audited libraries and standard constructions only. Never invent a protocol, cipher mode, KDF, or key schedule. Changes to `ambient-events`, `ambient-identity`, `ambient-spaces`, `ambient-capabilities`, or `ambient-relay` need owner review.
- **Dependencies are a budget.** Add one only with a stated reason, in `[workspace.dependencies]`, then run `just deny`. Prefer fewer and well-maintained over convenient.
- **Browser-compatible by default.** Protocol crates must keep passing `just wasm-check`. Platform-specific code goes behind traits in the crates built for it.
- **Do not weaken checks.** Never disable, skip, or loosen a test, lint, or CI job to get green. Fix the cause. If a check is wrong, say so and propose the change separately.
- **No telemetry, no user data in logs.** Ever.
- **Honest claims.** Do not write docs, UI copy, or comments claiming a property the code does not yet provide (anonymity, forward secrecy, unrecoverable deletion, and so on). See [THREAT_MODEL.md](THREAT_MODEL.md).
- **Unsure about a security-relevant choice?** Stop and ask rather than guess.

## Conventions

- Conventional commits (`feat:`, `fix:`, `docs:`, `chore:`, `test:`, `refactor:`), small and focused.
- Decisions that are hard to reverse get an ADR (`docs/adr/`).
- Experiments live in `spikes/` with their own lockfile and are never part of the workspace build.
