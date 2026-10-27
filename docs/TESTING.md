# Testing

## What exists now

`just ci` builds the scaffold: format, clippy, nextest, rustdoc, wasm, and `cargo deny`. The crates have no protocol behavior yet, so those tests do not prove the specs.

Vectors in `conformance/vectors/` are the spec in executable form. `conformance/tools/generate.py` writes them and checks its HPKE code against a RFC 9180 test vector first. The event and sealing vectors are from that one generator. They are not trusted until `ambient-events` and `ambient-spaces` reproduce them. A runner crate is added with the first implementation, not before.

## When behavior lands

- Every `MUST reject` in a spec has a negative vector. The events spec already has non-canonical input, unsorted keys, indefinite length, and a bad signature.
- Pure functions in `ambient-events` take their inputs as arguments. Tests do not read the clock or generate randomness inside the library.
- Sync merge gets property tests: the same events in any order, including duplicates, produce the same state.
- The relay is tested against `docs/spec/04-relay-api.md` with opaque bytes. A test that needs the relay to understand an event is a failed design.
- No test hits the network.

## What not to claim

A green `just ci` on the scaffold is not a crypto audit and not evidence of forward secrecy, anonymity, or deletion. The claims that are allowed are in [THREAT_MODEL.md](../THREAT_MODEL.md).
