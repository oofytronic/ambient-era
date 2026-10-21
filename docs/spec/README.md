# Specifications

Normative specifications for the wire formats and protocols. Code conforms to these documents, and [conformance vectors](../../conformance/README.md) check that it does.

**Status:** none written yet. They come first, before the behavior they describe.

## Planned documents

| # | Document | Covers |
|:--|:--|:--|
| 01 | `01-events.md` | Event structure, deterministic CBOR encoding, content IDs, signatures |
| 02 | `02-identity-log.md` | Key-event log, device delegation, rotation, recovery |
| 03 | `03-spaces.md` | Space membership and role log, MLS usage, share Spaces and expiry |
| 04 | `04-relay-api.md` | Relay put/get/delete/subscribe, mailbox addressing, retention |
| 05 | `05-creations.md` | Documents, collections, versions, `CreationRef`, comment anchors |

## Conventions

- Schemas are written in **CDDL** (RFC 8610) alongside the prose.
- Requirement words (MUST, SHOULD, MAY) follow RFC 2119.
- Each document carries a version and a changelog. Formats evolve additively; breaking changes need an ADR.
- Licensing is undecided (see [LICENSING.md](../../LICENSING.md)).
