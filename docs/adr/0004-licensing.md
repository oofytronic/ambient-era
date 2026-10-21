# ADR-0004: Licensing

**Status:** Proposed. Needs the project owner's confirmation before any outside contribution is accepted.

## Context

Licensing is the hardest decision to reverse once others contribute, because relicensing needs every contributor's consent. We want independent clients and relays to exist (a stated long-term goal), and we do not want hosted forks to close the server or the apps.

## Decision (proposed)

| Part | License |
|:--|:--|
| Specifications and conformance vectors | CC0-1.0 |
| Core crates | Apache-2.0 OR MIT |
| Relay and client apps | AGPL-3.0-only |

The details, license texts, and per-path mapping are in [LICENSING.md](../../LICENSING.md).

## Consequences

- A permissive core lets other teams build clients, including closed ones. That is intentional, but it also means the core can be used in proprietary products.
- AGPL on the relay and apps means a company that hosts a modified relay must publish its changes.
- Contributions are accepted under these licenses.

## Open questions

- **`AGPL-3.0-only` or `AGPL-3.0-or-later`?** The scaffold uses `only`, the stricter choice, so no future license text is accepted on the project's behalf. `or-later` gives flexibility if the FSF revises the license.
- **DCO or CLA for contributors?** A Developer Certificate of Origin sign-off is lightweight and keeps copyright with contributors. A CLA would let the project relicense later, but it adds friction and trust cost. Recommendation: DCO, decided before the first outside contribution.
- Copyright holder name in the MIT text is currently "Ambient Era contributors"; change it if you want a named entity.

## Alternatives considered

- **AGPL everywhere:** strongest copyleft, but discourages independent clients that link the core.
- **Permissive everywhere:** maximum adoption, but allows closed hosted forks of the relay and apps.
- **Source-available licenses (BSL and similar):** not open source, and at odds with a sovereignty-focused project.
