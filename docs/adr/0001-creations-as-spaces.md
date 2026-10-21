# ADR-0001: Creations are Spaces, shared along an access ladder

**Status:** Accepted (model); staging proposed.

## Context

Creations (books, series, newsletters, note collections) need to be written privately, discussed with others, shared at different levels of trust, co-written, and eventually published. Conversations about a creation must stay attached to it. We want one mechanism for all of this, and we want the later social features to reuse it.

## Decision

1. **Each creation is its own Space** (a membership-controlled group with an encrypted event stream). Loose short notes live in a default Notes Space.
2. **Conversations are separate Spaces.** A chat refers to a creation with a `CreationRef`: creation Space id, document id, an exact version hash, and an optional text range.
3. **Access is a ladder.** Each rung is a scoped capability grant (UCAN-style), optionally with an expiry.

| Rung | What the other person gets | Mechanism | Revocable? |
|:--|:--|:--|:--|
| **Snapshot** | An immutable copy of one version, sent into a chat. They can read it and discuss it. | Encrypted attachment in the conversation Space | No: it's a copy |
| **Live read** | Read access that follows updates. | Reader member of the creation Space | Future only: removal rotates the group key, but what they already saw stays seen |
| **Comment** | Live read, plus comments anchored to ranges. | Reader member allowed to post comment events | Future only |
| **Write** | Edit the creation alongside you. | Writer member; edits merge through the document CRDT | Future only |
| **Admin** | Manage members and roles. | Admin member | Future only |
| **Public** | Anyone can read. | Open-read Space with no member list; "follow" is a subscription | Unpublish stops future reads |

4. **Upgrading keeps the conversation.** A `CreationRef` that points at a snapshot can later be "upgraded": the person is added to the creation Space and the same conversation now shows the live creation.
5. **History for new members is a choice:** "from now on" or "with history" (a checkpoint sent to the new member). A new MLS member cannot read earlier epochs by default.
6. **The creation-to-conversation sidebar is a local index** on each device. The relay never learns which chats are about which creation.

## Consequences

- Read-only is enforced by the signed member and role log, not by cryptography. Every member holds the keys, and every client ignores events from members without the right role. A reader can still leak what they can read. This is the same limit as any shared document, and the UI should say so.
- The capability model (v0.4) is the foundation of the social layer, not a separate feature.
- Social features map onto the same ladder: follow is a subscription to a public Space; a subscriber list is reader membership; a community is a Space with roles; later paid or gated reading is a token-gated read grant.
- Multi-writer editing needs concurrent-merge support in the document layer and a collaborative editor binding. It is a larger piece than reading and commenting.

## Staging (proposed)

- **v0.3:** single author.
- **v0.4:** snapshot, live read, and comment; the capability API.
- **Later milestone:** write access. Asynchronous co-writing (edit separately, merge) first, live presence and cursors afterward. Schedule to be decided.

## Open questions

- Default for new members: "from now on" or "with history"?
- Do comments live in the creation Space, in the conversation Space, or both?
