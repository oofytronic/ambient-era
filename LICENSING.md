# Licensing

Each part of the repository has one license, chosen so that others can build independent clients while hosted forks of the server and apps cannot quietly close the code. The reasoning and the open questions are in [ADR-0004](docs/adr/0004-licensing.md).

| Part | License (SPDX) | Text |
|:--|:--|:--|
| Core crates in `crates/` (all except the relay) | `Apache-2.0 OR MIT` | [Apache-2.0](LICENSES/Apache-2.0.txt), [MIT](LICENSES/MIT.txt) |
| Relay (`crates/ambient-relay`) and client apps (`clients/`) | `AGPL-3.0-only` | [AGPL-3.0-only](LICENSES/AGPL-3.0-only.txt) |
| Specifications (`docs/spec/`) and conformance vectors (`conformance/`) | `CC0-1.0` | [CC0-1.0](LICENSES/CC0-1.0.txt) |
| Other documentation | `CC0-1.0` unless a file says otherwise | [CC0-1.0](LICENSES/CC0-1.0.txt) |

Crate manifests carry the `license` field for their crate. License texts live in `LICENSES/`, named by SPDX identifier.

**Status:** proposed. The licensing choice is awaiting confirmation by the project owner before any outside contribution is accepted (ADR-0004).
