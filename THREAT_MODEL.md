# Threat model

What this system promises, and what it refuses to promise. [VISION.md](VISION.md) §3 is the background. This page is the one to follow. If a doc, a UI string, or a comment claims more than the tier that has actually shipped, it is wrong. See the honest-claims rule in [AGENTS.md](AGENTS.md).

**Status:** accepted with [ADR-0005](docs/adr/0005-private-space-encryption.md). v0.1 is one computer and promises the at-rest seal below. v0.2 is the first version in which events leave the device, and it targets T0 and T1.

## What we are protecting

The owner's notes, and later the contents of private conversations, against a relay operator and a network observer who can read every byte the relay stores and every packet on the path. Also against that relay learning that a set of Spaces belong to one person.

The first user is the owner. v0.1 is a desktop daily driver for that person on one computer. v0.2 adds a second computer and a few 1:1 conversations. Neither is a public network.

**What v0.1 promises.** The notes log is sealed on disk under a key derived from the root, and the root is in the key file. Someone who has the notes file and not the key file cannot read it. The key file is plain unless the owner set a passphrase, and then it is wrapped. The key file is not the OS keystore. There is nothing on the network to promise about, because v0.1 sends nothing off this computer. The window still starts a relay on `127.0.0.1` that only this computer can reach. A copy of the notes exists elsewhere only if the owner makes one with Back up in settings, by hand. The copy holds the key file, so it is as readable as the folder is. Anyone who gets it can read the notes unless the key file is wrapped. A lost disk is lost notes if no copy was made.

## Tiers

| Tier | Promise | When it is honest to claim it |
|:--|:--|:--|
| **T0 Content** | A relay and a network observer cannot read the contents of a private Space. | v0.2, once events are sealed before they are uploaded. The code seals them already. |
| **T1 Unlinkability** | There is no global user id on the wire. Contacts see pairwise pseudonyms. Mailbox addresses do not say whose they are, and the relay is not given a key that ties one person's Spaces together. | v0.2, by construction of identity and mailboxes. Not a claim about traffic analysis. |
| **T2 Metadata minimization** | Sealed-sender submission, padded sizes, jittered timing, content-free push. | v0.2 through v0.5. Not v0.1. |
| **T3 Network anonymity** | The relay cannot see your IP. Tor or an equivalent, opt-in. | v0.8. |
| **T4 Anti-coercion** | Duress PIN, hidden vaults, remote wipe. | After 1.0, if ever. |

Until T3 ships, call the product private and sovereign. Do not call it anonymous.

## What v0.1 does not defend against

Say these limits in the product when a person could reasonably think otherwise.

- **A stolen or seized device that is unlocked, or whose keystore is extracted.** Events already sealed to that device key can be opened. HPKE to long-term device keys does not give forward secrecy or post-compromise security. MLS was considered for those properties and rejected for v0.1 ([ADR-0005](docs/adr/0005-private-space-encryption.md)). A lost device is handled by revocation: new events are no longer sealed to it. Old ones are not pulled back.
- **Someone with the disk.** The notes log, the conversation logs, and the search index are sealed. A few small files beside them are plain text: `names` (what you call people), `profile` (the device name, the theme, how times are shown, where home is as a time zone name, where the last backup went and when), `seen` (how far each conversation was read), `dictionary` (words you told the spellcheck are fine), `order` (the order you put the notes of each group in, with the group's name, which is a word from your notes), and `blocked` (signing keys this computer will not invite). The `chat-files` folder holds files a conversation cited, in the clear, once they have been opened. The `export` folder, if Export notes was used, is readable Markdown of the notes, with no lock at all. None of these holds a key, but they can hold names and words that came from notes. Erasing the notebook removes them. A backup copy leaves out `export`, and holds everything else, including the key file. Do not describe the folder, or a copy of it, as encrypted unless the key file is wrapped.
- **Someone who already received an event.** Removing a member, or expiring a share, stops future events. It does not erase plaintext a hostile client saved. The UI must not say "deleted everywhere" or "cannot be copied."
- **A global passive adversary, a malicious relay that withholds or reorders blobs, or a network that blocks the relay.** Clients detect gaps. They do not pretend the relay is honest about delivery.
- **Traffic analysis.** T1 means the protocol does not hand the relay a stable user id. It does not mean timing, volume, and IP addresses reveal nothing. The owner runs the relay in v0.2, so the relay operator is the owner.
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
