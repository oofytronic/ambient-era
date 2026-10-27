# Architecture decision records

Decisions that are hard to reverse, or that others will wonder about later, are recorded here. One short file each, numbered in order, never rewritten after acceptance (supersede with a new ADR instead).

| # | Title | Status |
|:--|:--|:--|
| [0001](0001-creations-as-spaces.md) | Creations are Spaces, shared along an access ladder | Accepted |
| [0002](0002-rust-core-native-clients.md) | Rust core, native mobile clients, Dioxus for web and desktop | Accepted, pending proof spikes |
| [0003](0003-workspace-and-tooling.md) | Workspace layout, lints, dependency policy, and CI | Accepted |
| [0004](0004-licensing.md) | Licensing | Deferred |
| [0005](0005-private-space-encryption.md) | Private Spaces are sealed with HPKE, not MLS | Accepted |

## Template

```markdown
# ADR-NNNN: Title

**Status:** Proposed | Accepted | Superseded by ADR-NNNN

## Context
What forces are at play, and what problem needs deciding.

## Decision
What we decided, in plain sentences.

## Consequences
What gets easier, what gets harder, what we accept.

## Alternatives considered
What else we looked at and why not.
```
