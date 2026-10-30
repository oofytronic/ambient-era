//! Loopback relay. Cleartext, bound to 127.0.0.1. Not a public deployment.

use std::io::Write as _;

fn main() {
    let port = std::env::args().nth(1).unwrap_or_else(|| "8787".to_owned());
    let addr = format!("127.0.0.1:{port}");
    if let Err(err) = ambient_relay::serve(&addr) {
        let _ = writeln!(std::io::stderr(), "{err}");
        std::process::exit(1);
    }
}
