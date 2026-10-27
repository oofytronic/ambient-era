# History

A log of meaningful decisions. Newest first. This is not a commit changelog.

When a turn changes direction, scope, or a locked design choice, add an entry here in the same change. Link the ADR or spec that records it. Do not relitigate an entry in a later one; supersede it.

## 2026-10-27 — Spaces stay, MLS does not

The owner and Grok reconciled the written plan (README, VISION, STACK, ADR-0001 through ADR-0004, the OpenMLS spike) with a later decision against MLS.

Kept:

- A Space is a membership list plus the merge of each member's signed log.
- SimpleX contributes one-way mailbox addresses only. It is not a replacement for Spaces.
- Rust core, Dioxus on desktop first, native mobile later.
- Key-event log. Passkeys unlock the seed. They are not the root.
- Chat, membership, and feeds are logs. A CRDT is for document bodies only.
- The relay stores ciphertext and must not learn a global identity.
- No wallet. No agent runtime in v0.1. An agent, later, is a delegated key with a grant. A shared Space still needs every member's consent before an agent can read it.

Changed:

- MLS is not the cipher for a private Space. [ADR-0005](docs/adr/0005-private-space-encryption.md) supersedes that sentence wherever VISION, STACK, or a crate rule still said it. The OpenMLS spike is kept as evidence and marked not adopted.
- A private Space seals each event to the members' current device keys with HPKE (RFC 9180). There is no second, hand-rolled ratchet.
- v0.1 is the owner's Notes space and 1:1 conversations, on desktop first. Personal notes are not deferred to v0.3. Phone push waits for the phone client.
- v0.1 promises content confidentiality and unlinkability only. See [THREAT_MODEL.md](THREAT_MODEL.md).

Not done yet:

- The normative specs in `docs/spec/` (events, identity log, Spaces, mailbox API) and their conformance vectors. No crate behavior until those exist.
