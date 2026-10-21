# ADR-0001: Creations are Spaces, shared along an access ladder

**Status:** Accepted (model and leased shares); staging proposed.

## Context

Creations (books, series, newsletters, note collections) need to be written privately, discussed with others, shared at different levels of trust, co-written, and eventually published. Conversations about a creation must stay attached to it. We want one mechanism for all of this, and we want the later social features to reuse it.

## Decision

1. **Each creation is its own Space** (a membership-controlled group with an encrypted event stream). Loose short notes live in a default Notes Space.
2. **Conversations are separate Spaces.** A chat refers to a creation with a `CreationRef`: creation Space id, document id, an exact version hash, and an optional text range.
3. **Access is a ladder.** Each rung is a scoped capability grant (UCAN-style), optionally with an expiry.

| Rung | What the other person gets | Mechanism | Revocable? |
|:--|:--|:--|:--|
| **Share** (leased snapshot) | A frozen version of one creation, available to them for a limited time. They can read it and discuss it. | A temporary **share Space** owned by the creation's owner: one frozen version, recipients as view-only members, an expiry in the grant | Yes, for honest clients: revoking or expiry removes the members, deletes the blobs, and clients purge cached content. A hostile client can still keep what it saw. |
| **Live read** | Read access that follows updates. | Reader member of the creation Space | Future only: removal rotates the group key, but what they already saw stays seen |
| **Comment** | Live read, plus comments anchored to ranges. | Reader member allowed to post comment events | Future only |
| **Write** | Edit the creation alongside you. | Writer member; edits merge through the document CRDT | Future only |
| **Admin** | Manage members and roles. | Admin member | Future only |
| **Public** | Anyone can read. | Open-read Space with no member list; "follow" is a subscription | Unpublish stops future reads |

4. **Upgrading keeps the conversation.** A `CreationRef` that points at a share can later be "upgraded": the person is added to the creation Space and the same conversation now shows the live creation.
5. **History for new members is a choice:** "from now on" or "with history" (a checkpoint sent to the new member). A new MLS member cannot read earlier epochs by default.
6. **The creation-to-conversation sidebar is a local index** on each device. The relay never learns which chats are about which creation.
7. **Shares are leased, not given.** Sharing a version creates a temporary share Space owned by the creation's owner. It holds one frozen version (content-addressed), carries an expiry (**default 7 days**; the owner can set any custom duration, or choose "until I revoke it or delete the Space"), and its recipients are view-only members. When the owner revokes, a recipient is removed, or the lease expires: the share Space closes, its blobs are deleted from relays (a retention policy that never reads content), honest clients purge cached plaintext, and the chat keeps a **placeholder** ("Shared "Chapter 3", no longer available") that is no longer linked. The conversation text itself stays, since chat messages belong to the participants.
8. **Comments outlive the lease; the creation does not.** Comments and other activity associated with a creation or share are kept when access ends. The shared content is no longer accessible. Comments store only their anchor (version hash and range), never a copy of the text they point at, so after expiry an anchored comment shows with a "text no longer available" placeholder instead of the passage.

## Consequences

- Revocation of shares and removed readers is best-effort. It relies on honest clients purging and relays deleting. Nothing stops a screenshot or a modified client, so the UI promises "stops access and asks devices to remove it", never "can't be copied". Per-share flags such as no-export are enforced by honest clients only.
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
- A removed reader's cached copy is purged. For removed writers, proposed that their own authored contributions stay in the history.
- Quoted excerpts in chat are chat content, not part of the lease. Proposed: allowed, never auto-quoted.
