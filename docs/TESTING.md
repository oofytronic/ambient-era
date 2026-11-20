# Testing

## What exists now

`just ci` runs format, clippy, nextest, rustdoc, wasm, and `cargo deny`. `ambient-events` reproduces the event vectors. `ambient-identity` reproduces genesis and projects the identity log through revoke. `ambient-spaces` projects notes and a conversation, and opens the published HPKE blob. The CLI, and the Linux window that calls it, cover notes, a second device, and one 1:1 through the loopback relay. A green run is not a second implementation of the vectors, and it is not the v0.1 exit.

Since 2026-11-19 there are also:

- **A test harness for the window.** `App::at(dir)` builds a real `App` on a real folder. A `cfg(test)` recorder in `apply_note` lets a test do the background writes the app asked for and hand each result back through `update`. A test follows the same path the window does: add a task, write, `Applied`, and the list. Iced does not let a test run a `Task` from outside its runtime, and `iced_runtime` was not added as a dependency for this.
- **Equivalence tests for the page.** The incremental parse cache is checked against a full parse over thousands of random edits and chains of edits, from a seeded generator so a failure can be reproduced. The verified-event projections in `ambient-identity` and `ambient-spaces` are checked against the wire ones.
- **Stress tests for writes.** Three writers race into one log. A local writer interleaves with a sync that is waiting on the relay, for messages and for notes. The store has a test of six threads that lose no event.
- **A test that builds the view** in every state of focus, including mid-slide and with the delete question open. It checks that nothing panics, not what is drawn.

Tests that touch the published notebook or the personal dictionary take turns with `tests_support::one_at_a_time`, because each is one per process.

Vectors in `conformance/vectors/` are the spec in executable form. `conformance/tools/generate.py` writes them and checks its HPKE code against a RFC 9180 test vector first. The event vectors are reproduced by `ambient-events`. `ambient-spaces` opens the published HPKE blob; it does not re-seal the generator's raw scalar. There is no separate runner crate.

## When behavior lands

- Every `MUST reject` in a spec has a negative vector. The events spec already has non-canonical input, unsorted keys, indefinite length, and a bad signature.
- Pure functions in `ambient-events` take their inputs as arguments. Tests do not read the clock or generate randomness inside the library.
- Sync merge gets property tests: the same events in any order, including duplicates, produce the same state.
- The relay is tested against `docs/spec/04-relay-api.md` with opaque bytes. A test that needs the relay to understand an event is a failed design.
- No test hits the network.

## Measuring

Measure the build that people run. A plain `cargo run` is the dev profile. The workspace's own crates were once unoptimized there, and that was most of what made typing late. A timing probe is a throwaway `#[ignore]` test or an example. Do not commit one with numbers in it, because a time bound in a test is flaky across machines. Put the numbers in [HISTORY.md](../HISTORY.md) or a session record with the date and the machine's build profile.

A test that fails some of the time is a finding, not noise. Run it many times, on the unchanged tip too, before deciding what caused it. The restore test failed about three runs in eight, and the cause was a merge-order bug and not the guess that was made first.

## What not to claim

A green `just ci` on the scaffold is not a crypto audit and not evidence of forward secrecy, anonymity, or deletion. The claims that are allowed are in [THREAT_MODEL.md](../THREAT_MODEL.md).
