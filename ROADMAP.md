# Roadmap

The order of work. [ARCHITECTURE.md](ARCHITECTURE.md) is the shape of the system. [THREAT_MODEL.md](THREAT_MODEL.md) is what each stage is allowed to claim. This file is the sequence.

No dates. A stage starts when the previous one is actually in use, not when the code compiles. v0.1 is the only stage that starts now.

## Where this is

Specs 01 through 04 are written. `ambient-events` reproduces the event vectors. `ambient-identity` derives the root, signs genesis, and projects device add, inbox rotation, and revoke-by-ancestor. `ambient-spaces` projects a Notes space, including a conflict between two devices, and a conversation. The Linux window creates, edits, and searches notes, pairs a second folder, and exchanges one 1:1 through the loopback relay. The window is Iced, not a webview. The notes file is sealed with ChaCha20-Poly1305. It is not SQLCipher, and the key file is not the OS keystore. HPKE seals events to other devices. The recovery seal is not implemented. The relay binds `127.0.0.1` and speaks cleartext, so it is not a public deployment. Spec 05 is intentionally unwritten.

## v0.1 — notes and a private 1:1

Desktop only. One person, then a few conversations. No agent, no groups, no phone push, no public feed.

Build in this order. Do not skip ahead.

| Step | Work | Done when |
|:--|:--|:--|
| 1 | `ambient-events` reproduces the event vectors | Done. Canonical CBOR, ids, and signatures match `conformance/vectors/events/` |
| 2 | Identity log | Derivation, genesis, device add, inbox rotation, revoke-by-ancestor, and the pairing-offer bytes are in. The recovery seal is not. |
| 3 | Local store and notes | The `ambient` CLI creates, edits, and searches notes offline. The store is an encrypted file, not SQLCipher yet. |
| 4 | Desktop app | The Linux window is Iced over the same folder as the CLI. Not the OS keystore. |
| 5 | Relay and a second device | The window pairs a second folder and a note shows up there after Sync in both. The relay stores opaque blobs and binds `127.0.0.1` in the clear, which spec 04 says is not a public deployment. Two folders on one computer, not two machines. |
| 6 | 1:1 | The window opens one conversation. Text is sealed to a per-contact key. The invite is the contact card as text, not a QR image. Messages show up after Sync in both windows. |
| 7 | Devices, activity, copy | Revoke a device. See which device did what. Copy a note into a chat as text, not as a grant |

**Exit.** The owner uses the notes for real work, and uses 1:1 with at least one person, including an offline gap and one restore from the recovery phrase. A green test run is not the exit.

**Still true at the end of v0.1.** Concurrent edits of one note are kept as a conflict, not merged. A stolen device key can open what was already sealed to it. The product is not anonymous.

## After v0.1

Each row is a usable product. None of them is permission to start early.

| Version | Ships | Does not ship | Why it is next |
|:--|:--|:--|:--|
| **v0.2** | Small private groups on the same HPKE sealing. A numeric member cap in the spaces spec. Encrypted attachments. Member remove, block, and a rate limit on first contact. Content-free push, and only because a phone client exists. | Rooms of hundreds. A group ratchet. MLS. | The Space already has membership. This is the first time more than two people share one. |
| **v0.3** | Long-form creations, each its own Space. Spec 05. Versions, links between pieces, Markdown export. | A CRDT, unless a spike has picked Loro or Automerge and a new payload type. Collaborative writing. | The notebook already exists. This is for a work that needs its own members. |
| **v0.4** | Weave. A creation shows the conversations about it. The capability API: a grant that can be narrowed, expired, and revoked. | An agent that uses those grants. A mini-app platform. | Sharing and a later agent must be the same permission, designed once. |
| **v0.5** | One-command self-host. More than one relay. Full export and import of the logs and the blobs. | A hosted service as the only way to sync. | Sovereignty you can check, not a claim in the README. |
| **v0.6** | A separate public identity. Profiles, a follow graph, a pull-based feed of published creations. A Nostr read and cross-post bridge. | An algorithmic feed. Encrypting the public log. Linking the public identity to the private one by default. | The first public thing is a published creation, not a bare post. |
| **v0.8** | Optional Tor. Direct connections when both peers are reachable, behind the transport trait. Relay blinding. | A promise of anonymity before this ships. | T3 in the threat model. |
| **v0.9** | An independent review of the protocol and the implementation. Findings fixed in public. | Calling the review a formality. | v1.0 cannot claim the threat model without this. |
| **v1.0** | A local agent that is a delegated key and a client of the v0.4 grants. Tipping through an external wallet. The protocol version frozen. | An agent in a shared Space without every member's consent. A wallet inside the app. An orchestrator that competes with vendor bots. | The thesis: private conversation, owned writing, and a public presence on one identity you can take elsewhere. |

## After 1.0

Themes only. They are not scheduled, and they do not change v0.1.

| | Theme |
|:--|:--|
| v1.5 | Polish a stranger can recommend. Import from other messengers. Push that stays content-free. |
| v2.0 | Mini-apps on the same grants the agent already uses. |
| v2.5 | Communities, roles, and portable credentials. |
| v3.0 | Calendar, files, and bridges to other networks. Still no embedded wallet unless the evidence says otherwise. |
| v4.0 | The ambient floor: mesh, hardware keys, and moving off a host without asking its permission. |

## Rules that apply at every version

- Spec and vectors before behavior.
- A private Space stays a membership list plus a commutative log, sealed with HPKE, until a new ADR says otherwise.
- The relay stores ciphertext. It is not given a user id.
- Do not call the product anonymous before v0.8.
- Do not run an agent before the grant API exists, and do not put one in a shared Space without every member's consent.
