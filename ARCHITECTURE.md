# Architecture

The map of the system as it is now. Decisions live in [docs/adr/](docs/adr/). Wire formats live in [docs/spec/](docs/spec/). Promises live in [THREAT_MODEL.md](THREAT_MODEL.md). If this file disagrees with an ADR or a spec, the ADR or the spec wins.

## v0.1 fence

v0.1 is a desktop daily driver for one person:

- An identity log, unlocked locally. A passkey is not the root.
- One Notes space containing that person's devices.
- One conversation space per contact, sealed to per-contact keys.
- A relay that stores opaque blobs addressed by random mailbox ids.
- Local search over the decrypted notes.

v0.1 does not include groups beyond a 1:1, public Spaces, phone push, Tor, payments, a CRDT library, or a running agent. Do not scaffold those "so they exist." Reserved names in the specs are rejected on purpose.

## Larger project, not now

| Later | What it is | Why it waits |
|:--|:--|:--|
| Small private groups | The same Space and the same HPKE fanout | The cap and the UI wait until 1:1 is in use |
| Long-form creations | A Space per book or newsletter, with versions and links | Spec 05 is not written |
| Weave | A citation from a chat to a creation, already just a copied message in v0.1 | The sidebar is a projection, not a new protocol |
| Public identity and feed | Signed, unsealed logs. A second persona. | Must not be built by relaxing the private-Space rules |
| Phone clients | SwiftUI and Compose over the same core. Content-free push. | Desktop first |
| Agent | A delegated key plus a grant. Every member must agree before it reads a shared Space | No agent runtime in the core |

## Pieces

```
desktop client (Iced on Linux)
        │ commands / queries
        ▼
ambient-core          facade. No UI types.
  identity, spaces, sync, store, docs, transport, capabilities
        │ sealed blobs
        ▼
ambient-relay         mailbox store. Cannot decrypt.
```

The relay crate depends only on transport. It must not depend on the store or the core.

Layering is in [AGENTS.md](AGENTS.md). Dependencies point down.

## What the code does today

The CLI appends notes into a sealed file, pairs a second folder that does not hold the root, and syncs those notes through `ambient-relay` on `127.0.0.1`. It can open one conversation and exchange text the same way. The Iced window reads and writes that notes file and does not open a socket. There is no recovery seal, no QR image, no revoke command, and no command that copies a note into a chat. The local file is not SQLCipher and has no FTS index. Search scans the decrypted notes.

## What happens to a note

This is the path the spec requires. The CLI does steps 1, 3, and 4. It does not write the recovery copy below. Step 2 is a sealed file and a scan, not SQLCipher or FTS. The window does step 1 only, with the network off.

1. The device appends a `note.put` event, signed by that device's key, linked to that device's previous event.
2. The local database projects it, and FTS indexes the plaintext. The database file is encrypted. The index never goes to the relay.
3. The wire bytes are sealed with HPKE to each other device in the Notes space and put in that device's inbox mailbox.
4. The other device opens the seal, checks the signature and the device chain, and projects the same way.

The machine that holds the root also writes a recovery copy, sealed to a key derived from the root, into a mailbox derived from the root. Pairing does not copy the root. A mnemonic rebuilds the identity and the notes on a new install. It does not recreate device keys. The owner then revokes keys they no longer have.

Two devices can both write a note while offline. `text-snapshot-v1` keeps both versions when neither is a parent of the other. It is not a CRDT. A CRDT codec is a later choice and a new type string.

A conversation is the same path with `msg.put`, signed by a per-contact key rather than the Notes device key. Copying a note into a chat copies the Markdown into a message. The contact is not added to the Notes space.

## What is deliberately absent

- MLS, or any totally ordered group commit. Merge has to work in any order.
- A global user id on the relay.
- Forward secrecy against theft of a device's long-term key.
- An embedded wallet.
- JavaScript in the product.

## Where to change something

| Change | Touch |
|:--|:--|
| A byte on the wire | The spec, then a vector, then code. Not the reverse. |
| Who may be in a Space | `docs/spec/03-spaces.md` and ADR-0001 if the access ladder changes |
| How sealing works | A new ADR. ADR-0005 is the current one. |
| A new crate or dependency | ADR-0003's policy, `deny.toml`, the crate's `AGENTS.md` |
| A product promise | `THREAT_MODEL.md` first. Then the README, if the claim is public. |
