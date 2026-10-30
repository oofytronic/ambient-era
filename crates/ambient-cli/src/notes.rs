//! Command parsing for the `ambient` binary.

use std::io::{Read, Write};
use std::path::Path;

use crate::session::Notebook;

/// A failure whose text is safe to print. It does not include note bodies.
#[derive(Debug)]
pub struct CliError {
    message: String,
}

impl CliError {
    pub(crate) fn message(text: impl Into<String>) -> Self {
        Self {
            message: text.into(),
        }
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for CliError {}

/// Run one notes command. `args` is everything after the program name.
///
/// # Errors
///
/// Returns a message for unknown commands, a bad directory, or a rejected event.
pub fn run(
    args: &mut impl Iterator<Item = String>,
    stdin: &mut impl Read,
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let command = args.next().ok_or_else(|| {
        CliError::message(
            "usage: ambient <init|add|edit|list|search|show|device|pair|sync|card|talk|say|hear> ...",
        )
    })?;
    match command.as_str() {
        "init" => init(args, stdout),
        "add" => add(args, stdin, stdout),
        "edit" => edit(args, stdin, stdout),
        "list" => list(args, stdout),
        "search" => search(args, stdout),
        "show" => show(args, stdout),
        "device" => device_cmd(args, stdout),
        "pair" => pair_cmd(args, stdout),
        "sync" => sync_cmd(args, stdout),
        "card" => card_cmd(args, stdout),
        "talk" => talk_cmd(args, stdout),
        "say" => say_cmd(args, stdout),
        "hear" => hear_cmd(args, stdout),
        _ => Err(CliError::message("unknown command")),
    }
}

fn init(args: &mut impl Iterator<Item = String>, stdout: &mut impl Write) -> Result<(), CliError> {
    let dir = args
        .next()
        .ok_or_else(|| CliError::message("usage: ambient init <dir>"))?;
    let _ = Notebook::create(Path::new(&dir))?;
    writeln!(stdout, "initialized {dir}").map_err(|_| CliError::message("stdout"))?;
    Ok(())
}

fn add(
    args: &mut impl Iterator<Item = String>,
    stdin: &mut impl Read,
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let dir = args
        .next()
        .ok_or_else(|| CliError::message("usage: ambient add <dir> <title>"))?;
    let title = args
        .next()
        .ok_or_else(|| CliError::message("usage: ambient add <dir> <title>"))?;
    let body = read_stdin(stdin)?;
    let id = Notebook::open(Path::new(&dir))?.add(&title, &body)?;
    writeln!(stdout, "{id}").map_err(|_| CliError::message("stdout"))?;
    Ok(())
}

fn edit(
    args: &mut impl Iterator<Item = String>,
    stdin: &mut impl Read,
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let dir = args
        .next()
        .ok_or_else(|| CliError::message("usage: ambient edit <dir> <note-id> <title>"))?;
    let note_hex = args
        .next()
        .ok_or_else(|| CliError::message("usage: ambient edit <dir> <note-id> <title>"))?;
    let title = args
        .next()
        .ok_or_else(|| CliError::message("usage: ambient edit <dir> <note-id> <title>"))?;
    let body = read_stdin(stdin)?;
    Notebook::open(Path::new(&dir))?.edit(&note_hex, &title, &body)?;
    writeln!(stdout, "updated {note_hex}").map_err(|_| CliError::message("stdout"))?;
    Ok(())
}

fn list(args: &mut impl Iterator<Item = String>, stdout: &mut impl Write) -> Result<(), CliError> {
    let dir = args
        .next()
        .ok_or_else(|| CliError::message("usage: ambient list <dir>"))?;
    for note in Notebook::open(Path::new(&dir))?.list()? {
        let mark = if note.conflict { " conflict" } else { "" };
        writeln!(stdout, "{} {}{mark}", note.id, note.title)
            .map_err(|_| CliError::message("stdout"))?;
    }
    Ok(())
}

fn search(
    args: &mut impl Iterator<Item = String>,
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let dir = args
        .next()
        .ok_or_else(|| CliError::message("usage: ambient search <dir> <query>"))?;
    let query = args
        .next()
        .ok_or_else(|| CliError::message("usage: ambient search <dir> <query>"))?;
    for note in Notebook::open(Path::new(&dir))?.search(&query)? {
        writeln!(stdout, "{} {}", note.id, note.title).map_err(|_| CliError::message("stdout"))?;
    }
    Ok(())
}

fn hear_cmd(
    args: &mut impl Iterator<Item = String>,
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let dir = args
        .next()
        .ok_or_else(|| CliError::message("usage: ambient hear <dir>"))?;
    let text = crate::sync::hear(Path::new(&dir))?;
    write!(stdout, "{text}").map_err(|_| CliError::message("stdout"))?;
    Ok(())
}

fn say_cmd(
    args: &mut impl Iterator<Item = String>,
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let dir = args
        .next()
        .ok_or_else(|| CliError::message("usage: ambient say <dir> <text>"))?;
    let text = args
        .next()
        .ok_or_else(|| CliError::message("usage: ambient say <dir> <text>"))?;
    crate::sync::say(Path::new(&dir), &text)?;
    writeln!(stdout, "sent").map_err(|_| CliError::message("stdout"))?;
    Ok(())
}

fn talk_cmd(
    args: &mut impl Iterator<Item = String>,
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let dir = args
        .next()
        .ok_or_else(|| CliError::message("usage: ambient talk <dir> <card>"))?;
    let card = args
        .next()
        .ok_or_else(|| CliError::message("usage: ambient talk <dir> <card>"))?;
    crate::sync::talk(Path::new(&dir), &card)?;
    writeln!(stdout, "conversation").map_err(|_| CliError::message("stdout"))?;
    Ok(())
}

fn card_cmd(
    args: &mut impl Iterator<Item = String>,
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let dir = args
        .next()
        .ok_or_else(|| CliError::message("usage: ambient card <dir>"))?;
    let text = crate::sync::card(Path::new(&dir))?;
    writeln!(stdout, "{text}").map_err(|_| CliError::message("stdout"))?;
    Ok(())
}

fn sync_cmd(
    args: &mut impl Iterator<Item = String>,
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let dir = args
        .next()
        .ok_or_else(|| CliError::message("usage: ambient sync <dir> <relay-url>"))?;
    let url = args
        .next()
        .ok_or_else(|| CliError::message("usage: ambient sync <dir> <relay-url>"))?;
    let text = crate::sync::sync(Path::new(&dir), &url)?;
    writeln!(stdout, "{text}").map_err(|_| CliError::message("stdout"))?;
    Ok(())
}

fn pair_cmd(
    args: &mut impl Iterator<Item = String>,
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let dir = args
        .next()
        .ok_or_else(|| CliError::message("usage: ambient pair <dir> <offer> [label]"))?;
    let offer = args
        .next()
        .ok_or_else(|| CliError::message("usage: ambient pair <dir> <offer> [label]"))?;
    let label = args.next().unwrap_or_else(|| "other-computer".to_owned());
    Notebook::open(Path::new(&dir))?.pair_device(&offer, &label)?;
    writeln!(stdout, "paired").map_err(|_| CliError::message("stdout"))?;
    Ok(())
}

fn device_cmd(
    args: &mut impl Iterator<Item = String>,
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let dir = args
        .next()
        .ok_or_else(|| CliError::message("usage: ambient device <dir> [label]"))?;
    let label = args.next().unwrap_or_else(|| "other-computer".to_owned());
    let offer = crate::sync::device(Path::new(&dir), &label)?;
    writeln!(stdout, "{offer}").map_err(|_| CliError::message("stdout"))?;
    Ok(())
}

fn show(args: &mut impl Iterator<Item = String>, stdout: &mut impl Write) -> Result<(), CliError> {
    let dir = args
        .next()
        .ok_or_else(|| CliError::message("usage: ambient show <dir> <note-id>"))?;
    let note_hex = args
        .next()
        .ok_or_else(|| CliError::message("usage: ambient show <dir> <note-id>"))?;
    let note = Notebook::open(Path::new(&dir))?.show(&note_hex)?;
    for (index, head) in note.heads.iter().enumerate() {
        if note.conflict {
            writeln!(stdout, "# conflict {}", index + 1)
                .map_err(|_| CliError::message("stdout"))?;
        }
        writeln!(stdout, "{}", head.body).map_err(|_| CliError::message("stdout"))?;
    }
    Ok(())
}

fn read_stdin(stdin: &mut impl Read) -> Result<String, CliError> {
    let mut body = String::new();
    stdin
        .read_to_string(&mut body)
        .map_err(|_| CliError::message("could not read stdin"))?;
    Ok(body)
}
