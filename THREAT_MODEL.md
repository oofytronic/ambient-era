# Threat model

What this system promises, and what it refuses to promise. [VISION.md](VISION.md) §3 is the background. This page is the one to follow. If a doc, a UI string, or a comment claims more than the tier that has actually shipped, it is wrong. See the honest-claims rule in [AGENTS.md](AGENTS.md).

**Status:** accepted with [ADR-0005](docs/adr/0005-private-space-encryption.md). v0.1 targets T0 and T1 only.

## What we are protecting

The owner's notes, and later the contents of private conversations, against a relay operator and a network observer who can read every byte the relay stores and every packet on the path. Also against that relay learning that a set of Spaces belong to one person.

The first user is the owner. v0.1 is a desktop daily driver for that person and a few 1:1 conversations. It is not a public network.

## Tiers

| Tier | Promise | When it is honest to claim it |
|:--|:--|:--|
| **T0 Content** | A relay and a network observer cannot read the contents of a private Space. | v0.1, once events are sealed before they are uploaded. |
| **T1 Unlinkability** | There is no global user id on the wire. Contacts see pairwise pseudonyms. Mailbox addresses do not say whose they are, and the relay is not given a key that ties one person's Spaces together. | v0.1, by construction of identity and mailboxes. Not a claim about traffic analysis. |
| **T2 Metadata minimization** | Sealed-sender submission, padded sizes, jittered timing, content-free push. | v0.2 through v0.5. Not v0.1. |
| **T3 Network anonymity** | The relay cannot see your IP. Tor or an equivalent, opt-in. | v0.8. |
| **T4 Anti-coercion** | Duress PIN, hidden vaults, remote wipe. | After 1.0, if ever. |

Until T3 ships, call the product private and sovereign. Do not call it anonymous.

## What v0.1 does not defend against

Say these limits in the product when a person could reasonably think otherwise.

- **A stolen or seized device that is unlocked, or whose keystore is extracted.** Events already sealed to that device key can be opened. HPKE to long-term device keys does not give forward secrecy or post-compromise security. MLS was considered for those properties and rejected for v0.1 ([ADR-0005](docs/adr/0005-private-space-encryption.md)). A lost device is handled by revocation: new events are no longer sealed to it. Old ones are not pulled back.
- **Someone who already received an event.** Removing a member, or expiring a share, stops future events. It does not erase plaintext a hostile client saved. The UI must not say "deleted everywhere" or "cannot be copied."
- **A global passive adversary, a malicious relay that withholds or reorders blobs, or a network that blocks the relay.** Clients detect gaps. They do not pretend the relay is honest about delivery.
- **Traffic analysis.** T1 means the protocol does not hand the relay a stable user id. It does not mean timing, volume, and IP addresses reveal nothing. The owner runs the relay in v0.1, so the relay operator is the owner.
- **The person's other devices, while those devices are still members.** They are supposed to read the Notes space.
- **A public Space.** Public events are signed and readable. Do not describe them as end-to-end encrypted.

## Trust assumptions for v0.1

| Party | Trusted for | Not trusted for |
|:--|:--|:--|
| This device, after unlock | Holding the device keys and the local plaintext. | — |
| The OS keystore | Keeping the seed and device keys out of the database file. | Hiding them from a compromised OS. |
| Other devices of the same identity | Reading what they were added to see, until revoked. | Staying honest after revocation. |
| The other person in a 1:1 Space | Having been chosen by the owner. They can read what was sealed to them. | Keeping copies, or keeping malware off their device. |
| The relay | Storing and returning opaque blobs. | Reading content, identifying the owner across Spaces, or deleting data only the client can actually purge. |
| The network | Delivering packets. | Confidentiality or anonymity. |

## Consequences for the design

- Seal before upload. The relay crate must not be able to decrypt.
- Address delivery with one-way mailbox ids, not a username inbox.
- Derive pairwise pseudonyms. Do not put the identity-log hash in a place a contact or a relay must see.
- Keep the local search index on the device, inside the encrypted database, never on the relay.
- Logs and crash reports contain no note text, message text, keys, mailbox ids, or tokens.
- A future agent is not a new trust tier. It is a delegate. It can read a shared Space only after every member has agreed, and only inside a grant that expires.
