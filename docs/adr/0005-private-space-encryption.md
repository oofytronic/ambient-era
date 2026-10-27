# ADR-0005: Private Spaces are sealed to member device keys, not MLS groups

**Status:** Accepted

Supersedes the encryption mechanism in [VISION.md](../../VISION.md) §5 and §7, the "MLS for every Space" row in [docs/STACK.md](../STACK.md), the crate rule in `crates/ambient-spaces/AGENTS.md`, and the MLS-epoch sentence in [ADR-0001](0001-creations-as-spaces.md) point 5. Does not supersede ADR-0001's model: a creation is a Space, and access is still a ladder of roles and grants.

## Context

A Space is a membership list plus the merge of each member's signed, append-only log. That merge is required to be commutative: the same events in any order produce the same state (`ambient-sync`). MLS group state is the opposite. Commits are totally ordered, and two members who commit while offline fork the group. The relay would have to order the repair. That fights local-first sync.

MLS was also aimed at forward secrecy and post-compromise security. [THREAT_MODEL.md](../../THREAT_MODEL.md) does not promise either for v0.1. A stolen device is out of scope until a later tier says otherwise. The OpenMLS spike (`docs/spikes/06-openmls.md`) ran and was not adopted: no third-party audit, a license exception the dependency policy does not allow, and no interop test. VISION §14 already named that outcome as the tripwire.

The scale table never used one cipher for everything. Large and public rooms were already signed logs, not encrypted groups. SimpleX was adopted only as one-way mailbox addresses, not as a replacement for Spaces.

## Decision

- A **private Space** seals each event to the current encryption key of each member device, using HPKE (RFC 9180) from the RustCrypto crates. One encapsulation per device. A fresh ephemeral sender key per event. No hand-rolled ratchet, KDF, or cipher mode.
- Membership changes are ordinary signed events. Adding a device includes a checkpoint of history sealed to that device. Removing a device means later events are not sealed to it. Events it already opened stay opened.
- The same mechanism covers the Notes space (the owner's devices), a 1:1 Space, and a later small private group. Private Spaces are for a handful of members. The spaces spec will set a numeric cap before groups ship. This design does not claim rooms of hundreds.
- A **public Space** is a signed log. It is not sealed. Clients and the UI must not call it end-to-end encrypted.
- MLS, sender keys, and any other group ratchet are not a one-way door. Revisit them only if the threat model is raised to cover theft of a device key, and only with a library that is audited and already allowed by `deny.toml`.
- v0.1 ships the Notes space and 1:1 Spaces on desktop. Personal notes are not deferred to the long-form creation work. Content-free push ships with the first phone client.

## Consequences

- Offline edits commute. There is no epoch to catch up.
- There is no forward secrecy against compromise of a device's long-term encryption key. The threat model says so.
- Fanout cost grows with the number of member devices. That is acceptable only while Spaces stay small.
- Share revocation stays best-effort, as ADR-0001 already required. Honest clients purge. Hostile clients keep what they decrypted.
- `ambient-spaces` must not grow an MLS dependency. Spike 6 stays in the tree as evidence, marked not adopted.

## Alternatives considered

- **MLS for every private Space.** Rejected. It conflicts with commutative merge, and the evaluated library did not clear the dependency or audit bar.
- **A pairwise double ratchet for 1:1 and a different scheme for groups.** Rejected for now. It is two protocols before there is a second person in a group, and the official Signal library's license does not fit the dependency policy. HPKE is one construction the stack already planned to use.
- **Last-write-wins without sealing to each device.** Rejected. The relay would see plaintext, or a single device would silently drop another device's offline edits.
