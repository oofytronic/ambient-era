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

## One primitive

A Space is the only membership model: a membership list plus the merge of its members' signed logs, sealed with HPKE to current member keys when the Space is private.

Anything new is one of these, and not a third mechanism:

- A Space of this same kind (a notebook is the owner's devices, a 1:1 is two members, a later group is N members, a later creation is its own Space).
- A capability grant on a Space (narrowed, expiring, revocable access). That grant is v0.4, not a new protocol invented beside Spaces.
- A copy. Putting note text into a chat copies the Markdown into a `msg.put`. The contact does not join the Notes space. The citation grants no key and no later edits.

Do not add a per-share Space type, a second access list, or a "deleted everywhere" promise. Revocation stops future seals. It does not pull back plaintext a member already opened. See [THREAT_MODEL.md](THREAT_MODEL.md).

## Larger project, not now

| Later | What it is | Why it waits |
|:--|:--|:--|
| Small private groups | The same Space and the same HPKE fanout | The cap and the UI wait until 1:1 is in use |
| Long-form creations | A Space per book or newsletter, with versions and links | Spec 05 is not written |
| Weave | A citation from a chat to a creation, already just a copied message in v0.1 | The sidebar is a projection, not a new protocol |
| Public identity and feed | Signed, unsealed logs. A second persona. | Must not be built by relaxing the private-Space rules |
| Phone clients | SwiftUI and Compose over the same core. Content-free push. | Desktop first |
| Agent | A delegated key plus a grant. Every member must agree before it reads a shared Space | No agent runtime in the core |

These are the structural problems that decide whether a later version is still user-owned. They are not v0.1 work. Do not scaffold them. Each one has to be a Space, a grant on a Space, or a copy. None of them is a second authority beside the owner.

| Problem | The constraint | What it is not |
|:--|:--|:--|
| Recovery for people who will lose a phrase | A threshold of guardians or devices can authorize a key rotation after a timelock. No single party can seize the root. The v0.1 phrase remains the backup until that exists. | A custodian. A passkey that is the root. |
| Someone has to keep nodes on | People who need a relay, a host, or compute pay the operator who provides it. Reputation is uptime and quality. Most people stay clients. | A token that is the product. An obligation to run a home server. |
| Agents that can publish or spend | An agent starts with nothing. It receives a short-lived scoped grant. The grant expires or is revoked. Enforcement is on a device or node the owner controls. Consequential actions are in a local log the owner can read. | Standing power. A model that is trusted because it said it would ask. |
| Sensing | Inference and temporary context stay on the owner's devices. Raw sensor data leaves only when chosen for one task. Sensing has a visible indicator. Agents ask for the narrowest context. | A permanent life log. A wearable that uploads by default. |
| Nodes that sleep | Public content can be mirrored as signed snapshots the mirror cannot alter. Relays are several and interchangeable. An agent that acts while the owner is offline already holds a grant, or it escalates to a person or another device. Intermittent nodes are the normal case. | A home server that must stay up for a public page to exist. |
| Finding anyone without a center | Signed announcements, local discovery, and an out-of-band introduction (the v0.1 link or QR). An operator may publish a bootstrap set. The protocol still works if that set is gone. | A directory that is the only way in. |
| Abuse without a central delete | Clients filter. People and communities share blocklists. A node refuses content under its own policy. The operator's nodes are legally separate from nodes other people run. | A moderator who can erase someone else's log. |

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

The CLI and the Linux window append notes into a sealed file, pair a second folder that does not hold the root, and sync those notes through `ambient-relay` on `127.0.0.1`. They can open one conversation and exchange text the same way. The window is a notes and chat shell: Sync sits in the toolbar, an invite is part of the conversation, and devices and recovery are their own screens. It calls the client in `ambient-cli` and does not seal events itself. There is a recovery seal, a contact invite as a link or a QR, a revoke command, and a command that copies a note into a chat. Message events carry the advisory unix time from the event envelope. The key file can be wrapped with a passphrase; that passphrase is not a platform passkey. Search uses a SQLCipher FTS5 index next to the sealed log. The log is still the source of truth, and `ambient-store` is still not SQLCipher. Two folders on one computer is not a second machine, and the cleartext relay is not a public deployment.

## What happens to a note

This is the path the spec requires. The CLI and the window do steps 1, 3, and 4. They write the recovery copy below. The window can show the phrase and restore from it. The sealed file is still the log. The native client also projects that log into a SQLCipher FTS5 index. `ambient-store` itself is not SQLCipher.

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
