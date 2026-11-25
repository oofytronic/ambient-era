# Creations

**Version:** 0
**Status:** draft, not normative. Nothing in the client implements this. No conformance vector exists yet. Confirm before either.

A creation is a private Space whose readers are not the notebook's guests. This version has one author: the identity that signed genesis, and the devices that identity has already enrolled. It does not have a second writer, a comment, a grant, or a public copy.

Events use the [envelope](01-events.md). Membership, notes, and sealing use [03-spaces.md](03-spaces.md), with the exceptions in this document. Devices use [02-identity-log.md](02-identity-log.md). This spec does not use MLS and does not select a CRDT.

When this draft is confirmed, two lines elsewhere change and nothing else does. The genesis kind in spec 03 gains `"creation"`. `note.put` becomes valid in a Space of that kind. Spec 01 gains no prefix.

## Kind

`space.genesis` with kind `"creation"`. The Space id is the event id of that genesis, as in spec 03.

```cddl
creation-genesis = {
  1 => "creation",
  2 => [member]          ; exactly one
}
```

`member` is the spec 03 member. The array MUST hold exactly one member. That member's signing key MUST equal the envelope signer, and the role MUST be 1. Any other length, role, or signer is `bad_payload` and MUST NOT be applied.

`"creation"` is `bad_payload` under spec 03 until this document is confirmed and that spec says otherwise. Do not send it before then.

## Who may be a member

A `space.member-add` is applied only when its signing key is an unrevoked device of the identity that signed genesis. The check is the identity log: an accepted `id.device-add` for that key, and no later `id.device-revoke` that covers it. A key that fails the check is not applied. The code is `not_device`. Later events from the genesis signer are still read.

Adding one of those devices MUST include a checkpoint of the accepted creation events, sealed to the new encryption key, as spec 03 requires for the owner's devices in Notes.

This version has no other members. A reader, a commenter, and a second writer are a later spec, not a role number slipped into this one. Role 3 and every other role stay `bad_payload`, as in spec 03. The sixteen-member cap still applies.

`space.member-remove` and `space.inbox` are unchanged. Removing a device stops future seals to that key. It does not drop the events that device already signed.

## Pieces

A piece is a `note.put`, with the bytes and the conflict rule in spec 03. Two heads are both kept. Clients MUST NOT pick a winner. There is no new document type: the merge rule is the same as a note.

`note.put` in a creation is valid. `msg.put` in a creation is `wrong_kind` and MUST NOT be applied. A `note.put` does not grant membership in any other Space.

A body whose only bytes are `%%ambient-deleted%%` hides that piece when every head is that snapshot. The older text stays in the log. This is the same deletion as a note, not a new event.

## Links

A link is text in a piece body. It is not a record. It grants no key and no membership.

Outside a fence, and not after `!`:

```text
[[ note-id ]]
[[ note-id | label ]]
[[ note-id @ version | label ]]
```

`note-id` and `version` are 64 hex digits, either case, each decoding to 32 bytes. `version` is the event id of one `note.put`. The label is the text after the first `|` and MUST NOT contain `]]` or a newline. Space around the id is ignored. An image embed (`![[`) is not a link. Text inside a fence is not a link.

A link with no version names the piece. A link with a version names that snapshot and no other. The current heads are not a version. Opening an older snapshot does not write it back.

## One version

A version of a piece is one `note.put` event id.

A version of the creation is the set of applied events. Its id is BLAKE3-256 over the applied event ids, each 32 bytes, sorted by their bytes and concatenated. The id is not an event. Arrival order does not change it.

## Export

Export writes a directory for one version, or it writes nothing.

The caller MAY name one event id per piece that has more than one head. A piece with more than one head and no name fails the export. The code is `conflict`. A named event that is not a head of that piece fails the export. The code is `not_head`. Naming a head does not delete the other head and does not append an event.

A contents piece is a note whose every non-blank line, outside fences, is exactly one link and nothing else. If the chosen head of exactly one piece is a contents piece, export follows those links in order. The same note id twice is kept once, at its first place. A link to the contents piece itself is skipped. Pieces not listed follow, ordered by note id bytes. If there is no contents piece, or more than one, every piece is ordered by note id bytes. The contents piece is not itself a chapter file.

A pinned version that is not an applied `note.put` for that note id in this Space fails the export. The code is `bad_link`. An unpinned link uses the head rule above.

A piece whose chosen head is the deletion body is omitted.

The directory holds:

- `export.txt`. The first line is the creation version id, lowercase hex. Each later line is the lowercase hex note id, a space, the lowercase hex event id, a space, and the relative file name. The lines follow export order.
- One Markdown file per exported piece. The bytes are field 4 of the named event, unchanged. No title is inserted.

A file name is the piece title with control codes, `/`, `\`, and `:` removed, trimmed, and cut to 80 UTF-8 bytes on a character boundary. An empty result uses the lowercase hex note id. The names `export` and `export.txt` are reserved. A reserved name, or a name another piece in this export already took, gains `-` and the first 8 lowercase hex digits of the note id. The suffix is `.md`.

## Not in this version

An event whose type starts with `creation.` or `comment.` is `bad_type`. Do not skip it. Reserved for a later spec, and not defined here: `creation.ref`, comment anchors, a share Space, a grant, a public log, and a second writer.

A conversation about a creation is a conversation Space. This version does not link the two. Copying text into a message is a copy, as in spec 03.

## Changelog

- v0 (2026-11-25): draft for confirmation. Single author, pieces as `note.put`, links in the body, export that refuses a conflict.
