# Identity log

**Version:** 1
**Status:** normative for v0.1

The identity is a key-event log. It is not a `did:key`. The root seed never appears on the wire. Passkeys and biometrics unlock the seed on the device. They are not keys in this log.

Events use the [envelope](01-events.md). Identity events set `space` to null and are signed by the identity key, except where a section below says otherwise. None of these events are signed by a passkey.

## Derivation

The root seed is 32 bytes from a CSPRNG. The recovery phrase is the BIP39 mnemonic of that seed. BIP39 is a backup encoding, not an on-wire format.

The identity signing seed is HKDF-SHA256 (RFC 5869):

- input keying material: the root seed
- salt: the UTF-8 string `ambient-era/v1`
- info: the UTF-8 string `identity-sign`
- length: 32

That 32-byte output is the Ed25519 seed. The identity public key is the matching public key.

Two further values use the same salt and input, with different info strings:

- `identity-seal`, 32 bytes, an X25519 private key. Clients seal a recovery copy of the identity log and the Notes space to its public key.
- `recovery-mailbox`, 32 bytes, the mailbox id where those recovery blobs are put.

Device keys and per-contact keys are generated on the device, not derived from the root. A mnemonic recreates the identity key, the seal key, and the recovery mailbox. It does not recreate device keys. Those are re-paired.

Vector `conformance/vectors/identity/0001-genesis.json` pins all three derivations for root seed `00 01 .. 1f`.

## Where the root lives

The root is required to append to the identity log. Pairing a device MUST NOT copy the root onto it. A paired device can write in Spaces it belongs to. It cannot add or revoke devices. Authorizing a device requires a machine that already holds the root, unlocked locally, or a new install from the mnemonic.

Every device that holds the root also holds the ability to bless another device. Losing that machine is the stolen-device case in the threat model. Revoking its device key does not help if the thief already copied the root. The mnemonic is how the owner starts a new log of device membership from a machine the thief does not have, and then revokes the old keys.

## Recovery copy

Clients that hold the root MUST seal the identity log and the accepted Notes-space events to the identity seal key and put those blobs in the recovery mailbox, using the same HPKE construction as any other inbox. Conversation history SHOULD be included.

A new install from the mnemonic derives the seal key and the mailbox id, fetches the blobs, and projects them. It then revokes any device key the owner no longer has.

## Genesis

The first identity event is `id.genesis`. `prev` is null. The signer is the identity public key.

```cddl
genesis-payload = { 1 => "ambient-era" }
```

A log with any other first event, or with a second `id.genesis`, is `bad_payload`.

## Devices and contact keys

`id.device-add` authorizes one signing key and one encryption key.

```cddl
device-add = {
  1 => bstr .size 32, ; Ed25519 signing key
  2 => bstr .size 32, ; X25519 encryption key
  3 => tstr .size (1..32), ; local label, never sent to a relay as an identity
  4 => bstr .size 32  ; inbox mailbox id
}
```

The same event type is used for two different jobs, and they MUST be kept apart by what they sign:

- A **device key** signs events in the owner's Notes space and nothing addressed to another person.
- A **contact key** is a fresh keypair created for one relationship. It signs events in that conversation Space and is the only key that other person is shown.

Using a Notes-space device key as the signer inside a conversation would show that person a key that also identifies the notebook. Clients MUST NOT do that.

`id.inbox` rotates the mailbox for a key that was already added. It is signed by the identity key.

```cddl
inbox = { 1 => bstr .size 32, 2 => bstr .size 32 } ; signing key, new mailbox
```

`id.device-revoke` is signed by the identity key.

```cddl
revoke = {
  1 => bstr .size 32,       ; signing key
  2 => bstr .size 32 / nil  ; up_to, last accepted event id from that signer
}
```

Signature checks on that signer's past events still succeed. Application of those events changes:

- Events that are `up_to`, or reachable by following `prev` from `up_to`, stay in the projected state.
- Any other event from that signer is `revoked` and MUST NOT be applied.
- Clients MUST stop sealing new payloads to that signer's encryption key.
- `up_to` null means the signer never wrote an accepted event. Nothing from them is applied.

Revocation does not delete plaintext the signer already decrypted.

Identity rotation, social recovery, and a second identity key are not in this version. A client MUST reject those types rather than ignore them.

## What is shown to whom

| Audience | Sees | Does not see |
|:--|:--|
| Relay | Mailbox ids and ciphertext | Identity public key, device keys, labels |
| Another person | The contact key made for them, inside sealed events | The identity public key, and device keys used for the Notes space |
| The owner's other devices | The identity log, sealed to their device keys | — |

There is no global user id in any field a relay or a contact has to store.

## Pairing a device

The new device generates a device signing key, an encryption key, and a 32-byte inbox mailbox, and shows them as a QR code (canonical CBOR, then unpadded base64url):

```cddl
pair-offer = {
  1 => 1,
  2 => bstr .size 32, ; signing key
  3 => bstr .size 32, ; encryption key
  4 => bstr .size 32  ; inbox mailbox
}
```

The device that already holds the identity key requires the local unlock, appends `id.device-add`, and puts the sealed identity log and a Notes-space checkpoint into that inbox. The checkpoint is ordinary sealed history, not a different cipher. See [03-spaces.md](03-spaces.md).

## Changelog

- v1 (2026-10-27): genesis, device add, inbox rotation, revoke-by-ancestor. No rotation of the identity key.
