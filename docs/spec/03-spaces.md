# Spaces

**Version:** 1
**Status:** normative for v0.1

A Space is a membership list and the merge of its members' signed logs. Membership and application events use the [envelope](01-events.md). Private Spaces are sealed with HPKE to member device keys ([ADR-0005](../adr/0005-private-space-encryption.md)). This spec does not use MLS.

## Kinds

`space.genesis` creates a Space. Its envelope `space` field is null. The Space id is the event id of the genesis event.

```cddl
genesis = {
  1 => "notes" / "conversation",
  2 => [+ member]
}
member = {
  1 => bstr .size 32, ; signing key
  2 => bstr .size 32, ; X25519 key
  3 => 1 / 2,        ; 1 owner, 2 writer
  4 => bstr .size 32  ; inbox mailbox
}
```

The signer MUST be one of the members and MUST have role 1. Any other kind, including `public`, is `bad_payload` in this version. Public signed logs are a later spec, not a quiet variant of this one.

Role 3 and every other role number are `bad_payload`. Read-only and share expiry are not in this version. Do not invent them in the client.

## Membership changes

Signed by a current owner. Applied only if the signer's key is still an owner in the projected membership at that point in the merge.

```cddl
member-add = member
member-remove = { 1 => bstr .size 32 }          ; signing key
inbox = { 1 => bstr .size 32, 2 => bstr .size 32 } ; signing key, new mailbox
```

Types: `space.member-add`, `space.member-remove`, `space.inbox`.

Adding one of the owner's devices to the Notes space MUST include a checkpoint: the accepted events of that Space, sealed to the new encryption key, written to the new inbox. Adding anyone else SHOULD include a checkpoint when the owner wants them to see history, and MUST NOT include one when the owner wants them to see only what happens next. Neither choice un-sends what a removed member already opened.

A removed member's earlier events stay. New events MUST NOT be sealed to their key. Events they sign after removal are `not_member` and MUST NOT be applied.

## Notes and messages

`note.put` is valid only in a Space whose kind is `notes`. `msg.put` is valid only in a Space whose kind is `conversation`. The other combination is `wrong_kind` and MUST NOT be applied.

```cddl
note-put = {
  1 => bstr .size 32,     ; note id, random, stable
  2 => tstr .size (0..200),
  3 => "text-snapshot-v1",
  4 => bstr,              ; UTF-8 Markdown
  5 => [* bstr .size 32]  ; parent note.put event ids
}
msg-put = {
  1 => tstr .size (0..16000),
  2 => bstr .size 32 / nil, ; reply_to event id
  3 => bstr .size 32 / nil  ; note id this text was copied from
}
```

`text-snapshot-v1` is the whole Markdown document, not a CRDT. A CRDT codec is not selected. Two `note.put` events for the same note id are concurrent when neither event id appears in the other's parent list, walking parents transitively. Clients MUST keep both and MUST NOT pick a winner. The UI shows a conflict. Replacing this codec later is a new type string, not a silent change of these bytes.

`msg.put` field 3 is a citation. It grants no key, no membership, and no later edits. The message text is a copy.

Writers (role 2) MAY append `note.put` and `msg.put`. They MUST NOT append membership events. Owners MAY append either.

## Merge

Project state by taking the set of applied events. Order of arrival does not matter. Parent links, signer chains, and membership events are the only structure. The relay's cursor is not an order.

## Sealing

Private Space wire bytes are sealed before they are given to a relay. One HPKE base-mode seal per recipient, with a fresh ephemeral key each time.

Suite, and no other:

- KEM: DHKEM(X25519, HKDF-SHA256), identifier `0x0020`
- KDF: HKDF-SHA256, identifier `0x0001`
- AEAD: ChaCha20-Poly1305, identifier `0x0003`
- mode: Base (0)
- `info`: UTF-8 `ambient-era/hpke/v1`
- associated data: UTF-8 `ambient-era/aad/v1`, then the 32-byte mailbox id, then the 32-byte recipient encryption key

The recipient's public key MUST NOT be written in the clear next to the ciphertext. The hint is the first 8 bytes of BLAKE3 of UTF-8 `ambient-era/hint/v1`, then the mailbox id, then the recipient encryption key. A client matches a hint against its own keys and then opens the seal. A hint match is not authentication. Opening the seal is.

```cddl
sealed-blob = {
  1 => 1,
  2 => [+ box]
}
box = { 1 => bstr .size 8, 2 => bstr .size 32, 3 => bstr } ; hint, enc, ciphertext
```

The plaintext of an event blob is the wire event. Clients put one blob to each recipient inbox, sealed to that one key. They MUST include the sender's other devices that are members.

This construction does not provide forward secrecy or post-compromise security if a recipient's long-term encryption key is stolen. [THREAT_MODEL.md](../../THREAT_MODEL.md) says so.

Vector `conformance/vectors/spaces/0001-hpke-blob.json` seals the plaintext `hello` with fixed secrets. The generator checks its HPKE code against a CFRG RFC 9180 vector before writing it. The Ambient bytes are still provisional until the Rust core reproduces them.

## Changelog

- v1 (2026-10-27): notes and conversation Spaces, HPKE to member keys, snapshot notes with an explicit conflict rule.
