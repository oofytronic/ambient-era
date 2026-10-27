# Conformance vectors

Language-neutral test vectors. They are the real specification in executable form: any implementation of the protocol (this repository's Rust core, a future independent client, a relay) must produce the expected output for every vector.

**Status:** vectors exist for the event envelope, identity derivation, and one HPKE blob. They were produced by [tools/generate.py](tools/generate.py), which checks its HPKE implementation against a RFC 9180 vector before writing. The Ambient vectors are provisional until the Rust core reproduces them. No protocol behavior is implemented yet.

## Layout

```
conformance/
  vectors/
    <area>/                 events, identity, spaces, docs
      NNNN-short-name.json
```

## Vector format

One JSON file per vector. Binary values are lowercase hex strings.

```json
{
  "description": "What this vector demonstrates, in one sentence.",
  "spec": "docs/spec/01-events.md#section",
  "input":  { "bytes": "a2..." },
  "expect": { "ok": { "id": "..." } }
}
```

For inputs that must be rejected, `expect` is `{ "error": "<stable-error-code>" }`.

## Rules

- **Negative vectors are mandatory.** For every rule that says "MUST reject", include inputs that violate it (non-canonical encoding, bad signature, truncated input, wrong version).
- **Vectors are append-only.** Once published, a vector's meaning does not change. Fix a wrong vector by adding a new one and deprecating the old with a note.
- **Every vector cites its spec section.** No spec section, no vector.
- **Vectors must be reproducible** by at least two independent code paths before they are trusted (for example the encoder and a hand-built byte string).
- A conformance runner crate (`ambient-conformance`) will be added when the first vectors land.
