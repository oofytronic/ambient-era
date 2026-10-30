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
    if heard.trim() != "hello" {
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
