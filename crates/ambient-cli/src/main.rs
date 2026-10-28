//! `ambient` — offline notes.

use std::io::Write as _;

fn main() {
    let mut args = std::env::args().skip(1);
    let code = match ambient_cli::run(&mut args, &mut std::io::stdin(), &mut std::io::stdout()) {
        Ok(()) => 0,
        Err(err) => {
            let _ = writeln!(std::io::stderr(), "{err}");
            1
        }
    };
    std::process::exit(code);
}
