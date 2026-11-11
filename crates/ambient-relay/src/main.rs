//! Relay process.
//!
//! Without `--tls` it binds `127.0.0.1` and speaks cleartext. That is this computer.
//! `--tls` is how another computer is allowed to connect. The certificate is a PEM
//! the operator already has. This program does not request one.

use std::io::Write as _;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = if args.iter().any(|arg| arg == "--tls") {
        serve_tls(&args)
    } else {
        let port = args.first().map_or("8787", String::as_str);
        let dir = args.get(1).map_or("relay-data", String::as_str);
        let addr = format!("127.0.0.1:{port}");
        ambient_relay::serve(&addr, Path::new(dir))
    };
    if let Err(err) = result {
        let _ = writeln!(std::io::stderr(), "{err}");
        std::process::exit(1);
    }
}

fn serve_tls(args: &[String]) -> Result<(), ambient_relay::RelayError> {
    let mut cert = None;
    let mut key = None;
    let mut store = "relay-data".to_owned();
    let mut bind = "0.0.0.0:443".to_owned();
    let mut index = 0;
    while index < args.len() {
        let arg = args.get(index).map_or("", String::as_str);
        match arg {
            "--tls" => {
                cert = args.get(index + 1).cloned();
                key = args.get(index + 2).cloned();
                index = index.saturating_add(3);
            }
            "--store" => {
                if let Some(value) = args.get(index + 1) {
                    store.clone_from(value);
                }
                index = index.saturating_add(2);
            }
            "--bind" => {
                if let Some(value) = args.get(index + 1) {
                    bind.clone_from(value);
                }
                index = index.saturating_add(2);
            }
            _ => index = index.saturating_add(1),
        }
    }
    let (Some(cert), Some(key)) = (cert, key) else {
        return Err(ambient_relay::RelayError::Io);
    };
    let cert_pem = std::fs::read(&cert).map_err(|_| ambient_relay::RelayError::Io)?;
    let key_pem = std::fs::read(&key).map_err(|_| ambient_relay::RelayError::Io)?;
    ambient_relay::serve_tls(&bind, Path::new(&store), &cert_pem, &key_pem)
}
