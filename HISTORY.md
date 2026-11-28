# History

A log of meaningful decisions. Newest first. This is not a commit changelog.

When a turn changes direction, scope, or a locked design choice, add an entry here in the same change. Link the ADR or spec that records it. Do not relitigate an entry in a later one; supersede it.

## 2026-11-30 — habits, phase one

Habits are in the Tasks tab. A habit is a line in a note called Habits, so it is text and a note like any other. It can be every day, some weekdays, or some days a week, and it can ask for a count a day, like eight glasses. Today shows the habits that are asked for, each with a round button. A Habits view shows the week as seven dots, how the last seven days went, and a month to look back over and fill in. Check-ins are kept on this computer only. How they should sync is not decided, and nothing was built that would make it harder to decide. There is no visual day timeline and no reminders yet.

## 2026-11-29 — a task line reads better in its note, and a long subtask no longer breaks

A task's priority and day are drawn in color in the note, and a subtask is moved in under its task. The saved text is unchanged. A long indented line with the caret in it used to put its checkbox alone on one row and its words on the next, because the wrapper made a row of the leading space. That is fixed and held by a test. A count of done subtasks drawn at the end of the parent's line is not done yet.

## 2026-11-28 — tasks: Today, plain-language adding, subtasks, priority, snooze

The Tasks tab opens on Today: what is late, what is due today, and what was checked off today. Typing `call vet tomorrow 3pm every week !2` in the add field reads the day, time, repeat, and priority out of the words, shows what it read, and can be turned off for that one task. A task can have subtasks, which are tasks indented under it in its note. A task has a priority, `!1` to `!3`, written as a token in its words. A task can be put off to later today, tomorrow, next week, or a picked day. All of it is words in the task's own line. Nothing is a new record, and nothing changes on the wire. Habits are not started.

## 2026-11-27 — a link on a note, rename and merge a group, a deeper graph, and a choice of facts

A note can hold one link, kept in a hidden line beside its tags. The line is in the sealed body, so nothing on the wire changes. Renaming a group, or typing the name of another group to merge into it, rewrites the tag line of every note in it, each as a new snapshot. The old text stays in the log. The graph shows the second part of a tag as a circle inside its group, can be limited to one group, and pans and zooms. The note panel lists more facts (link, when it changed, words), and which ones show is a choice kept on this computer. A choice that was "per note" is the panel's, for every note, because a per-note one would be saved data.

## 2026-11-25 — a palette, quick capture, templates, and pins, all over what was already there

`Ctrl+P` opens a palette: jump to a note, switch tab, run an action. `Ctrl+J` opens a small box that writes a note or a task without leaving the page. A new note can start from a template. A template is a note tagged `Templates`, and the daily note is the first row of the picker and is never made unless chosen. A note can be pinned to the top of the list. The pin is a local file, like the order of a group. No record, no room, no wire change. These keys work inside the window only. A system-wide hotkey is not claimed.

## 2026-11-23 — the light follows the line, and the chat log stays still

The spotlight was the open word's paragraph, so it stayed on the first block. It is now the line the caret is on. A wrapped line stays together. The next line does not.

The chat log stays anchored at the latest line. Scrolling up still offers Latest. Scrolling on at the bottom no longer flips the anchor, which was what shook the thread. Enter on the task the arrows are on checks it, or clears the check.

## 2026-11-23 — a tighter notes, tasks, and chat

Focus has a light on the far right. It leaves the paragraph under the caret as it is and draws the rest quieter. The list remembers which groups are folded, and which note and line were open. A search hit and a task open on that line. Insert on the format bar stays closed until it is asked for. The note's panel shows its tags and its id.

The Tasks tab counts what is overdue. Done is grouped by the day it was checked, on this computer. A repeat that moves says when it comes back if it has left the list. Arrow keys move through the tasks, and Alt with an arrow moves one up or down in its note.

A chat shows a time when the hour changes, the person changes, or it is the latest line. Scrolling up and receiving a line offers Latest, and does not jump. An unsent line stays with its conversation. The people under a group's name open the same names, remove, and block as the pencil.

## 2026-11-23 — how to stand a relay up, written and not started

[docs/network/standing-up.md](docs/network/standing-up.md) is the path for a Raspberry Pi or any other machine: a name, a Let's Encrypt certificate, and the relay on port 8443. A homemade certificate will not do. The owner is not doing this yet. The window's relay stays this computer.

## 2026-11-23 — remove and block from the window

The pencil on a conversation can remove a person or block them. Only the window that started it can remove, and they keep what was already sealed to them. Block is this computer only: an invite from here will not include them, and conversations they are already in stay. The commands are still there.

## 2026-11-23 — the card is a pass

The card in Settings, and on the guide before the notes, is a pass: a dark face, the name large, the initial as a seal, and the key along the bottom. It does not follow the light or dark theme. The fields that change it sit underneath.

## 2026-11-23 — the card is a name you offer

The first thing in Settings is the card: a name, one optional line, the initial, the start of the key, and Copy link. The name is signed by the contact key and rides on the invite link. The other computer stores it only when it has not already named that key. Onboarding asks for the name before the notebook opens, and shows the card before the notes. The computer's name stays under Devices.

## 2026-11-23 — a group shows its people, and each name stays here

A conversation of more than two people keeps the name it already had, and under that name, and in the list, it says how many people and what this computer calls them. The pencil in a group edits those names, and the conversation's own name, separately. Adding someone can name them at the same time. A message that is not yours shows that name and an initial. The color comes from their key. None of this is sent. A name this computer has not typed yet is the start of the key.

## 2026-11-23 — a third person, from the window that is already open

The extra window in Settings was one other person, and a group could only be changed from the command line. The same path now opens one more person, a new folder beside the notebook, and the conversation that this window started has Add someone. Paste that person's invite link. They wait. They do not press Start conversation. The next sync seals the conversation to them. The list names a conversation of more than two people by the count. Remove and block stay commands.

## 2026-11-23 — the second computer is the end of v0.2

The owner cannot use a second computer yet. On 2026-11-20 the second computer was the first exit of v0.2, and groups waited behind it. That order is reversed. v0.2 now builds what one computer can test, and the second computer is the last check.

A private Space holds at most 16 member keys. A 17th `space.member-add` is not applied. Genesis of more than 16 members is `bad_payload`. [Spec 03](docs/spec/03-spaces.md). The person who started a conversation can add a member and remove one. A signing key in the local file `blocked` is not invited. That file does not sync. The relay refuses a mailbox written more than 10000 times in a minute, counted in memory and not logged ([spec 04](docs/spec/04-relay-api.md)). That is the rate limit on a flood, including a first contact. It is not a user id.

A file cited from a chat line is sealed to the other members and written beside their copy of the conversation. It is not a new record. A file that does not fit in one relay blob is sent in pieces. The window still has no button for add, remove, or block. The commands are `ambient add-member`, `ambient remove-member`, and `ambient block`.

The second computer, the 1:1 with another person, the offline gap, the restore, and the relay replacement test stay the exit of v0.2. Content-free push stays with them, because it needs a phone. None of that was run.

## 2026-11-20 — Back up this folder

v0.1 had no copy of the notes off the computer, and the owner was about to use it every day. Back up, in settings, copies the notebook folder to a place the owner types, with a default of `Aerodash backups` in the home folder. It is by hand. The copy is named for the time, `aerodash-2026-11-20-1405`, and never overwrites an earlier one.

The copy holds the key file, because the notes cannot be opened without it. That is the choice with a cost: whoever has the copy can read the notes unless the key file is wrapped. The card says so, and warns in red when the key file is not locked with a passphrase. The alternative was to leave the key out and restore from the recovery phrase. That needs a restore flow that does not exist yet, and so it was not chosen. It remains an option.

It leaves out the `export` folder, which holds readable Markdown with no lock and would have defeated a passphrase. It leaves out the search index, which is rebuilt, and lock and unfinished-write files. It does not follow symbolic links. It is made in a folder named `.partial` and renamed only when whole, so a cut-short copy never looks finished. The key file and the notes log are compared with the originals byte for byte, and flushed to disk. The copy folder is private to its owner. A place inside the notebook is refused before anything is made. The notes and chat locks are held while it runs, so no save or sync lands in the middle. A note typed in the last moment is saved first.

A copy opens as a notebook, and a test checks it. Restoring is putting the copy where the notebook is. There is no folder picker, because one would need a new dependency, and no reminder or schedule. Those are in [docs/open-items.md](docs/open-items.md).

Found on the way: Export notes writes readable Markdown beside the sealed log, and the threat model now says so.

## 2026-11-20 — v0.1 is notes on one computer, and the second computer is v0.2

The owner declared v0.1 and moved the exit. v0.1 is notes on one computer, and its exit is the owner using it daily. The second computer, the relay, 1:1 chat, device revoke, and recovery restore move to v0.2, with the exit test they had: the notes on a second computer for real work, one 1:1, an offline gap, and a restore from the phrase. The old v0.2, small private groups with encrypted attachments and content-free push, is folded into v0.2 as a second stage that does not start until the 1:1 is in use. Every later version keeps its number.

All of that code is already in the app and tested, on two folders on one computer. Moving the exit does not remove it. It changes what v0.1 promises. v0.1 promises notes on one disk, sealed, with no network. It does not promise a second device or any traffic. [THREAT_MODEL.md](THREAT_MODEL.md) now says T0 and T1 are v0.2 promises, and says plainly that v0.1 has no copy of the notes off the computer. The recovery phrase brings back the identity, and brings back the notes only from a relay copy that can be reached. [ROADMAP.md](ROADMAP.md), the README, [ARCHITECTURE.md](ARCHITECTURE.md), the network track, the spec index, and a dated note on [ADR-0005](docs/adr/0005-private-space-encryption.md) say the same.

The workspace version is 0.1.0. No tag was made. `trunk` was fast-forwarded to `test` on the same day, and work on v0.2 continues on `test`.

## 2026-11-20 — the log of the last two days, in one place

[docs/session-2026-11-19-to-20.md](docs/session-2026-11-19-to-20.md) is the story of 2026-11-19 and 2026-11-20 in order, with every commit, what was measured, and the mistakes. [docs/open-items.md](docs/open-items.md) is what is still open. [ADR-0006](docs/adr/0006-wait-for-enrollment.md) records the merge-order decision and is Proposed, waiting for owner review. Entries below are in the order they were written, newest first.

## 2026-11-20 — erasing a notebook erases its dictionary

"Erase this notebook" removed the files, but the personal dictionary's words stayed in memory, and the next word added would have written all of them into the new notebook's `dictionary` file. The words came from notes that were erased. They are cleared now, along with focus mode, and a test checks it. The small local files beside the log (`names`, `profile`, `seen`, `dictionary`) are plain text, and [THREAT_MODEL.md](THREAT_MODEL.md) now says so.

## 2026-11-20 — fixes from using the app

The Tasks tab did not show a task that was added, removed, checked, or cleared until the tab was left and opened again. A change to a note that is not open is written in the background and comes back as `Applied`, and `finish_applied` installed the new notebook without rebuilding the task list. `finish_saved` did, in the same case. It rebuilds it now.

A conversation's name is plain text with a pencil beside it, and the pencil turns it into a field. The delete question appears in the lower right over the editor, where the delete button was, and not at the bottom of the left column.

These came with a test harness for the window: `App::at(dir)` builds a real `App` on a folder, and a `cfg(test)` recorder in `apply_note` lets a test do the background writes the app asked for and hand each result back through `update`. See [docs/TESTING.md](docs/TESTING.md).

## 2026-11-19 — the search index builds in one transaction

Rebuilding the SQLCipher index deleted every row and then inserted each note in its own auto-committed statement, which is one flush to disk per note. The index is rebuilt on the first search after any note changes, so the first keystroke in search after a save paid for all of it, 1.7 s at a thousand notes. The rebuild is one transaction now, rolled back if an insert fails, and takes about 65 ms. Searches while nothing has changed were already fast. The rebuild still happens in full, not per changed note. See [docs/open-items.md](docs/open-items.md).

## 2026-11-20 — small things for daily use

Enter in a note's title moves to the writing. The Tasks tab puts the cursor in the add field when it opens and keeps it there after a task is added. Ctrl or Cmd with a full stop turns focus on and off. A Keys card in settings lists every key, because most have no button to find them by.

Ctrl or Cmd with Shift and D adds the word at the caret to a personal dictionary, so a name or a term of the trade stops being underlined. It is a plain file, `dictionary`, in the notebook folder, one lowercase word per line. It stays on this computer, like the names and the reading marks, and it is not sent. Its plurals and possessives are fine too.

## 2026-11-20 — writing with nothing else in view

An icon at the top right of a note turns on focus. Both panels go away, and the title shrinks to a small field at the top right with a button beside it that brings the formatting bar in. The round buttons at the lower right slide off to the right and leave a handle to pull them back, and the bar slides up out of sight. The tags and the note warning line go too. The panels' open or closed state is not changed, so leaving focus puts everything back at once. Focus is for the wide layout and a note being written. It is not for reading a book or an older version.

The panels, the buttons, and the bar slide for about a fifth of a second. A timer redraws the window only while one of them is moving. Typing does not start it, and neither does scrolling. A later version of focus will spotlight the paragraph being written.

The "Written by" line is off the page. The note list still shows the writer. The writer is kept for a per-note info icon, later.

## 2026-11-20 — the page parses and measures only what changed, and its marks can be reached

Typing felt late in a note of any length. Every key re-parsed the whole page, `follow` measured every line again to find the caret, and `view` shaped every paragraph again to wrap it. At 13 KB a key cost about 12 ms to parse, 25 ms to follow, and the view on top of that. Now the parse restarts from the line that changed and stops as soon as the marks open at the top of a line match what they were. A test checks it against a full parse over thousands of random edits. Wrap widths and wraps are remembered, since they depend only on the text, the width, and the font. The start of each line is a running sum and no longer a scan from the top. The desktop crate and the crates on the saving path are built with optimizations in a dev build too. A key plus its view is now about 0.6 ms at 2.5 KB, 3 ms at 13 KB, and 12 ms at 51 KB. It was about 7, 36, and 130 ms.

The caret used to open only the word it was in, so the `**` around a bold phrase stayed hidden until the caret landed exactly on them. It now opens the whole construct: the outermost run of marks around the caret, or a whole link, image, or note link. A caret moved to just after a construct, by a key or the pointer, shows it too, so a Backspace there acts on marks that are on the screen. A caret that got there by typing does not, so the closing mark still settles at once. A link with spaces in it also settles when its last mark is typed.

Bold and italic together, as in `***both***`, now toggle one layer at a time. They used to add a second layer. The underscore spelling, `_em_` and `__bold__`, is taken off too. Enter on an empty list item ends the list, and on a nested one steps out a level. This replaces the earlier rule that it added another empty item. A quote continues on Enter and ends the same way. Tab nests the list items under the caret or a selection and Shift+Tab brings them out. Ctrl or Cmd with B, I, K, or E is bold, italic, link, or code, and with Shift and X or H is strike or highlight.

## 2026-11-20 — dev builds optimize dependencies, and the lints pass

A plain `cargo run` builds in the dev profile, which ran the crypto crates, SQLCipher, and Iced unoptimized. Checking a signature took about 170 times as long as in a release build, and the window felt slow. The workspace manifest now builds dependencies with `opt-level = 3` in the dev profile. The workspace's own crates stay unoptimized. The first build after this is slower, and every build after is not. In a debug build a save with 400 notes is now about 115 ms and the reload after it about 2 ms.

`just lint` was failing before this change. It passes now, for the whole workspace and all targets, with warnings denied. Nothing was allowed or silenced to get there. The unchecked indexing in `sync.rs` is gone, because `sync_chat` finds its conversation once and `sync_except` marks each conversation as part of the sync or not instead of keeping a list of positions. `sync_except` is split into small functions. A function with no callers, `store_key_from_root`, was removed.

The retry loops added on 2026-11-20 gave up too soon. A writer in a tight loop can win the lock several times in a row while another waits, and the lock is not first come, first served. Each loop now allows 32 tries and waits a little longer after each one, up to 8 ms. A test with three writers failed every time once the faster build made them collide more.

## 2026-11-20 — scrollbars are hidden

Every scrollable in the window goes through `vscroll` and `hscroll`, so they share one rule. A scrollbar is no longer drawn at rest. Scrolling a pane shows a faint thin thumb for about 0.7 seconds, only on the pane under the pointer, and then it goes. A clear thumb shows while the pointer is on the bar itself, so a person who wants to drag it still can. Hovering a pane does not show it. Scrolling is how the bar is asked for, so typing does not draw it and does not start a timer.

The first version sent a message for every scroll tick. Each one went through `update` and rebuilt the whole view, and it restarted a subscription, so scrolling was slow and jerky. A scroll must never go through `update`. A wheel tick now only records the time, which is one atomic store. The first tick of a burst sends one message, which arms the same one-shot wake the pause and the search use. The wake only redraws, and looks again later if the pane is still moving.

## 2026-11-20 — a new device's first note is not dropped, and a sync no longer overwrites a write

A note written on a newly enrolled device could vanish from every other device, about four runs in ten in `restored_folder_writes_back`. The note was never lost from the log, but the projection refused it. A device becomes a member when a `space.member-add` is applied. Its first note has no earlier event, so it is ready at once, and events were applied in the order of their random ids. When the note came first its signer was not a member yet, so it was skipped as `NotMember`, and the skip moved that signer's chain past it for good.

A note or message from a signer that is not yet a member now waits. It stays in the merge and is looked at again after every round. Only when a round applies nothing is it clear that no event left can enroll the signer, and then it is refused as before. A signer nothing ever enrolls is still refused. [Spec 03](docs/spec/03-spaces.md) said when an event is `not_member` only for events signed after a removal, and did not say how a first note and its enrollment are ordered when neither refers to the other. It now says so under Merge. The wire format and the vectors are unchanged. Removals keep their order among the other events.

The second fix is for writers that read a log, wait, and write it back. Sync waits on the relay between the two. A note or a message written in that wait was overwritten, and so were the notes the relay delivered. Sync, `say`, enroll, pair, and revoke now write with `write_log_if`. If the file moved, they read it again, merge again, and retry a few times. Restore still uses `write_log`, because it writes a folder that has no notes file yet.

## 2026-11-19 — a save checks each signature once

A save in the window took about 330 ms with a thousand notes, and a second reload after it about 160 ms more. Every `Notebook` started with a full load that checked every signature, the projection checked them again, and a write threw the cache away, so the next read began cold. A save, a sync, and a search each opened a new `Notebook`, so none of them found a warm one.

A signature is now checked when its wire is loaded or written, and never again. `project_log_verified` and `project_notes_verified` project events that `verify` already accepted. The older functions verify first and call them, so their behavior is unchanged. The loaded notes keep those events, and `add` and `edit` append to them in place. A new `Notebook` adopts the live one's cache while the file still has the stamp it was built from. The cache is held by weak reference, so nothing stays in memory after the last notebook is dropped. A save with a thousand notes is about 30 ms and the reload after it about 2 ms.

The write itself is safer. `write_log_if` writes only if the file is still the one that was read, and returns `Stale` instead of overwriting a write it never saw. `add` and `edit` then reload and sign again, so a second window or the CLI can no longer lose an event, or reuse a counter, which is the nonce. A lock on a sibling `.lock` file covers the check and the rename, the new bytes are flushed to disk before the rename, and the directory is flushed after it. The old write could leave a zero-length file after a power cut, and every writer shared one `.part` file name. The notebook folder now has a `notes.lock` file in it.

Sync, pairing, restore, and revoke still write with `write_log`, which now takes the same lock and flushes, but still replaces the file without checking that it is unchanged.

## 2026-11-19 — spelling knows ordinary words

The word list behind the underline was about 9,900 words from a web scrape. It flagged `cannot`, `encrypted`, `pause`, and `sovereign`, and it let `alot` through. It is now the Hunspell `en_US` and `en_GB` dictionaries, about 90,000 forms, plus a short list of technical words. The terms are in `crates/ambient-desktop/dict/SCOWL-LICENSE.txt`. Both spellings are accepted. No dependency was added.

The check also reads a word the way a person writes it. `well-known` and `and/or` are two words. `don't`, `John's`, and curly apostrophes are fine. A word with a digit, an accent, a symbol, or a capital inside it (a file name, an address, `iPhone`, `NASA`) is not judged. A capitalized word that is not in the dictionary is left alone unless it starts a sentence, because it is more likely a name than a typo. The price is that a misspelled capitalized word in the middle of a sentence is not underlined.

## 2026-11-19 — the pause and the search stop blocking

A keystroke does not search the folder and does not start a timer that redraws the window. Titles still filter at once. The body and the conversations wait about a fifth of a second, and only the latest query runs. Notes and conversations take different locks. A search of notes does not wait on a chat pull. Conversation text stays in memory on the folder thread and is reread only when a log's stamp changes. That scan happens after the lock is dropped.

A pause is one wake, about 700ms after the last key. It saves, rebuilds the group list, and checks spelling. The three-second timer only syncs. Opening a note checks spelling at once. Spelling runs on its own thread and never takes a lock.

An older version is an index built while the log is loaded. Opening it reads that one event, not every wire.

## 2026-11-18 — search, versions, and a citation

After a pause, a misspelled word is underlined and a mark that is still open is one quiet line. Neither check runs on the caret's thread. Search shows the line that matched, in a note or in a conversation. A note link can name one version of a note, `[[id@version|title]]`. The version is the event id. The current head opens the note. An older snapshot opens read-only and is not written back. A task keeps a note link on its line. A chat line cites a note in the existing `msg.put` field. That field is the note id only. It grants no key and no later edits.

## 2026-11-18 — the writing column wraps

A paragraph wraps inside a readable measure, about the width of a printed page, centered in the middle. The left list, the writing, and the group panel are three columns. Closing one side gives that room to the middle. The writing stays inside the measure and does not run under a panel.

## 2026-11-17 — the caret only paints

The window's thread paints and takes keys. Folder work runs on two background threads that share one lock. One thread pulls only the conversation on screen, a few times a second, and pushes a line as soon as it is written. The other reads notes, builds the group list, and runs the slower full sync. A full sync leaves the open conversation to that faster pull. The lock is held across a sync so a save cannot be written over, and it is not held while word counts are added up.

A click reads the projection already in memory. A save or a sync hands over a new projection when it finishes. The screen keeps what it drew until that arrives. Search shows title matches at once and fills in the rest when the scan finishes.

The group list is built when the folder opens, in that same background pass, and is on the left before any note is clicked.

## 2026-11-17 — tags, the list, the pause, and note links

A tag is a chip under the title. A `#` in the writing is not a tag. The chips are a hidden line at the end of the note, so they sync with the note and do not need a new record. Older notes that used `#name` in the writing are not filed until the chip is added.

The left list shows a group and notes that belong to no group. Pieces sit under the group. Search still finds a piece.

A keystroke changes the caret and the letters. The word count, the group, and the notes that link here update after a short pause.

A note link can store the note's id and show its title. `[[` offers notes. A link that does not resolve is drawn quieter. Command-click still opens it. A plain click still places the caret.

## 2026-11-16 — a group keeps its name

`[[]]` is a note link, not a wikilink. Two note links make a group, and the note's name is the group's name. A first line `Kind: issues` names the pieces, and is optional. The panel shows that name and the same list on a piece as on the group note. The group's name opens the group note. Read draws a line and a gap between pieces.

## 2026-11-16 — chapters sit in a panel on the right

The chapter list is a panel on the right, with the word count and Read or Write. It is not beside the title. A book opens that panel. Closing it keeps it closed. The list on the left hides the same way. Each edge has one button. On a narrow window the same button shows the chapters above the page, and the other returns to the list.

## 2026-11-16 — a new note starts in the title

A new note starts in the title. The paragraph caret goes away while the title is being written, so a key does not land in both and jump down. Clicking the paragraph brings that caret back.

## 2026-11-15 — a click leaves a highlight, a drag changes the selection

A click moves the caret and drops the selection anchor. The caret move was leaving that anchor behind, so the range followed the pointer and a highlight could not be left. This supersedes the note that treated the open mark as the cause.

## 2026-11-15 — a selection can be formatted where it sits, and a book is a contents note

After the button comes up, a range that is more than a caret shows a pill above those letters. The pill is bold, italic, strike, highlight, link, copy, cut, and paste. Paste replaces the selection. Escape clears it. Heading, list, task, quote, and the block inserts stay on the bar. The pill uses the same actions as the bar.

A book is not a new record. A note with two or more wikilinks is the contents: those names, in that order, are the chapters. The rail opens a chapter, or creates a note with that title when none exists. Each chapter shows its word count, and the rail shows the total. The contents lines are not counted again. Read draws the chapters one after another and does not write a combined note. A fence and an image embed are not chapters.

## 2026-11-15 — a selection is the letters under the pointer

A drag keeps the page where it is. The line does not reflow while the button is down, and the word editor does not keep a second drag. The highlight is those letters, including the word at the caret. Letting go stops it and opens the word under the caret. A second click selects the word. A third selects the line. Shift extends from the anchor. This supersedes the line that said a selection grows only while the button is down.

## 2026-11-14 — a drag ends with the button, and an invite is one path

A selection grows only while the mouse button is down. Letting go, or the pointer leaving the window, ends it. A click still lands on the letter.

The message box uses the same border as the other fields. Rename, remove, up, down, and search are icons. Search opens its field when the icon is pressed.

An invite is one path. Either you paste their link and start, or you send your link and wait. The name is only on the path where you start, and only this computer keeps it.

## 2026-11-14 — a due day is the end of the line, and undo stays with the note

A due day is a trailing `YYYY-MM-DD` on the checkbox line. It is not a new record. Open tasks with a date come first, soonest first. Hide done only hides them. Clear done removes those lines. The older copy stays in the log. Up and down swap a checkbox line with the next checkbox line in the same note.

Unread is a local `seen` file, the same kind of file as names. The first time a conversation is listed, it is caught up, so an older folder does not light up. A later message shows New, and the Chat tab counts those conversations. Looking at the conversation clears it. Messages are not deleted.

The message box keeps more than one line. Shift+Enter starts a line. Enter still sends. Search in the header looks through that conversation.

Ctrl+F finds in the open note. Ctrl+click or Cmd+click opens an http, https, or mailto link, or a wikilink's note. A plain click still only lands on the letter. That click rule is unchanged.

Undo steps stay with the note you left, in memory, for the last eight notes. They are still not the log. This supersedes the line that said leaving the note drops the steps.

A file still stays on this computer. The citation can sync. The bytes cannot.

## 2026-11-13 — undo is the open note, and the first open explains Settings

Undo and redo are for the note that is open. Typing and deleting each gather into one step for a short moment. A format, a paste, or Enter is its own step. Ctrl+Z steps back. Ctrl+Shift+Z and Ctrl+Y step forward. Leaving the note drops the steps. The next save writes the text as it is. It does not rewrite the log.

A new notebook shows the recovery phrase, then what lives in Settings, before the notes. The passphrase is optional and is not collected there. An older folder has no `seen` line and skips this. The Tasks list can rename a line, remove it, and search. A chat row shows the last message and its time. A message can be copied. The open chat can name the other person on this computer. This does not add a task record, a deleted message, or an agent.

## 2026-11-13 — a drag selects the letters

A drag across the page selects those letters, including words that have already settled. Delete, backspace, and a mark such as bold use that range. Bold on words that already contain a mark wraps the words, instead of leaving a stray mark in the middle. The caret word stays the only editor. A selection does not open the marks on the rest of the line. This supersedes the line that said a click only puts the caret on the letter.

## 2026-11-12 — the agent note is kept in full

The owner's note is [docs/agent-layer.md](docs/agent-layer.md), as written. [docs/agents.md](docs/agents.md) is how that note is read against the app. The decision is unchanged: an agent is a short-lived grant, not a teammate, and it is not v0.1. This supersedes the line that said the shape lived only in docs/agents.md.

## 2026-11-12 — an agent is a short-lived grant

Recorded in [docs/agents.md](docs/agents.md). Not built. An agent spawns with nothing, receives a scoped key that expires, proposes actions, and dies. It keeps no memory. A supervisor matches triggers and does not reason. What lasts is in the notes, the tasks, the messages, and a signed local log. A shared room still requires every member's agreement before an agent reads it. This does not change v0.1.

## 2026-11-12 — the caret lands on the letter

A click goes to the letter under the pointer, and hovering a word does not underline it. The note title is the largest heading. Each heading in the page is smaller than the one above it, in the same color as the text. A strike is a line through that color, and a highlight sits behind the letters. A task keeps its box, including a bare `[ ]`, a tabbed line, and a task under a quote. This supersedes the line that said a click puts the caret in the word.

## 2026-11-12 — a plain word stays on the line

The painter only recorded marks. A plain word left the caret and was not drawn, so the next word started at the left edge. The page now draws the unmarked text as well. The saved text is unchanged.

## 2026-11-12 — a settled word leaves the marks

The page draws a word as soon as its format is closed, one word at a time, and the caret stays in the gap after it. A click in a settled word puts the caret in that word and leaves the marks hidden. A heading is a real size, including on the word being typed. A strike is a line through the letters once that word has settled. The saved text is still the markdown, marks included. This supersedes the lines that said a heading is not a larger size and that the editor cannot change the size of one span.

## 2026-11-12 — settings is a set of cards, and the gear is a real icon

The welcome screen sits in the middle of the window. Settings is separate cards, not one tight list. The gear is Lucide's settings icon. The window title is only Aerodash. The small role next to the name is gone.

## 2026-11-11 — names stay on this computer

The window opens on the word Aerodash, one letter at a time, over the two lines. That stays up for at least three seconds and until the folder has opened. A folder with no key asks for a device name, a personal name, and light, dark, or system. An existing key does not. The device name is the key-file label, set once, 1 to 32 bytes. The personal name and the color are a local `profile` file. Nicknames are a local `names` file. Neither is a record. The contact card is still four fields and still has no name, so a preferred username cannot be projected yet. What you call someone is only on this computer, and their key stays visible. Devices, recovery, and erasing the folder live in settings. Erase deletes the folder's children and starts the opening again. Saving a note runs off the typing thread. The editor is not reloaded when the seal finishes.

## 2026-11-11 — a note is filed by a tag in its text

Notes are grouped by `#tag` in the body. A heading is not a tag. Fenced and inline code are not tags. The list is a view: there is no folder record, because a note is a snapshot and a tag syncs with it. `#clinic/billing` is one tag. The note screen no longer keeps a task editor under the page. A task is still the toolbar button, and the Tasks screen is still the same checkbox lines. New, delete, and send are small round buttons at the bottom right of the note. On a narrow window the new button sits on the list the same way. A pause writes the note on this computer even when the relay is off. Scrollbars are a thin thumb with no track.

## 2026-11-11 — a note is deleted by a later snapshot

The notes tab creates, opens, edits, and deletes a note. Delete asks first. The write is still `note.put`: the body is only `%%ambient-deleted%%`, and the list hides the note when every head is that snapshot. The older text stays in the log. It is not a new event type.

Tasks in the open note are the checkbox lines already there. Add, rename, check, and remove write that line. Enter saves a renamed line. Backspace is not involved. The Tasks screen is still the same view across notes.

## 2026-11-11 — enter continues a list, and a heading stays one size

Enter on a list or a task inserts the next marker, and splits the line if the caret is in the middle. A new task is unchecked. Backspace on an empty item removes the marker. This is still the same checkbox line, not a new record.

A heading is not a larger size. The editor's highlight format is only a color and a font. The text engine under it can size one span, but iced 0.14 does not pass a size through, and this window does not fork iced. `#` and `##` use the serif. Smaller headings are bold. A strike is still a color, not a line through the letters.

## 2026-11-11 — the note owns its toolbar, and a loose task is still a line

The format row sits in the note column, above the editor, not above the list. On a narrow window it stays at the bottom. The buttons are labeled Word, Line, and Insert. A word button wraps the word or the selection and toggles off. A line button toggles the prefix. An insert drops a link, table, rule, callout, or code block. The preview also reads comments, math, tags, embeds, callouts, and footnote definitions. The editor still cannot change the size of one span, and a strike is still a color, not a line through the letters.

A task added on the Tasks screen is a `- [ ]` line in a note titled Tasks. That note is created the first time. It is not a new record. Checking a box still writes the line back. Send to chat asks which conversation, then waits for Send. Chat bubbles stay inside the pane as the window gets narrower, and the words wrap. Opening the window draws two lines longer than the frame, drifting across it like a breeze, for a moment.

## 2026-11-10 — the window fits a small frame, and a chat file stays local

The note editor covers the marks people actually type: headings, emphasis, strike, highlight, code, quotes, lists, tasks, tables, links, images, and wikilinks. The marks still stay in the text. A heading is still not a larger font. The editor cannot change the size of one span.

A format row sits under the top bar when the window is wide, and at the bottom when the window is narrower than 720. Notes, Tasks, and Chat move to that bottom bar too. A wide window keeps the list beside the page. A narrow or square window shows the list, then the page.

A file, picture, or video added to a chat is copied into the notebook folder. The message cites it. The citation can sync. The bytes do not. That is not a new record. Encrypted attachment bytes stay later work. The bubble says when the file is not on this computer. A video opens with the system. It is not played inside the window.

## 2026-11-10 — a folder can hold a relay

The window is no longer hard-wired to `127.0.0.1`. The folder file `relays` is this computer, the introduction list, or a pin and a backup. The list is empty until a real relay exists, so the default stays this computer. `ws://` is refused off this machine. `wss` uses rustls. The relay process terminates TLS from a PEM the operator supplies. Pushes and cursors are per relay, so a dead relay is not treated as delivered. A checkmark means the note is saved on this computer. The globe stays gray, red, or green. The address is behind Advanced. The replacement test, with a second computer, is not done.

## 2026-11-09 — the relay keeps its mail

The loopback relay writes a `mailboxes` file and loads it on the next start. Cursors are not reused. A file that does not parse is refused and left alone. The window's file is under `~/.local/share/ambient/relay`. This is still cleartext on this computer. It is not a public relay. See [docs/network/README.md](docs/network/README.md).

## 2026-11-09 — connection is its own track

The relay on this computer is not the network. The protocol already is: identity, Spaces, sealed logs, mailbox ids. The missing piece is a connection a second computer can use without an address.

That work is [docs/network/README.md](docs/network/README.md), inside this repo, not a second project and not a second protocol. Spec 04 still wins on bytes. The owner started it before the v0.1 exit. It does not pull groups, Tor, payments, or a directory forward. v0.5 remains the later self-host and export.

An introduction list run by the owner is a convenience, not an authority. The claim worth making is that the relay can be swapped and cannot read, and that the notebook stays on the device.

## 2026-11-09 — tasks are a view, and the marks have to read

The Tasks screen lists `- [ ]` lines from notes. Checking a box writes that line back into the note. It is not a new record. A note with two versions is left alone until someone saves one.

In the editor, a heading is the serif face, code is a warm monospace, strike is a different color, and a finished task line goes quiet. The marks stay. The editor cannot change the size of one span, so a heading is not a larger font.

## 2026-11-08 — a feature is a room, a record, or a view

Before adding a kind of thing, read "Adding a feature" in [ARCHITECTURE.md](ARCHITECTURE.md). A room is a Space (who can have the log). A record is one signed event (how two edits merge). A view is a local picture of records and is not synced. Most new screens are views over documents. A new record is only for a different merge rule. A new room is only for a different guest list. Not an object graph.

The note body is one editor. Markdown is styled as it is typed: opening `**` starts bold, and the closing pair ends it, including across lines until a blank line. The marks stay in the text. That text is what is saved. This supersedes the click-a-line preview from 2026-11-06.

## 2026-11-06 — crates only when they are the standard

Dependencies stay on a short list. A new crate is allowed only when it is crucial and the standard for that job: ubiquitous, as small as the job allows, and actually fast. Iced, the audited crypto crates, and the protocol libraries already chosen stay. A convenient or merely popular crate does not. Build the feature. The note preview is the example: a small parser in the window, not a markdown crate. See [ADR-0003](docs/adr/0003-workspace-and-tooling.md).

## 2026-11-06 — markdown preview, chat in time order

The note body previews a small markdown set: headings, emphasis, lists, task boxes, quotes, code, links, and rules. A line shows the rendered text until you click it, and then the marks come back. Enter or an arrow leaves that line and renders it. What is saved is still the raw markdown. Chat messages are reading order by the advisory clock, oldest first, so the latest sits at the bottom. The prev chain is unchanged, and the clock is still not a protocol order. Your messages sit on the right. Theirs sit on the left.

## 2026-11-05 — Aerodash, and a click does not re-read the log

The window is named Aerodash. Choosing a note, a chat, or Devices reuses the projection already in memory. A sync that brought nothing does not rewrite the sealed file and does not reload the window. Search keeps the SQLCipher index open instead of deriving the key again on every letter.

## 2026-11-05 — typing, notes on the other folder, parchment

Typing does not wait on sync. The window runs sync off its own thread, and a short pause writes the open note without moving the cursor, so a new note or an edit reaches the paired folder without a separate Save. A note that is already saved refreshes when the other folder's version arrives. The window is parchment: light `#EDEADE` with `#16446F`, dark `#1C1917` with `#66A5E1`. Titles are Crimson Pro. The rest is DM Sans. A sun or moon beside the globe switches them. Notes and chats in the list are rows, not the same control as Save, Send, or New note.

## 2026-11-04 — globe, auto-sync, drafts, restore enrolls

The window no longer has a Sync button or a relay button. A globe at the right of the toolbar is gray when the relay is off, green when it is on and nothing is unsent, and red when it is on and something has not been pushed. Clicking it opens a popover under the icon, over the notes, with the last sync line and a control to start or stop the relay this window started. The relay starts when the folder opens, and the window syncs on its own while that relay is on. The relay is still cleartext on `127.0.0.1`. It is not a public server.

An unsaved note is kept when you switch notes, start a new note, or open chat. Restore from the phrase still does not recreate old device keys. It appends `id.device-add` and one `space.member-add` signed by the identity key, so the new folder can sync. See [spec 02](docs/spec/02-identity-log.md).

## 2026-11-04 — revoke, writers, and more than one conversation

The root window lists devices and can revoke one. Revoke keeps the notes that device already wrote and stops sealing new events to it. The note list and the editor name the device that wrote the current version. A folder can hold more than one conversation. Each conversation has its own contact key. The first one still lives in `chats`. Later ones live under `conversations/`.

## 2026-11-04 — user-owned only if the later problems stay grants

The later system is still user-owned only if recovery, node payment, agent power, sensing, sleeping nodes, bootstrap, and abuse handling stay inside Spaces and grants. A phrase is the v0.1 backup, not the long-term recovery. No single guardian, operator, or agent may seize the root. See [ARCHITECTURE.md](ARCHITECTURE.md). None of this is v0.1 work.

## 2026-11-02 — one primitive, local unlock, message time, search index

Agents must keep one membership model: a new object is a Space of the same kind, a capability grant, or a copy. Not a second sharing protocol. See [AGENTS.md](AGENTS.md) and [ARCHITECTURE.md](ARCHITECTURE.md).

Message events now set envelope field 4 (advisory unix milliseconds). The window shows that time. It is not an order.

The key file can be wrapped with Argon2id and ChaCha20-Poly1305. The passphrase unlocks the file in this process. It is not the root and not a WebAuthn passkey. The Linux window has no platform passkey API.

Native search builds a SQLCipher FTS5 index beside the sealed log. The log is still the source of truth. `ambient-store` stays a sealed file so the protocol crates still compile for wasm.

## 2026-11-01 — contact invite is a link or a QR

The notes window shows a shareable `ambient://contact/` link and a QR of that link. Pasting the link or the raw card starts a conversation. A selected note can be copied into the chat as text. The recovery phrase and restore remain in the window.

## 2026-10-31 — Recovery seal and phrase restore

The root device seals the identity log and Notes-space events to the identity seal key (HKDF `identity-seal`) and puts the blobs in the recovery mailbox (HKDF `recovery-mailbox`). A new folder can be created from the BIP39 phrase of the root seed; it fetches and opens those blobs. Device keys are still not derived from the root. See [spec 02](docs/spec/02-identity-log.md).

## 2026-10-29 — The notes window syncs on loopback

The Linux window stayed offline so protocol rules would not land in `ambient-desktop`. The owner is testing the relay, a second device, and 1:1 in that window, so the window now calls the client in `ambient-cli`. It still does not seal events or parse relay JSON. The relay it can start is cleartext on `127.0.0.1`, which [spec 04](docs/spec/04-relay-api.md) says is not a public deployment. Recovery, a QR invite, revoke, and copying a note into chat are still not done.

## 2026-10-28 — Linux is an Iced window, not a webview

The owner corrected what "native" means for Linux. It means a real desktop window, not a page drawn by WebKit. It does not mean the app has to look as if GNOME or KDE made it.

[ADR-0002](docs/adr/0002-rust-core-native-clients.md) is amended. The Linux client is Iced. Mac, Windows, iPhone, and Android stay SwiftUI, WinUI, SwiftUI, and Compose. The web client is a PWA and only a fallback. Dioxus is not a client. The notes window in `ambient-desktop` is the Iced app over the same folder the CLI uses.

## 2026-10-27 — Roadmap

[ROADMAP.md](ROADMAP.md) is the sequence. v0.1 is specified and not yet implemented. Later versions do not start until the previous one is in use. The README table is the short form of that file.

## 2026-10-27 — v0.1 specs, before any protocol code

Wrote the four v0.1 specs and the first conformance vectors.

- [docs/spec/01-events.md](docs/spec/01-events.md): canonical CBOR envelope, BLAKE3 ids, Ed25519 over the unsigned bytes, per-signer chains.
- [docs/spec/02-identity-log.md](docs/spec/02-identity-log.md): root seed, HKDF to an identity key, a seal key, and a recovery mailbox. Device keys are not derived. Pairing does not copy the root.
- [docs/spec/03-spaces.md](docs/spec/03-spaces.md): Notes and conversation Spaces, HPKE to member keys, `text-snapshot-v1` with an explicit conflict rule. Not a CRDT, and not MLS.
- [docs/spec/04-relay-api.md](docs/spec/04-relay-api.md): mailbox id is the capability. The relay stores opaque blobs.

Also added [ARCHITECTURE.md](ARCHITECTURE.md), [CONTRIBUTING.md](CONTRIBUTING.md), and [docs/TESTING.md](docs/TESTING.md). Rewrote [SECURITY.md](SECURITY.md) so it points at the threat model.

Not done: a second implementation of the vectors, and any crate behavior. Spec 05 (long-form creations) is intentionally unwritten.

## 2026-10-27 — Spaces stay, MLS does not

The owner and Grok reconciled the written plan (README, VISION, STACK, ADR-0001 through ADR-0004, the OpenMLS spike) with a later decision against MLS.

Kept:

- A Space is a membership list plus the merge of each member's signed log.
- SimpleX contributes one-way mailbox addresses only. It is not a replacement for Spaces.
- Rust core, Dioxus on desktop first, native mobile later.
- Key-event log. Passkeys unlock the seed. They are not the root.
- Chat, membership, and feeds are logs. A CRDT is for document bodies only.
- The relay stores ciphertext and must not learn a global identity.
- No wallet. No agent runtime in v0.1. An agent, later, is a delegated key with a grant. A shared Space still needs every member's consent before an agent can read it.

Changed:

- MLS is not the cipher for a private Space. [ADR-0005](docs/adr/0005-private-space-encryption.md) supersedes that sentence wherever VISION, STACK, or a crate rule still said it. The OpenMLS spike is kept as evidence and marked not adopted.
- A private Space seals each event to the members' current device keys with HPKE (RFC 9180). There is no second, hand-rolled ratchet.
- v0.1 is the owner's Notes space and 1:1 conversations, on desktop first. Personal notes are not deferred to v0.3. Phone push waits for the phone client.
- v0.1 promises content confidentiality and unlinkability only. See [THREAT_MODEL.md](THREAT_MODEL.md).

Not done yet:

- The normative specs in `docs/spec/` (events, identity log, Spaces, mailbox API) and their conformance vectors. No crate behavior until those exist.

## 2026-11-20 — The second list from daily use

The owner used v0.1 for a short time and asked for nine small things. No locked choice changed. Recorded here because they change where things are.

- A key the editor does not take, while the page is up, is typed into the page by the window. Fast typing lost letters when the page moved in the widget tree and the editor lost focus until a task gave it back. The state-level test never dropped one, so the loss was in the gap, not in the text.
- New note is at the bottom of the left list. Delete is on a row under the pointer. Send to a chat and Delete are in the note's settings, which the gear in the right panel switches to. Links and backlinks stay the default.
- The Tasks note is not in the notes list. The Tasks tab opens it in a card.
- Select words and Send in the pill sends those words, quoted, with the title, the lines, and the writing device, and cites the note.
- The tag field offers tags used before and narrows as you type.
- A scrollbar has ten pixels of room. A wheel notch is a page step, not 60 pixels.
- The side panels ease in and out of focus.

Not done, and why: [open-items.md](docs/open-items.md). Nothing was seen in a window.

## 2026-11-20 — The list nests groups inside groups

A piece that was itself a group showed twice: under its parent and again at the top as its own group. The list is now an outline. Groups nest to any depth, each has a handle to fold it, and a note is drawn at the top only when no group links it. Loops of links end. Folded state is not kept between runs. Nothing was seen in a window.

## 2026-11-20 — Links, groups, and the graph are three separate things

The nested outline of the earlier entry was the wrong model. It treated a link as containment, so a note with two links became a group, and a note linked by one group that had its own links appeared twice. The owner asked for links to be visualized correctly and said a group should not need a new type. What replaced it:

- **A link is a reference.** The right panel of the open note lists `Links to` and `Linked from`. A link never nests a note in the left list.
- **A group is a tag.** `Novel` is a group and `Novel/Part 1` is a group inside it. It is stored on the note's hidden tag line, as tags already were, so there is no new record, no reference note, and no new merge rule. The left list is built from the tags with a fold handle on each group. The earlier link-built outline, `outline`, is removed.
- **A graph** behind a button at the top of the left panel draws notes as dots, links as lines, and a color per top-level group. A dot opens its note.
- The reading pieces (a note with two or more links, a `Kind:` line, Read) are unchanged. They are a reading order and do not nest the list.

The word group in the notes UI means a tag path. It does not mean a multi-member Space, which is what ARCHITECTURE and ROADMAP call a group. Nothing in the protocol changed. Nothing was seen in a window.

## 2026-11-21 — Links only reference; Read belongs to a group

The owner asked what links were for once groups were tags. Two or more links in a note no longer make a contents note, a chapter rail, an auto-opened panel, or a Read mode, and the `Kind:` line is gone. Links are references: `Links to` and `Linked from` in the right panel, the lines in the graph, and a count under the tags that opens the panel. Read now reads the notes of the group in view, in title order. Not built: the link field beside the tags, which the owner wants after a few days of use. Nothing was seen in a window.

## 2026-11-21 — The graph clusters by group, and the right panel is restyled

The graph gives each top-level group its own cluster of dots inside a soft circle of the group's color, with the name on the circle, so a link from one group to another stands out as a heavier line crossing between circles. Titles wrap, and the picture keeps a margin from the edge and sits in a card. The right panel has a larger serif title, a larger gear with a soft background while settings are showing, a line under the header, small capitalized section labels with counts, and taller rows. Nothing was seen in a window.

## 2026-11-21 — Notes in a group are ordered

Notes in a group are alphabetical, counting numbers as numbers, so Chapter 2 comes before Chapter 10. The owner can drag a note by a grip to sit beside another note in its group, or onto the group's name to put it first, for a prequel. Read follows the same order. The hand order is a plain local file named `order`, not part of the sealed log, so it does not sync to a second computer. That was chosen to avoid a new record type before the owner had used it. If it should sync, it needs a record with its own merge rule, and so a spec edit and an ADR. THREAT_MODEL now lists the file. Nothing was seen in a window.

## 2026-11-21 — Typing a tag no longer jumps to the page

The first key in the tag field made the tag suggestions appear, which changed the row that holds the field from a row to a column. Iced then built the field again, and it lost the caret. The next keys had nothing focused, so the window's own typing fallback put them in the note body. The tag row now has the same two rows whether or not anything is offered, and a test checks its shape stays the same as the draft changes. The same cause would show for any field that gains a sibling as you type, so keep a field's container steady.

## 2026-11-21 — The task field clears after every task

The field was cleared only when the status message was empty after the write, and the message still held "Added to Tasks." from the task before. So the first task cleared the field and the next ones did not. The message is now cleared before the write, so a message after it means the write complained. A test adds four tasks in a row and checks the field each time. Nothing was seen in a window.

## 2026-11-21 — The machine is asked through one trait

The owner chose the long-term path for a Mac: one Rust core and a native UI for each platform, so Iced stays the Linux window. The first step is the notebook asking the machine through one trait, `Host`, in `ambient-cli`, instead of reading environment variables and running programs in several places. It answers the home and data folders, dark mode, opening a file or a link, and choosing a file. There are implementations for Linux (the old behavior, moved), macOS (written, never run on a Mac), and a bare one that refuses. The relay's mail folder, the system theme, the backup place, opening files and links, and the file chooser all go through it. Nothing about the folder or the wire changed. Rules for the core and what is left are in [docs/platforms.md](docs/platforms.md). A design system shared between the UIs is the owner's stated goal and is not started.

## 2026-11-21 — The Tasks panel, and due dates from a calendar

The left panel of the Tasks tab had no use. It now chooses how to look at the tasks: by date (All open, Due today, Overdue, Upcoming, No date, Done), by group (the groups the notes with tasks are in), and by note, each with a count. A due date was a typed `YYYY-MM-DD` at the end of a task line, which a person can get wrong. It is now picked from a calendar that opens from a Due date button beside the add field and on a task being edited, and it writes the date in that one form and only for a day that exists. The typed field is gone. A date in a note that is not a real day, such as `2026-02-30`, now counts as no date. The format of a task line did not change, so older tasks and other notes read as before. Nothing was seen in a window.

## 2026-11-22 — "Today" is a reading of the clock, and tasks get a time and a repeat

The owner asked that nobody be mistaken about when something is due. Three things were wrong or loose.

- **The zone reader ignored the rule at the end of a zone file.** A file lists the changes it knows, then gives a rule for every year after them. Files written in recent years list changes only up to the last time a government changed the rule, and the offset after that comes from the rule. Reading only the list gives the wrong hour all summer, and every year after 2037 in a long file. `zone.rs` now reads both, and was checked against Python's `zoneinfo` on 12,000 instants in eight zones from 2000 to 2065, on the system's own files, with none wrong. The same reader serves the chat times and backup names, which had the same fault.
- **Nothing redrew at midnight,** and the zone was read once. The window now holds one reading of the clock, `App::clock`, taken when the Tasks tab opens, when the window comes back, before anything that writes a day, and every 20 seconds while that tab is up. Every date and late task on a screen is worked out from it, and the top of the Tasks panel says the day, the time, and the zone. A reading costs about 2 microseconds.
- **The zone is asked through `Host`** (`time_zone`), so a Mac or a phone answers it its own way. This removed the last `/etc/localtime` read outside the host.

Then the rest of the task features the owner asked for, except reminders, which wait for notifications. A task can have a time of day, and a repeat, set in the calendar and written as the end of its line: `2026-11-20 14:30 repeat:weekly`. A task is late the minute its time passes. Checking a repeating task moves it to its next day and leaves it open. A time has no zone. Nothing was seen in a window.

## 2026-11-22 — Time settings, a calendar that fills its card, and trips

- Settings has a Time section: a 12 hour clock by default (`3:42 PM`) or 24 hour, and weeks that begin on Sunday or Monday. Times are shown that way everywhere, including chat. What is written on a task stays `HH:MM`, so changing the setting changes nothing in a note.
- The calendar's days were a fixed width in a card that the other rows made wider, so the grid sat to the left with empty space. Every row now spreads across the card, which is as wide as the pane allows up to a limit, centered, and a click outside closes it.
- A person on a trip is noticed. The first named zone the folder is opened in is home. A later zone with a different name that sets the clock differently over the next year gets one notice on the Tasks tab, naming both and saying what the clock at home reads, and nothing on any task is changed. Two names for one place are not a trip. `Host` gained `zone_named` to read the home zone's file.

Nothing was seen in a window.

## 2026-11-22 — Popups float, panels do not squeeze, narrow windows lose the old buttons

A pass over how the window behaves when it is small, and over what opens and where.

- The round buttons were still on a narrow window's note. They are gone. The list has New note at the bottom, and a note has New and a gear in its header, which opens the panel with Send and Delete.
- The panel on the right was squeezing the page down to a hundred or so pixels between 720 and 1020. It is beside the page from 1020 and a sheet over a dimmed page below that, and narrowing past 1020 closes it.
- The Due date calendar floats under its button, and flips above it when there is no room. On a narrow window it is a centered card over a dimmed page. The Cite menus float under their buttons and no longer push the field and the list down. Delete, send to a chat, and clear done tasks are centered cards, where send used to push the note down and delete was in the corner. All of them are built on the one `anchored` overlay the sync card already used, which now takes the message that closes it, or on `modal`.
- A narrow window can reach the ways to look at the tasks through a Showing button that opens them as a card. They had no way in. The add row of the Tasks tab, the tag row, and the row of task tools wrap when there is no room.

Nothing was seen in a window. The new test draws every screen at nine widths with every popup open, which catches a panic and not an overflow.

## 2026-11-22 — Popups were never drawn, and the narrow window is one model

The owner found that the calendar and the Cite menu did not appear on a wide window and the sync card did not appear at any width. The cause was `Wheel`, the wrapper added to make the wheel scroll a page step: it did not pass up the overlays of what it held, so nothing drawn as an overlay, which includes every hover label, reached the window. It does now, and that is the first thing a widget of ours that wraps a child must do. The tests built the views and could not have seen it.

The same report found that the centered cards on a narrow window could not be closed except by acting on the page behind them. They were two layers, a dim part and a card layer over it, and a stack hides the layers under the one the pointer is over. Each is now one layer.

The owner also found the narrow window confusing: the Tasks tab's list had become a popup behind a button, the graph could not be opened from the list, and a conversation had no way back to the list of conversations. Every tab is now the same two steps: its list, then the thing chosen, with a Back at the top of the second. For Tasks the list is the views, which the panel on the left is on a wide window, and the popup and its button are gone. In a conversation, Add, Cite, and Send are icons on one line with the box.

The explanation under the add row of the Tasks tab, and the legend above the graph, are gone. An onboarding sequence will explain things. Nothing was seen in a window.

## 2026-11-25 — A test that draws and clicks real frames

The popup bug got past every test because the tests built the views and never drew them. `headless.rs` now lays out, draws, and clicks the whole window on the software renderer, with the program's fonts, with no display. It checks that the sync card, the Cite menu, and the calendar are drawn and that a press elsewhere closes them, that a press on the dimmed page closes a card and a press on the card does not, that the right panel closes as a sheet, and that nothing runs past the right edge at nine widths. With the wheel wrapper's overlay passing taken out again, the first of those fails. It rasterizes nothing.

It needed two crates as test-only dependencies, `iced_runtime` and `iced_tiny_skia`, which are already in the build as parts of `iced`, at the same versions. No crate was added to the program, and `just deny` was not run.

## 2026-11-25 — The editing gaps are closed

A bold or italic key inside code does nothing, and the address of a link is left alone. A web address is one word when it is marked. Enter in the middle of an item no longer leaves a space at the start of the next one. An ordered list is counted again when an item is nested, brought out, added, or removed, and a nested list starts at one. Backspace at the start of an item's words takes it out a level, or takes the marker off. Ctrl or Cmd with Enter checks a task. A backtick comes in a pair, and a mark typed on a selection wraps it. The typing path is unchanged for every other character. Nothing was seen in a window.

## 2026-11-25 — Recently deleted, and a note's version history

The trash button at the top of the notes list shows the notes that were deleted, each as it read before, with Bring back. A note's panel has Version history, which lists every snapshot the note has had with Read and Restore. Both restore by writing the old words as a new snapshot after the others. Nothing in the log is changed or removed, and no event or format is new. A restore is refused while the open note has typing that is not saved yet. Neither is a way to erase anything: a deleted note's text stays in the log, as the threat model says.

## 2026-11-25 — Find and replace, notes carry a time, and a date filter

Find in the note has Replace and Replace all, and Replace all is one undo. The notes list can be limited to what changed in the past day, week, month, or year.

That needed a note to know when it was changed, and note events carried no time. They now carry the advisory time field of the envelope, as a conversation line always did. It is inside the sealed event, it is not an order, nothing merges by it, and no format changed: the field was always optional. A note written before has no date. A device with a wrong clock writes a wrong time, and the version history and the date filter would show it. The same time gives the version list and the recently deleted list their dates. [THREAT_MODEL.md](THREAT_MODEL.md) says so.

`an_invite_link_carries_a_signed_name` failed about one run in three on the branch as it was. It changed the last character of a base64 string, which can carry bits that are not part of the bytes, so the change sometimes altered nothing. It changes the one before it now, and passed ten runs in ten. The test was wrong, not the code.
