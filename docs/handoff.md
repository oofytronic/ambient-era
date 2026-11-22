# Handoff

For the next agent. Written 2026-11-20 at the end of a long working session with the owner. Read [AGENTS.md](../AGENTS.md) first, then this, then [session-2026-11-19-to-20.md](session-2026-11-19-to-20.md) for the story and [open-items.md](open-items.md) for what is undone.

## State

- v0.1 is declared. It is notes on one computer, in the Linux window (Aerodash, `crates/ambient-desktop`). The owner is using it daily for a week and sending small requests. v0.2 (a second computer, a relay, a 1:1) is next and has not started in the window.
- `test` is the working branch. `trunk` is fast-forwarded to it after each batch the owner approves. Only fast-forward `trunk` when asked. Do not make other branches. Version is 0.1.0 and untagged.
- No display was available in any session so far. Every window behavior is covered by state and pure-function tests, never by eyes. When the owner reports a visual problem, believe them over the tests.

## How to work here

- Gates, gated before each commit: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked`. `just` and `cargo-deny` were not installed; `just deny` has never been run here.
- `crates/ambient-desktop/src/main.rs` is about 10,000 lines: `App`, `Message`, `update`, `view`, and the tests at the bottom (`app_tests` drives a real `App` on a temp folder through `update`; Iced `Task`s cannot be run from tests). `markdown.rs` is the editor and its custom widgets. `lane.rs` is the background thread that builds the side data (links, backlinks, graph edges). `profile.rs` is local settings, backup, and export.
- A scroll must never go through `update`. A key must never be lost: the window listener types a key the editor did not take. Iced widget state, including focus, is tied to tree position, so keep the layout's shape steady when something shows or hides (the side panels stay in the row at width zero for this). The tag field jumping to the page was this: suggestions appearing turned its row into a column. Give a field a container of the same shape whether or not its extras are showing.
- The workspace denies unwrap, expect, panics and unchecked indexing outside tests, and runs clippy pedantic. No new dependencies. Changes to the crypto, identity, spaces, capabilities, or relay crates need the owner. Say so in a commit that changes how a write is ordered or retried.

## What was built, in order

See the table in [session-2026-11-19-to-20.md](session-2026-11-19-to-20.md) and the dated entries at the end of [HISTORY.md](../HISTORY.md). The short version: performance and data-safety work on the single-device path, a real spellcheck, a flaky merge-order bug fixed, markdown editing made smooth, focus mode, backup, and then two lists of small daily-use requests: typing fixes, a settings panel per note, hover delete, a send-selection-to-chat button, tag suggestions, scrollbar room, wheel speed, and the side panels easing.

## Links, groups, and the graph, which changed twice

This is the part most likely to be misread, so it is spelled out. The notes UI now has three separate ideas:

1. **Links**, written as `[[ ]]` in a note. A reference. The right panel shows `Links to` and `Linked from`. A link never nests anything.
2. **Groups**, which are tags. `Novel/Part 1` is a group inside `Novel`. Stored on the note's hidden tag line. The left list is built from them by `group_outline`. No new record type exists and none should: the owner asked for this specifically. See "One primitive" in [AGENTS.md](../AGENTS.md).
3. **The graph**, a toggle at the top of the left panel: dots, lines, a color per top-level group.

Do not confuse the word group here with a multi-member Space, which is what ARCHITECTURE and ROADMAP call a group, and which is v0.2 later. The first design nested notes under the note that linked them. It duplicated notes and was dropped. The earlier rule where a note with two or more links became a contents note with a chapter rail is gone. Read mode now belongs to a group: it reads the notes in the group in view, in title order. Links only reference; a count under the tags opens the panel that lists them.

Notes in a group are ordered alphabetically and can be reordered by a drag on a grip. That order is the local file `order` and does not sync, on purpose, until the owner decides it should (it would need a record with a merge rule and an ADR).

## Platforms

The owner chose the long-term path for a Mac and later phones: one Rust core with a native UI for each, not Iced everywhere. The first step is done: `Host` in `ambient-cli` is every question the notebook asks of the machine, with Linux, macOS, and bare implementations. Read [platforms.md](platforms.md), and keep to its rules for the core: no environment, programs, or path conventions outside `host.rs`, protocol crates stay browser-compatible, and no UI in the notebook layer. `ambient-core`, `ambient-ffi`, and a design system are all still to do, and the owner wants the UIs to look alike though built differently.

## Time

Dates are `dates.rs`, a due moment is `due.rs`, the time zone is `zone.rs`, and what time it is comes from one reading, `App::clock`. Do not call `clock::now_ms` to decide anything on screen: refresh the reading and use it. A due moment is written only by the calendar. The zone reader handles the rule at the end of a zone file, which an earlier version missed and was an hour wrong all summer. Tests that depend on the day fix the clock with `pinned_clock`. How times are shown is a setting (`clock::hour12`), 12 hour by default, and only display uses it: `due::Time` is always `HH:MM`. A computer in a zone other than home gets a notice and no change to any task.

## Owner preferences learned

- Short plain answers. Say what was done, what was not, and what was not seen. Do not claim a visual result.
- Before a design change, say what you think and ask the one question that matters. The owner rejected two designs for links before settling on the third, so check the model before building.
- Docs are kept honest and current. Add a [HISTORY.md](../HISTORY.md) entry when a choice changes, and update the desktop `AGENTS.md`, which describes the window in one long paragraph, and [open-items.md](open-items.md).
- Decisions that need the owner are in the first table of [open-items.md](open-items.md): review of the merge change (ADR-0006), the recovery mailbox conflict with the relay spec, licensing, the crypto dependency bumps, two editing rules, backups by hand, and dev-build optimization.

## First things to look at in the window

The owner has not seen, or has only just seen, the following: the note settings panel and its gear, the hover delete, the Tasks note card, Send in the selection pill, tag suggestions, scrollbar room, the wheel step, the focus-mode panel slide (the panels squeeze rather than slide), the group list with its fold handles, `Links to`, and the graph.
