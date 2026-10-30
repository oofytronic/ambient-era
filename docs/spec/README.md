# Specifications

Normative specifications for the wire formats and protocols. Code conforms to these documents, and [conformance vectors](../../conformance/README.md) check that it does.

**Status:** 01 through 04 are version 1, written for v0.1. 05 is not written. `ambient-events` implements the envelope. `ambient-identity` implements derivation and the identity-log rules through revoke. `ambient-spaces` projects notes and a conversation, and opens the published HPKE blob. It does not re-seal that blob's exact ciphertext (see spec 03). `ambient-transport` and `ambient-relay` implement the mailbox operations. The recovery seal is not implemented.

## Planned documents

| # | Document | Covers |
|:--|:--|:--|
| 01 | `01-events.md` | Event structure, deterministic CBOR encoding, content IDs, signatures |
| 02 | `02-identity-log.md` | Key-event log, device delegation, rotation, recovery |
| 03 | `03-spaces.md` | Space membership and role log, HPKE-to-members sealing ([ADR-0005](../adr/0005-private-space-encryption.md)), share Spaces and expiry |
| 04 | `04-relay-api.md` | Relay put/get/delete/subscribe, mailbox addressing, retention |
| 05 | `05-creations.md` | Not written. Documents, collections, versions, `CreationRef`, comment anchors. After v0.1. |

## Conventions

- Schemas are written in **CDDL** (RFC 8610) alongside the prose.
- Requirement words (MUST, SHOULD, MAY) follow RFC 2119.
- Each document carries a version and a changelog. Formats evolve additively; breaking changes need an ADR.
- Licensing is undecided (see [LICENSING.md](../../LICENSING.md)).
