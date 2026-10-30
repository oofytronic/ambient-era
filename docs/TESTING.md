# Testing

## What exists now

`just ci` runs format, clippy, nextest, rustdoc, wasm, and `cargo deny`. `ambient-events` reproduces the event vectors. `ambient-identity` reproduces genesis and projects the identity log through revoke. `ambient-spaces` projects notes and a conversation, and opens the published HPKE blob. The CLI, and the Linux window that calls it, cover notes, a second device, and one 1:1 through the loopback relay. A green run is not a second implementation of the vectors, and it is not the v0.1 exit.

Vectors in `conformance/vectors/` are the spec in executable form. `conformance/tools/generate.py` writes them and checks its HPKE code against a RFC 9180 test vector first. The event vectors are reproduced by `ambient-events`. `ambient-spaces` opens the published HPKE blob; it does not re-seal the generator's raw scalar. There is no separate runner crate.

## When behavior lands

- Every `MUST reject` in a spec has a negative vector. The events spec already has non-canonical input, unsorted keys, indefinite length, and a bad signature.
- Pure functions in `ambient-events` take their inputs as arguments. Tests do not read the clock or generate randomness inside the library.
- Sync merge gets property tests: the same events in any order, including duplicates, produce the same state.
- The relay is tested against `docs/spec/04-relay-api.md` with opaque bytes. A test that needs the relay to understand an event is a failed design.
- No test hits the network.

## What not to claim

A green `just ci` on the scaffold is not a crypto audit and not evidence of forward secrecy, anonymity, or deletion. The claims that are allowed are in [THREAT_MODEL.md](../THREAT_MODEL.md).
