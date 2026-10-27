# Event envelope

**Version:** 1
**Status:** normative for v0.1

Every log entry, including identity-log entries, uses this envelope. Payload meaning is defined by the spec for that type. This document does not define notes, membership, or mailboxes.

Conformance: `conformance/vectors/events/`. Those vectors were produced by one generator (`conformance/tools/generate.py`) and are provisional until the Rust core reproduces them.

## Encoding

The unsigned event is a CBOR map (RFC 8949) with the integer keys below, and only those keys. Encoders MUST produce the canonical form in this section. Decoders MUST reject anything else with `noncanonical`.

Canonical form:

- Definite lengths only. Indefinite byte strings, text strings, arrays, and maps are rejected.
- Integers are encoded in the shortest form. A value under 24 MUST be a single byte.
- Map keys are sorted by the bytewise order of their canonical encodings. Encoders MUST emit them in that order. Duplicate keys are rejected.
- No floating-point values, no tags, no simple values other than null, and no negative integers.
- Text is UTF-8. Ill-formed UTF-8 is rejected.
- The decoder MUST re-encode the unsigned map and reject the input if the bytes differ.

```cddl
unsigned-event = {
  1 => 1,                  ; version
  2 => bstr .size 32,      ; signer, Ed25519 public key
  3 => bstr .size 32 / nil,; prev, prior event id from this signer
  4 => uint / nil,         ; unix milliseconds, advisory, not an order
  5 => bstr .size 32 / nil,; space id, nil on identity events and on space.genesis
  6 => tstr .size (1..64), ; type, ASCII [a-z0-9.-] only
  7 => bstr                ; payload, itself canonical CBOR for known types
}
```

The wire form is a CBOR array of the unsigned map and the signature. The signature is not inside the hashed bytes.

```cddl
wire-event = [ unsigned-event, bstr .size 64 ]
```

`unix milliseconds` is a claim a client may display. Verifiers MUST NOT use it to order events, to judge revocation, or to accept a signature.

## Identifiers and signatures

`event id` is BLAKE3-256 of the canonical unsigned-event bytes, 32 bytes.

The signature is Ed25519 (RFC 8032) over those same canonical bytes, not over the event id and not over the wire array. The signing key's public half MUST equal field 2. A mismatch or a bad signature is `bad_signature`.

Verifiers MUST check the signature before interpreting the payload.

## Chains

Field 3 is the previous event id signed by the same signer, or null when that signer has no previous event. A signer MUST NOT create two events with the same `prev`. A verifier that sees a fork MUST apply the event with the lexicographically smaller event id and reject the other with `fork`.

Absence of the named previous event is not a rejection. The client holds the event as `gap` until that predecessor arrives. Application specs say whether a gap blocks projection.

## Types

Field 6 is a type name owned by another spec:

| Prefix | Spec |
|:--|:--|
| `id.` | [02-identity-log.md](02-identity-log.md) |
| `space.` `note.` `msg.` | [03-spaces.md](03-spaces.md) |

An unknown type inside a prefix this version understands MUST be rejected with `bad_type`. Do not skip it. Skipping an unknown identity event could skip a revocation.

Field 7 is an opaque byte string at this layer. The type's spec defines the canonical CBOR inside it. A payload that is not canonical CBOR, or that does not match that type, is `bad_payload`.

## Errors

Stable codes: `noncanonical`, `bad_version`, `bad_signature`, `bad_type`, `bad_payload`, `fork`, `truncated`.

`bad_version` is field 1 not equal to 1. Later versions of this spec will say how a decoder treats a higher version. This version rejects them.

## Changelog

- v1 (2026-10-27): initial envelope. No MLS framing.
