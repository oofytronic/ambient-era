# Architecture

The map of the system as it is now. Decisions live in [docs/adr/](docs/adr/). Wire formats live in [docs/spec/](docs/spec/). Promises live in [THREAT_MODEL.md](THREAT_MODEL.md). If this file disagrees with an ADR or a spec, the ADR or the spec wins.

## v0.1 fence

v0.1 is a desktop daily driver for one person:

- An identity log, unlocked locally. A passkey is not the root.
- One Notes space containing that person's devices.
- One conversation space per contact, sealed to per-contact keys.
- A relay that stores opaque blobs addressed by random mailbox ids.
- Local search over the decrypted notes.

v0.1 does not include groups beyond a 1:1, public Spaces, phone push, Tor, payments, a CRDT library, or a running agent. Do not scaffold those "so they exist." Reserved names in the specs are rejected on purpose.

## One primitive

A Space is the only membership model: a membership list plus the merge of its members' signed logs, sealed with HPKE to current member keys when the Space is private.

Anything new is one of these, and not a third mechanism:

- A Space of this same kind (a notebook is the owner's devices, a 1:1 is two members, a later group is N members, a later creation is its own Space).
- A capability grant on a Space (narrowed, expiring, revocable access). That grant is v0.4, not a new protocol invented beside Spaces.
- A copy. Putting note text into a chat copies the Markdown into a `msg.put`. The contact does not join the Notes space. The citation grants no key and no later edits.

Do not add a per-share Space type, a second access list, or a "deleted everywhere" promise. Revocation stops future seals. It does not pull back plaintext a member already opened. See [THREAT_MODEL.md](THREAT_MODEL.md).

## Adding a feature

Read this before adding a screen, a type, or a table. The note editor is a view: it styles Markdown while you type, and the bytes that sync are still the `text-snapshot-v1` snapshot.

A new feature is one of three pieces.

| Piece | What it is | What it is not |
|:--|:--|:--|
| **Room** | A Space. The guest list and the signed logs. The notebook's guests are your devices. A conversation's guests are you and one contact. A later site is its own room because strangers are not guests of the notebook. | A content type. Notes and chat are the same machine. |
| **Record** | One signed event. Today: `note.put` (the whole Markdown document; both versions stay when they diverge) and `msg.put` (a short text, kept, never merged). | A row in a side database. If it has to reach another device, it is a record. |
| **View** | A picture this device draws from records it already has. Search, a task list, a rendered page, the styling in the editor. | A permission boundary, and not synced. Two views may draw the same records. |

Cursor, scroll, and which screen is open stay on the device. They are not records.

Ask two questions, in order.

1. **Is the guest list the same as a room that already exists?** The same people as the notebook: it stays in the notebook. A different set (a co-author, a leased reader, the public): its own room, the same Space code, not a new protocol. Public rooms are a later spec. Do not fake one by uploading plaintext.
2. **If two devices change it while apart, what is kept?** The whole text, and both copies when they diverge: a document (`note.put`). Do not invent a type. Small facts that should all apply, such as a chat line, or a checkbox flip that must not collide with editing a paragraph: a small record whose merge is "keep every event," and only then. Character-by-character merge: a new codec and a new type string, specified first. Not a silent change to the current document bytes.

Pointing at another thing is a citation (an id and a version, no key), a copy (the bytes, and then the two diverge), or a grant (membership, or a narrowed expiring capability). Not a relation in an object graph.

Fifteen names are not fifteen types. A person page, a meeting, a chapter, and a website draft are documents. A label inside the Markdown, plus a view that queries those documents, is the difference. A new record is justified only when the merge rule is different. A new room is justified only when the guest list is different. A new view is justified when the screen should look different. Most features are the third.

A plugin, when one exists, is a view with a grant: which rooms, which records, read or append, expiring, and never widened. It does not hold the root, it does not seal, and it does not keep its own guest list. That grant is [ADR-0001](docs/adr/0001-creations-as-spaces.md) and `ambient-capabilities`. It is not a second app beside the log.

Two shapes, so the questions stay concrete:

- **Tasks.** Rows that must survive a bad merge are records in the notebook: a stable id, the text, and a done bit, each change its own event. "Today" is a view. Not a second file the other devices never see.
- **A website.** Each page is a document. The site is its own room, because its readers are not the notebook's guests. Pages share that room the way chapters share a book. Publishing copies the text, and the draft and the page then diverge. The theme and the navigation are a view. Unpublish stops future reads. It does not delete the draft, and it does not erase a copy someone already opened.

Do not add an object graph of typed things with relations. That is a second model. One document shape plus views is how a notebook grows. The guest list is the part this app adds, because who can read the log is the product.

## Larger project, not now

| Later | What it is | Why it waits |
|:--|:--|:--|
| Small private groups | The same Space and the same HPKE fanout | The cap and the UI wait until 1:1 is in use |
| Long-form creations | A Space per book or newsletter, with versions and links | Spec 05 is not written |
| Weave | A citation from a chat to a creation, already just a copied message in v0.1 | The sidebar is a projection, not a new protocol |
| Public identity and feed | Signed, unsealed logs. A second persona. | Must not be built by relaxing the private-Space rules |
| Phone clients | SwiftUI and Compose over the same core. Content-free push. | Desktop first |
| Agent | A delegated key plus a grant. Every member must agree before it reads a shared Space | No agent runtime in the core |

These are the structural problems that decide whether a later version is still user-owned. They are not v0.1 work. Do not scaffold them. Each one has to be a Space, a grant on a Space, or a copy. None of them is a second authority beside the owner.

| Problem | The constraint | What it is not |
|:--|:--|:--|
| Recovery for people who will lose a phrase | A threshold of guardians or devices can authorize a key rotation after a timelock. No single party can seize the root. The v0.1 phrase remains the backup until that exists. | A custodian. A passkey that is the root. |
| Someone has to keep nodes on | People who need a relay, a host, or compute pay the operator who provides it. Reputation is uptime and quality. Most people stay clients. | A token that is the product. An obligation to run a home server. |
| Agents that can publish or spend | An agent starts with nothing. It receives a short-lived scoped grant. The grant expires or is revoked. Enforcement is on a device or node the owner controls. Consequential actions are in a local log the owner can read. | Standing power. A model that is trusted because it said it would ask. |
| Sensing | Inference and temporary context stay on the owner's devices. Raw sensor data leaves only when chosen for one task. Sensing has a visible indicator. Agents ask for the narrowest context. | A permanent life log. A wearable that uploads by default. |
| Nodes that sleep | Public content can be mirrored as signed snapshots the mirror cannot alter. Relays are several and interchangeable. An agent that acts while the owner is offline already holds a grant, or it escalates to a person or another device. Intermittent nodes are the normal case. | A home server that must stay up for a public page to exist. |
| Finding anyone without a center | Signed announcements, local discovery, and an out-of-band introduction (the v0.1 link or QR). An operator may publish a bootstrap set. The protocol still works if that set is gone. | A directory that is the only way in. |
| Abuse without a central delete | Clients filter. People and communities share blocklists. A node refuses content under its own policy. The operator's nodes are legally separate from nodes other people run. | A moderator who can erase someone else's log. |

## Pieces

```
desktop client (Iced on Linux)
        │ commands / queries
        ▼
ambient-core          facade. No UI types.
  identity, spaces, sync, store, docs, transport, capabilities
        │ sealed blobs
        ▼
ambient-relay         mailbox store. Cannot decrypt.
```

The relay crate depends only on transport. It must not depend on the store or the core.

Layering is in [AGENTS.md](AGENTS.md). Dependencies point down.

## What the code does today

The CLI and the Linux window append notes into a sealed file, pair a second folder that does not hold the root, and sync those notes through `ambient-relay`. The default is `127.0.0.1`. The folder can pin a `wss` relay instead. They can open one conversation and exchange text the same way. The window is a notes, tasks, and chat shell. Tasks are a view over checkbox lines in notes: checking one writes the line back. It is not a second file. A task added on the Tasks screen is a line in a note titled Tasks, created if it is missing. That is the same view, not a new record. Deleting a note is a later `note.put` whose body is only `%%ambient-deleted%%`. The list hides it when every head is that snapshot. The older text stays in the log. It is not a new event type. A `#tag` in the body is how the list groups notes. That is a view, not a folder and not a new event. A file added in chat is a local copy cited from the message text. The citation can sync. The bytes do not, and that is not a new record. A globe in the toolbar shows whether the loopback relay is off, caught up, or holding unsent events, and the window syncs on its own while that relay is on. An invite is part of the conversation. Devices and recovery are sections of settings, with the local name, the color, and erasing the folder. A `profile` file and a `names` file in the folder are local and not records. The contact card still has no name. A device label is set when the folder is created. It calls the client in `ambient-cli` and does not seal events itself. There is a recovery seal, a contact invite as a link or a QR, a revoke command, and a command that copies a note into a chat. A phrase restore enrolls the new device; it does not recreate old device keys. Message events carry the advisory unix time from the event envelope. The key file can be wrapped with a passphrase; that passphrase is not a platform passkey. Search uses a SQLCipher FTS5 index next to the sealed log. The log is still the source of truth, and `ambient-store` is still not SQLCipher. Two folders on one computer is not a second machine. The loopback relay keeps its mailboxes in a file and is still cleartext, so it is not a public deployment. The window keeps a list beside the page when it is wide, and a bottom bar when it is narrow.

## Default network

Not built. The track is [docs/network/README.md](docs/network/README.md). This paragraph is the shape, not the steps.

A person never points the app at a server. The app ships with a small list of relays, tries them, and remembers the one that answered. That memory is in the notebook folder. It is not an account. The relay still stores opaque blobs and still cannot read them. If every relay is down, notes keep working here. Advanced settings are the only place an address is typed. A self-hosted relay is that screen, not the first one.

## What happens to a note

This is the path the spec requires. The CLI and the window do steps 1, 3, and 4. They write the recovery copy below. The window can show the phrase and restore from it. The sealed file is still the log. The native client also projects that log into a SQLCipher FTS5 index. `ambient-store` itself is not SQLCipher.

1. The device appends a `note.put` event, signed by that device's key, linked to that device's previous event.
2. The local database projects it, and FTS indexes the plaintext. The database file is encrypted. The index never goes to the relay.
3. The wire bytes are sealed with HPKE to each other device in the Notes space and put in that device's inbox mailbox.
4. The other device opens the seal, checks the signature and the device chain, and projects the same way.

The machine that holds the root also writes a recovery copy, sealed to a key derived from the root, into a mailbox derived from the root. Pairing does not copy the root. A mnemonic rebuilds the identity and the notes on a new install. It does not recreate device keys. The owner then revokes keys they no longer have.

Two devices can both write a note while offline. `text-snapshot-v1` keeps both versions when neither is a parent of the other. It is not a CRDT. A CRDT codec is a later choice and a new type string.

A conversation is the same path with `msg.put`, signed by a per-contact key rather than the Notes device key. Copying a note into a chat copies the Markdown into a message. The contact is not added to the Notes space.

## What is deliberately absent

- MLS, or any totally ordered group commit. Merge has to work in any order.
- A global user id on the relay.
- Forward secrecy against theft of a device's long-term key.
- An embedded wallet.
- JavaScript in the product.

## Where to change something

| Change | Touch |
|:--|:--|
| A byte on the wire | The spec, then a vector, then code. Not the reverse. |
| Who may be in a Space | `docs/spec/03-spaces.md` and ADR-0001 if the access ladder changes |
| How sealing works | A new ADR. ADR-0005 is the current one. |
| A new crate or dependency | ADR-0003's policy, `deny.toml`, the crate's `AGENTS.md` |
| A new kind of thing | This file, "Adding a feature." A view needs no spec. A record or a room does. |
| A product promise | `THREAT_MODEL.md` first. Then the README, if the claim is public. |
| How a second computer syncs | [docs/network/README.md](docs/network/README.md). Spec 04 if a byte changes. |
