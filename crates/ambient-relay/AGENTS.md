# ambient-relay

Stores and forwards opaque encrypted blobs between mailbox addresses. The connection track, and the order of work for a relay another computer can use, is [docs/network/README.md](../../docs/network/README.md). See `VISION.md` section 6.

**Public API:** `Relay::open`, `Relay::handle_line`, `serve`, `serve_tls`, `serve_listener`, `serve_tls_listener`, `serve_until`. `Relay::open` reads and writes `mailboxes` in a directory. `serve` is cleartext and refuses any address that is not this computer. `serve_tls` speaks TLS from a PEM certificate and key the operator supplies. `serve_listener` is memory only, for tests. The binary without `--tls` binds `127.0.0.1` and takes a port and a directory (`relay-data` when omitted). `--tls cert key --store dir --bind addr` is the public listener. The notes window stores its loopback mail in `~/.local/share/ambient/relay`. Spec 04 says cleartext is not a conforming public deployment. There is no TTL, quota, or rate limit. This process does not parse blobs. `serve_until` returns when a generation counter changes, so the notes window can stop a relay it started.

## Invariants

- The relay must never need plaintext, and must not be able to link mailboxes to identities.
- Retention is by TTL and quota, enforced without reading content.
- No analytics or telemetry. Logs must not contain user data.
- Self-hosting stays a one-command job.

## Dependencies

May depend on: `ambient-events`, `ambient-transport`. This crate currently depends on transport only, and it must not parse blobs. Must not depend on any crate above it in the layering (see root `AGENTS.md`). Add external dependencies only with a stated reason, then run `just deny`.

## Targets and review

- Native only; excluded from the wasm check.
- Human review: Required (listed in `.github/CODEOWNERS`).

## Test

`cargo nextest run -p ambient-relay` and `cargo test -p ambient-relay --doc`. Run `just ci` before committing.
