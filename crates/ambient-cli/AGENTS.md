# ambient-cli

The `ambient` command and the library the Linux window calls. It owns the notebook folder: the key file, the sealed notes log, the conversation logs, the SQLCipher search index, and the sync that moves them to and from a relay. It is a client, not a protocol crate. It does not define a byte on the wire.

**Public API:** `Notebook` (the session both the CLI and the window use), `run` (the command line), `Draft`, the sync, pairing, recovery, and network-setting functions re-exported from `lib.rs`, and `Host` with `host`, `set_host`, and the implementations for Linux, macOS, and neither.

## Invariants

- **The log is the truth.** The sealed file is the source of notes. The search index and every cache are projections. Losing one of them loses nothing.
- **A signature is checked once**, when a wire is loaded or written. `Notebook` keeps the verified events, and every projection after that starts from them with `project_log_verified` and `project_notes_verified`. Do not call the wire-based projections in a loop.
- **A `Notebook`'s cache is shared.** A new `Notebook` on the same folder adopts the live one's cache while the file still has the stamp it was built from. The registry holds weak references, so nothing stays in memory after the last `Notebook` is dropped. Do not hold a strong reference anywhere else. Plaintext must not outlive the notebook that opened it.
- **No write replaces a file it did not read.** Writers use `write_log_if` with the stamp they read. `Stale` means the file moved. Read again, build again, and try again, up to `WRITE_TRIES` times with `pause_before_retry` between them. Never write over a log the caller has not read at that stamp. The exception is creating a folder that has no log yet.
- **A sync merges again.** Sync reads, waits on the relay, and writes. If the file moved in that wait, it takes what is there now and files what arrived into it again. Events from the relay are not lost because a write was refused. Conversation slots carry the stamp they were read at, taken before the read. `Slot::arrived` keeps what the relay delivered until the write.
- **The notebook folder has other files.** `notes.lock` and `*.lock` are the write locks. `*.part` is a write in progress and is renamed over the log. `names`, `profile`, `seen`, `dictionary`, and `order` are small local files. None of them is sealed. They hold no keys, but they can hold words and names from notes. Do not put anything in them that the threat model does not allow.
- **Do not index the notes slice or a `Vec` of slots by position** in production code. The workspace denies unchecked indexing. `sync.rs` used to; it does not now. Use `get`, `get_mut`, `iter().find`, or a flag on the item.
- **The machine is asked through `Host`.** No environment variable for a home or data folder, no program name, and no `Command` anywhere in this crate except `host.rs`. A new question about the system is a new `Host` method, with an answer for each host. See [docs/platforms.md](../../docs/platforms.md). `MacHost` has never been run on a Mac.
- **No user data in logs.** No note text, message text, keys, mailbox ids, or tokens.

## Dependencies

May depend on: `ambient-events`, `ambient-identity`, `ambient-spaces`, `ambient-store`, `ambient-transport`, and `ambient-relay` (for the loopback relay the window starts). It does not compile for wasm and is excluded from `just wasm-check`. Add external dependencies only with a stated reason, then run `just deny`.

## Targets and review

- Native only.
- Human review: standard. A change to how a write is ordered, locked, or retried is a change to data safety. Say so in the commit.

## Test

```
cargo test -p ambient-cli
```

- `tests/sync.rs` runs two folders through an in-process loopback relay, in the same process. It includes two stress tests that interleave a local writer with a sync that is waiting on the relay. Run a test you suspect is flaky many times, not once. The restore test failed about three runs in eight before its cause was found.
- `session.rs` has tests for the shared cache, for an incremental projection that must equal a full reload, and for three writers racing.
- A test that opens a `Notebook` in a temporary folder removes the folder at the end.
