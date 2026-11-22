# Open items

Everything that is undecided, unbuilt, unverified, or unfixed as of 2026-11-20, in one place. [ROADMAP.md](../ROADMAP.md) is the order of the work. This is the loose ends. When one is closed, delete its line and say so in [HISTORY.md](../HISTORY.md).

How it got here is [session-2026-11-19-to-20.md](session-2026-11-19-to-20.md).

## Needs the owner

These are not for an agent to settle.

| # | What | Why it waits | Where |
|:--|:--|:--|:--|
| 1 | **Review and accept the changes to `ambient-spaces`, `ambient-identity`, and `ambient-store`.** The merge now makes a note from a not-yet-enrolled signer wait. There are new `project_log_verified` and `project_notes_verified` functions, and a compare-and-write, `write_log_if`, with a lock. | The repo rules say these crates need owner review. No wire format, key, or signature rule changed. | [ADR-0006](adr/0006-wait-for-enrollment.md) (Proposed), commits `49d82e7` and `398b78c` |
| 2 | **The recovery mailbox conflicts with the relay spec.** Spec 02 derives it from the root with HKDF so a phrase alone can find the backup. Spec 04 says a mailbox id must never be derived from a key or an identity. Recovery is built and works, so the conflict is live. | It is a privacy question. A stable derived id is exactly what T1 says the relay is not given. Raised twice and not settled. | [spec 02](spec/02-identity-log.md), [spec 04](spec/04-relay-api.md), [THREAT_MODEL.md](../THREAT_MODEL.md) |
| 3 | **Licensing.** Undecided. The repo now bundles an English dictionary under the SCOWL and Hunspell notice, which is permissive and has to travel with the list, and a font under the OFL. | Needed before the first user who is not the owner. | [ADR-0004](adr/0004-licensing.md), `crates/ambient-desktop/dict/SCOWL-LICENSE.txt` |
| 4 | **The dependency bumps.** Dependabot branches for `ed25519-dalek` 3.0.0, `chacha20poly1305` 0.11, `hkdf` 0.13, `sha2` 0.11, `getrandom` 0.4, and one Actions bump. | They are crypto crates, two of them major versions, and they were cut on 2026-11-04 from the old `trunk`. Nobody has built or reviewed them, and they would have to be redone against `test`. | `origin/dependabot/*` |
| 5 | **Two editing rules changed and may not be what you want.** Enter on an empty list item now ends the list. It used to add another empty item. Tab does nothing in a paragraph. | Taste. Both are one line to reverse. | `continue_enter` and `binding()` in the desktop crate |
| 6 | **Backups are by hand.** Back up in settings copies the folder. Nothing reminds the owner, runs on a schedule, or says a copy is old. The place is a typed path, because a folder picker would need a new dependency. Restoring is copying the folder back. | A reminder or a schedule is a product choice, and a picker is a dependency choice. A daily driver should not lose a week of notes to one failure, and the first copy still has to be made on purpose. | [ROADMAP.md](../ROADMAP.md) v0.1 step 5, `backup_folder` in the desktop crate |
| 7 | **Whether dev builds should optimize the crates on the typing path.** | It makes the window fast and it makes those crates harder to step through. | `Cargo.toml`, [ADR-0003](adr/0003-workspace-and-tooling.md) |

## Asked for, not built

- **Spotlight the paragraph being written** in focus mode. Focus is the place for it. Nothing else in the page changes.
- **Focus mode for a narrow window.** It is only for the wide layout today. There is no key to leave it other than the icon and Ctrl or Cmd + a full stop.

## Asked for, not built (2026-11-20, second list)

- **Transitions between tabs, and on hover.** Only focus mode eases. Iced has no animation layer: a tab change would need the old view kept to fade out, and a hover color would need a widget that holds its own state. Both are a small widget each.
- **The tag field with nothing typed** offers nothing. There is no focus event on a text field to show the list on. Typing one letter shows it.
- **A scroll speed setting.** The wheel follows a page step now. Nothing reads a system wheel setting, because Linux has none to read: the system sends the notches already scaled or it does not.
- **Settings in the note's panel** show only the home device, send, and delete. Tags and the note's id are not there yet.

## Links, groups, and the graph, not built

- **A link field beside the tags.** The owner liked the idea of adding a link to a note from the top, as a chip with a picker like the tag field. Links are still written as `[[ ]]` in the body. The field would store them on the hidden line like tags, and the panel would read both.
- **Group order does not sync.** Notes in a group are alphabetical, and can be moved by hand with a grip. The hand order is a plain local file, `order`, so a second computer would show alphabetical order. Syncing it needs a place in the log, which is a record with its own merge rule, so it needs an ADR. A group also has no text of its own. A note in two groups is ordered separately in each.
- **Drag is by the grip only,** and only inside one group. There is no moving a note to another group by dragging, no keyboard way to reorder, and no way to clear a hand order except moving notes back.
- **The graph.** It lays out once and does not animate, pan, or zoom. Past thirty notes the titles are shown only on hover or for the open note. A click opens a note. Only the top of a group path is a circle, so `Novel/Part 1` is not a circle inside `Novel`. There is no filter, and no way to show only one group.
- **Fold state** of groups is not kept between runs.
- **Reading a group** is in title order. There is no hand-set order, and no per-group word count.

## Platforms, first step done

See [platforms.md](platforms.md).

- **`MacHost` has never been run on a Mac.** It is written from documentation and compiles. Its path rule and its dark-mode reading are tested as plain functions. The commands (`open`, `defaults`, `osascript`) are not.
- **`/dev/urandom`** in `files.rs` should be `getrandom`.
- **The `Host` trait lives in `ambient-cli`.** It should move to `ambient-core` when that facade exists, and a native shell should implement it through UniFFI.
- **The shared design system** (the same colors, type sizes, spacing, and icon sizes for every UI) is undecided and not started.

## Tasks and dates, not built

- **Typing a date by hand** is not possible in the window now. The calendar is the only way. A date or time written in a note by hand still counts if it is a real one in the one form.
- **The calendar's months and weekdays are in English.** The week can begin on Sunday or Monday, and times on a 12 or 24 hour clock, in settings.
- **A trip is only noticed and said.** Times stay the clock where the person is. There is no way to keep a task on home time, or to give a task its own zone, so a meeting at a fixed place can be an hour or more off after a flight. The hours at home come from the home zone's file, so a computer that cannot read it says only that home is somewhere else.
- **Home is the first named zone and is changed by hand.** A person who moves house has to say so, and one who first opened the folder while travelling has the wrong home until they do.
- **A time of day has no zone.** It is the wall clock where the person is, so a task due at 09:00 is due at 09:00 after flying across the world. A task shared in a note would be due at that wall time for whoever reads it. That is simple and it can be wrong for a meeting at a fixed place.
- **The window looks at the clock every 20 seconds, and only on the Tasks tab,** and when it is brought back. A task can read as not yet late for up to 20 seconds after its minute. Elsewhere nothing shows a due time.
- **The time zone comes from `/etc/localtime` only.** A `TZ` environment variable is not read, and a system that has no such file is treated as UTC and says so in the panel.
- **A repeat that is checked is moved, not recorded.** There is no history of when it was done, and no way to undo but to edit the date. Done early or late, the next one is the first after both the due day and today.
- **A monthly repeat on the 29th to 31st is clamped and then stays clamped:** January 31 goes to February 28, then March 28.
- **No reminders.** Waiting on notifications, which do not exist yet.
- **The task panel's counts and the groups** are rebuilt on every draw. That is fine at a few hundred tasks and has not been measured past that.

## Narrow windows and popups, not done

- **Nothing here was seen.** The layout was reasoned from the code and checked only by drawing every screen at nine widths with every popup open. An overflow, a card too tall for a short window, or a menu in the wrong place will not show in a test. Look at 1019, 760, 600, and 420 pixels first.
- **`[[` suggestions in a note** still sit in the flow and move the page while typing. They could float at the caret.
- **Panels do not slide.** The panel on the right comes in as a sheet at once, and the focus mode's slide is the only animation.
- **A card taller than a short window** is cut, not scrolled, except the task views and the chat chooser, which scroll.
- **Settings, the invite page, and the chat list** were looked at only as far as the widths above. They have not been combed for spacing.

## Offered, not built

Small things that would help daily use.

- Open the last note again when the app starts.
- A list of recently deleted notes, with a way to bring one back. A delete is a snapshot whose body is a marker, so the text is still in the log.
- Remember the window's size and place.
- A way to see and remove the words in the personal dictionary. There is only an add.
- Spelling for other languages, and for words with an accent. A word with a non-ASCII letter is never checked.

## Editing gaps found while testing

None is a regression. They were found, and left.

- Bold or italic inside a code span adds literal marks. It should do nothing.
- Bolding a URL splits it at the colon, `**https**://example.com`.
- Pressing Enter in the middle of a list item leaves a space at the start of the new item.
- Nesting an ordered item does not renumber the list.
- Backspace at the start of a list item's text does not take the item out a level.
- There is no key for a task's checkbox, and no auto-closing of marks.
- A capitalized word the dictionary does not know is taken for a name unless it starts a sentence. A capitalized typo in the middle of a sentence is not underlined.

## Performance headroom

Measured on 2026-11-19 and 2026-11-20. See the numbers in the session record.

- The page is still drawn in full on every update. A key plus its view is 3 ms at 13 KB, 12 ms at 51 KB, and 46 ms at 194 KB. A cache of the built lines, or drawing only what is on screen, would make it flat.
- A save still rewrites the whole sealed log and rebuilds the projection from the verified events. It is 30 ms at a thousand notes. An append-only layout needs a new store format and a nonce scheme, and so an ADR.
- The search index is rebuilt in full after any change, 65 ms at a thousand notes. Updating only the notes that changed would remove that.
- A first load verifies every signature, 80 µs each. That is once per run.

## Merge and protocol

- **A removal and a note from the removed member** still depend on event-id order when neither refers to the other. A device revocation has `up_to` for this. A member removal has no such marker. See [ADR-0006](adr/0006-wait-for-enrollment.md).
- **There is no vector format for a projection.** The merge rule has tests and no conformance vector, so a second implementation could not check itself against it.
- **Spec 05** (long-form creations) is intentionally unwritten.
- The relay is cleartext on `127.0.0.1`. TLS to a relay is built. The introduction list is empty, and the replacement test, step 7 of [the network track](network/README.md), has not been run. No public relay exists.

## Not verified

- **Nothing in the window was seen.** That includes everything from the 2026-11-20 second list: the settings panel and its gear, the hover delete, the Tasks card, Send in the pill, the tag offers, the scrollbar room, the wheel step, and the panel slide. While a panel slides it is squeezed, not moved, and may look like it rewraps. Look at these first. Also: There was no display. Covered by tests of state and of pure functions, not by eyes: when the scrollbar appears and for how long, the focus slides, where the delete card sits, the pencil, how the revealed marks look, and Tab on a selection. Look at these first.
- The slow scrolling was reported once and was fixed by removing a message per scroll tick. It was reported fine afterward, with other changes in the same build. It has not been confirmed on its own.
- `just deny` was not run. `just`, `nextest`, and `cargo-deny` were not installed where this was done. No dependency was added. The equivalent `cargo` commands passed.
- GitHub Actions was not looked at.
- A Mac or a Windows build. The window is Linux only.

## Still scaffolds

`ambient-core`, `ambient-sync`, `ambient-capabilities`, `ambient-docs`, and `ambient-ffi` have no behavior. The v0.1 exit in [ROADMAP.md](../ROADMAP.md) is the owner using it daily on one computer. It was declared on 2026-11-20 and that use has started. The first v0.2 exit is not met: the owner on a second computer for real notes, one 1:1 with one other person, an offline gap, and one restore from the phrase. A green test run is not that.
