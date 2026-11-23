//! Two folders on one machine, through a loopback relay.

use std::io::Cursor;
use std::net::TcpListener;
use std::thread;

use ambient_cli::run;

#[test]
fn note_reaches_the_second_device() -> Result<(), String> {
    let port = spawn_relay()?;
    let url = format!("ws://127.0.0.1:{port}");
    let root = std::env::temp_dir().join(format!("ambient-sync-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let owner = root.join("owner");
    let other = root.join("other");
    let owner_s = owner.to_string_lossy().to_string();
    let other_s = other.to_string_lossy().to_string();
    run_ok(&["init", &owner_s], "")?;
    run_ok(&["add", &owner_s, "Market"], "buy milk")?;
    let offer = run_ok(&["device", &other_s], "")?;
    run_ok(&["pair", &owner_s, offer.trim()], "")?;
    run_ok(&["sync", &owner_s, &url], "")?;
    run_ok(&["sync", &other_s, &url], "")?;
    let listed = run_ok(&["list", &other_s], "")?;
    if !listed.contains("Market") {
        return Err(format!("second device did not see the note: {listed}"));
    }
    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

#[test]
fn edited_note_reaches_the_second_device() -> Result<(), String> {
    let port = spawn_relay()?;
    let url = format!("ws://127.0.0.1:{port}");
    let root = std::env::temp_dir().join(format!("ambient-edit-sync-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let owner = root.join("owner");
    let other = root.join("other");
    let owner_s = owner.to_string_lossy().to_string();
    let other_s = other.to_string_lossy().to_string();
    run_ok(&["init", &owner_s], "")?;
    run_ok(&["add", &owner_s, "Market"], "buy milk")?;
    let offer = run_ok(&["device", &other_s], "")?;
    run_ok(&["pair", &owner_s, offer.trim()], "")?;
    run_ok(&["sync", &owner_s, &url], "")?;
    run_ok(&["sync", &other_s, &url], "")?;
    let listed = run_ok(&["list", &owner_s], "")?;
    let id = listed
        .lines()
        .find(|line| line.contains("Market"))
        .and_then(|line| line.split_whitespace().next())
        .ok_or_else(|| format!("missing note id: {listed}"))?;
    run_ok(&["edit", &owner_s, id, "Market"], "buy milk and bread")?;
    run_ok(&["add", &owner_s, "Later"], "eggs")?;
    run_ok(&["sync", &owner_s, &url], "")?;
    run_ok(&["sync", &other_s, &url], "")?;
    let book = ambient_cli::Notebook::open(&other).map_err(|err| err.to_string())?;
    let notes = book.list().map_err(|err| err.to_string())?;
    if !notes.iter().any(|note| note.title == "Market")
        || !notes.iter().any(|note| note.title == "Later")
    {
        return Err(format!("second device missed a note: {notes:?}"));
    }
    let shown = book.show(id).map_err(|err| err.to_string())?;
    let body = shown.heads.first().map_or("", |head| head.body.as_str());
    if body != "buy milk and bread" {
        return Err(format!("edit did not arrive: {body}"));
    }
    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

#[test]
fn one_message_crosses() -> Result<(), String> {
    let port = spawn_relay()?;
    let url = format!("ws://127.0.0.1:{port}");
    let root = std::env::temp_dir().join(format!("ambient-chat-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let alice = root.join("alice");
    let bob = root.join("bob");
    let alice_s = alice.to_string_lossy().to_string();
    let bob_s = bob.to_string_lossy().to_string();
    run_ok(&["init", &alice_s], "")?;
    run_ok(&["init", &bob_s], "")?;
    let bob_card = run_ok(&["card", &bob_s], "")?;
    run_ok(&["talk", &alice_s, bob_card.trim()], "")?;
    run_ok(&["say", &alice_s, "hello"], "")?;
    run_ok(&["sync", &alice_s, &url], "")?;
    run_ok(&["card", &bob_s], "")?;
    run_ok(&["sync", &bob_s, &url], "")?;
    let heard = run_ok(&["hear", &bob_s], "")?;
    if !heard.trim().ends_with("hello") {
        return Err(format!("bob heard {heard}"));
    }
    run_ok(&["say", &bob_s, "hi"], "")?;
    run_ok(&["sync", &bob_s, &url], "")?;
    run_ok(&["sync", &alice_s, &url], "")?;
    let heard = run_ok(&["hear", &alice_s], "")?;
    if !heard.contains("hello") || !heard.contains("hi") {
        return Err(format!("alice heard {heard}"));
    }
    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

#[test]
fn paired_folder_lists_nothing_before_sync() -> Result<(), String> {
    let root = std::env::temp_dir().join(format!("ambient-empty-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let other = root.join("other");
    ambient_cli::device_offer(&other, "other-computer").map_err(|err| err.to_string())?;
    let book = ambient_cli::Notebook::open(&other).map_err(|err| err.to_string())?;
    if book.holds_root() {
        return Err("paired folder must not hold the root".to_owned());
    }
    let listed = book.list().map_err(|err| err.to_string())?;
    if !listed.is_empty() {
        return Err("paired folder had notes before sync".to_owned());
    }
    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

#[test]
fn restored_folder_writes_back() -> Result<(), String> {
    let port = spawn_relay()?;
    let url = format!("ws://127.0.0.1:{port}");
    let root = std::env::temp_dir().join(format!("ambient-restore-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let owner = root.join("owner");
    let restored = root.join("restored");
    let owner_s = owner.to_string_lossy().to_string();
    let restored_s = restored.to_string_lossy().to_string();
    run_ok(&["init", &owner_s], "")?;
    run_ok(&["add", &owner_s, "Market"], "buy milk")?;
    run_ok(&["sync", &owner_s, &url], "")?;
    let phrase = ambient_cli::recovery_phrase(&owner).map_err(|err| err.to_string())?;
    ambient_cli::restore_folder(&restored, &phrase, &url).map_err(|err| err.to_string())?;
    let book = ambient_cli::Notebook::open(&restored).map_err(|err| err.to_string())?;
    if !book.holds_root() {
        return Err("restored folder must hold the root".to_owned());
    }
    let devices = book.devices().map_err(|err| err.to_string())?;
    if !devices
        .iter()
        .any(|device| device.current && !device.revoked)
    {
        return Err(format!("restored device was not enrolled: {devices:?}"));
    }
    let listed = book.list().map_err(|err| err.to_string())?;
    if !listed.iter().any(|note| note.title == "Market") {
        return Err(format!("restore missed the note: {listed:?}"));
    }
    book.add("Later", "from the new device")
        .map_err(|err| err.to_string())?;
    run_ok(&["sync", &restored_s, &url], "")?;
    run_ok(&["sync", &owner_s, &url], "")?;
    let listed = run_ok(&["list", &owner_s], "")?;
    if !listed.contains("Later") {
        return Err(format!(
            "owner did not see the restored device's note: {listed}"
        ));
    }
    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

#[test]
fn a_dead_relay_is_skipped_for_the_backup() -> Result<(), String> {
    let live = spawn_relay()?;
    let url = format!("ws://127.0.0.1:{live}");
    let root = std::env::temp_dir().join(format!("ambient-failover-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let owner = root.join("owner");
    let other = root.join("other");
    let owner_s = owner.to_string_lossy().to_string();
    let other_s = other.to_string_lossy().to_string();
    run_ok(&["init", &owner_s], "")?;
    run_ok(&["add", &owner_s, "Market"], "buy milk")?;
    let offer = run_ok(&["device", &other_s], "")?;
    run_ok(&["pair", &owner_s, offer.trim()], "")?;
    ambient_cli::set_net_setting(
        &owner,
        &ambient_cli::NetSetting {
            mode: ambient_cli::NetMode::Pin,
            pin: "ws://127.0.0.1:1".to_owned(),
            backup: url.clone(),
            choice: String::new(),
        },
    )
    .map_err(|err| err.to_string())?;
    ambient_cli::sync_notebook(&owner).map_err(|err| err.to_string())?;
    run_ok(&["sync", &other_s, &url], "")?;
    let listed = run_ok(&["list", &other_s], "")?;
    if !listed.contains("Market") {
        return Err(format!("backup relay did not carry the note: {listed}"));
    }
    if live == u16::MAX {
        return Err("no spare port".to_owned());
    }
    ambient_cli::set_net_setting(
        &owner,
        &ambient_cli::NetSetting {
            mode: ambient_cli::NetMode::Pin,
            pin: format!("ws://127.0.0.1:{}", live.saturating_add(1)),
            backup: String::new(),
            choice: String::new(),
        },
    )
    .map_err(|err| err.to_string())?;
    if !ambient_cli::has_unsent(&owner).map_err(|err| err.to_string())? {
        return Err("a new relay was treated as already sent".to_owned());
    }
    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

#[test]
fn cleartext_off_this_computer_is_refused() {
    assert!(ambient_cli::check_relay_url("ws://example.com/mailbox").is_err());
    assert!(ambient_cli::check_relay_url("wss://example.com/mailbox").is_ok());
    assert!(ambient_cli::check_relay_url("ws://127.0.0.1:8787").is_ok());
}

/// A sync reads a conversation, waits on the relay, and writes it back. A message said in
/// that wait must not be overwritten, and the messages the relay delivered must not be either.
#[test]
fn saying_while_a_sync_waits_loses_no_message() -> Result<(), String> {
    let port = spawn_relay()?;
    let url = format!("ws://127.0.0.1:{port}");
    let root = std::env::temp_dir().join(format!("ambient-chat-race-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let alice = root.join("alice");
    let bob = root.join("bob");
    let alice_s = alice.to_string_lossy().to_string();
    let bob_s = bob.to_string_lossy().to_string();
    run_ok(&["init", &alice_s], "")?;
    run_ok(&["init", &bob_s], "")?;
    let bob_card = run_ok(&["card", &bob_s], "")?;
    run_ok(&["talk", &alice_s, bob_card.trim()], "")?;
    run_ok(&["say", &alice_s, "hello"], "")?;
    run_ok(&["sync", &alice_s, &url], "")?;
    run_ok(&["card", &bob_s], "")?;
    run_ok(&["sync", &bob_s, &url], "")?;
    for n in 0..6 {
        run_ok(&["say", &bob_s, &format!("from-bob-{n}")], "")?;
    }
    run_ok(&["sync", &bob_s, &url], "")?;

    let syncing = {
        let (alice_s, url) = (alice_s.clone(), url.clone());
        thread::spawn(move || -> Result<(), String> {
            for _ in 0..10 {
                run_ok(&["sync", &alice_s, &url], "")?;
            }
            Ok(())
        })
    };
    let saying = {
        let alice_s = alice_s.clone();
        thread::spawn(move || -> Result<(), String> {
            for n in 0..14 {
                run_ok(&["say", &alice_s, &format!("from-alice-{n}")], "")?;
            }
            Ok(())
        })
    };
    syncing
        .join()
        .map_err(|_| "sync thread panicked".to_owned())??;
    saying
        .join()
        .map_err(|_| "say thread panicked".to_owned())??;

    run_ok(&["sync", &alice_s, &url], "")?;
    run_ok(&["sync", &bob_s, &url], "")?;
    let alice_heard = run_ok(&["hear", &alice_s], "")?;
    let bob_heard = run_ok(&["hear", &bob_s], "")?;
    for n in 0..6 {
        let want = format!("from-bob-{n}");
        if !alice_heard.contains(&want) {
            return Err(format!("alice lost {want}: {alice_heard}"));
        }
    }
    for n in 0..14 {
        let want = format!("from-alice-{n}");
        if !alice_heard.contains(&want) {
            return Err(format!("alice lost her own {want}: {alice_heard}"));
        }
        if !bob_heard.contains(&want) {
            return Err(format!("bob never got {want}: {bob_heard}"));
        }
    }
    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

/// The same for the notes file: a note written while a sync waits on the relay is kept, and so
/// are the notes the relay delivered.
#[test]
fn adding_while_a_sync_waits_loses_no_note() -> Result<(), String> {
    let port = spawn_relay()?;
    let url = format!("ws://127.0.0.1:{port}");
    let root = std::env::temp_dir().join(format!("ambient-notes-race-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let owner = root.join("owner");
    let other = root.join("other");
    let owner_s = owner.to_string_lossy().to_string();
    let other_s = other.to_string_lossy().to_string();
    run_ok(&["init", &owner_s], "")?;
    run_ok(&["add", &owner_s, "Market"], "buy milk")?;
    let offer = run_ok(&["device", &other_s], "")?;
    run_ok(&["pair", &owner_s, offer.trim()], "")?;
    run_ok(&["sync", &owner_s, &url], "")?;
    run_ok(&["sync", &other_s, &url], "")?;
    for n in 0..5 {
        run_ok(&["add", &other_s, &format!("other-{n}")], "text")?;
    }
    run_ok(&["sync", &other_s, &url], "")?;

    let syncing = {
        let (owner_s, url) = (owner_s.clone(), url.clone());
        thread::spawn(move || -> Result<(), String> {
            for _ in 0..10 {
                run_ok(&["sync", &owner_s, &url], "")?;
            }
            Ok(())
        })
    };
    let adding = {
        let owner_s = owner_s.clone();
        thread::spawn(move || -> Result<(), String> {
            for n in 0..10 {
                run_ok(&["add", &owner_s, &format!("owner-{n}")], "text")?;
            }
            Ok(())
        })
    };
    syncing
        .join()
        .map_err(|_| "sync thread panicked".to_owned())??;
    adding
        .join()
        .map_err(|_| "add thread panicked".to_owned())??;
    run_ok(&["sync", &owner_s, &url], "")?;

    let listed = run_ok(&["list", &owner_s], "")?;
    for n in 0..5 {
        let want = format!("other-{n}");
        if !listed.contains(&want) {
            return Err(format!("the owner lost {want}: {listed}"));
        }
    }
    for n in 0..10 {
        let want = format!("owner-{n}");
        if !listed.contains(&want) {
            return Err(format!("the owner lost its own {want}: {listed}"));
        }
    }
    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

#[test]
fn a_blocked_card_cannot_start_a_conversation() -> Result<(), String> {
    let root = std::env::temp_dir().join(format!("ambient-block-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let alice = root.join("alice");
    let bob = root.join("bob");
    let alice_s = alice.to_string_lossy().to_string();
    let bob_s = bob.to_string_lossy().to_string();
    run_ok(&["init", &alice_s], "")?;
    run_ok(&["init", &bob_s], "")?;
    let bob_card = run_ok(&["card", &bob_s], "")?;
    run_ok(&["block", &alice_s, bob_card.trim()], "")?;
    let mut output = Vec::new();
    let text = run(
        &mut ["talk", &alice_s, bob_card.trim()]
            .into_iter()
            .map(str::to_owned),
        &mut Cursor::new("".as_bytes()),
        &mut output,
    )
    .err()
    .map(|err| err.to_string())
    .unwrap_or_default();
    if text != "that person is blocked" {
        return Err(format!("block did not hold: {text}"));
    }
    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

#[test]
fn a_cited_file_reaches_the_other_folder() -> Result<(), String> {
    let port = spawn_relay()?;
    let url = format!("ws://127.0.0.1:{port}");
    let root = std::env::temp_dir().join(format!("ambient-file-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let alice = root.join("alice");
    let bob = root.join("bob");
    run_ok(&["init", &alice.to_string_lossy()], "")?;
    run_ok(&["init", &bob.to_string_lossy()], "")?;
    let bob_card = run_ok(&["card", &bob.to_string_lossy()], "")?;
    let id =
        ambient_cli::start_conversation(&alice, bob_card.trim()).map_err(|err| err.to_string())?;
    let file_id = "0123456789abcdef";
    let folder = alice.join("chat-files").join(&id);
    std::fs::create_dir_all(&folder).map_err(|err| err.to_string())?;
    std::fs::write(folder.join(format!("{file_id}-note.txt")), b"hello file")
        .map_err(|err| err.to_string())?;
    ambient_cli::send_chat(&alice, &format!("![note.txt](aerodash-file:{file_id})"))
        .map_err(|err| err.to_string())?;
    ambient_cli::sync_folder(&alice, &url).map_err(|err| err.to_string())?;
    ambient_cli::sync_folder(&bob, &url).map_err(|err| err.to_string())?;
    let landed = bob
        .join("chat-files")
        .join(&id)
        .join(format!("{file_id}-note.txt"));
    let bytes = std::fs::read(&landed).map_err(|err| format!("{err} at {}", landed.display()))?;
    if bytes != b"hello file" {
        return Err(format!("file bytes were {bytes:?}"));
    }
    let text = run_ok(&["hear", &bob.to_string_lossy()], "")?;
    if !text.contains("aerodash-file:0123456789abcdef") {
        return Err(format!("the cite did not arrive: {text}"));
    }
    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}

fn spawn_relay() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|err| err.to_string())?;
    let port = listener.local_addr().map_err(|err| err.to_string())?.port();
    thread::spawn(move || {
        let _ = ambient_relay::serve_listener(&listener);
    });
    Ok(port)
}

fn run_ok(args: &[&str], stdin: &str) -> Result<String, String> {
    let owned: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    let mut input = Cursor::new(stdin.as_bytes());
    let mut output = Vec::new();
    run(&mut owned.into_iter(), &mut input, &mut output).map_err(|err| err.to_string())?;
    String::from_utf8(output).map_err(|err| err.to_string())
}
