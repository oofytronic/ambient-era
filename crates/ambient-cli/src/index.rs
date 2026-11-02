//! SQLCipher FTS5 index for the native client.
//!
//! The sealed notes log stays the source of truth. This file is a projection.
//! It is not uploaded. The protocol store crate does not link SQLCipher.

use std::path::Path;

use rusqlite::{Connection, params};

use crate::notes::CliError;

/// Note ids whose indexed text matches `query` as FTS5 tokens.
///
/// # Errors
///
/// Returns [`CliError`] when the database cannot be opened or the query cannot run.
pub(crate) fn match_ids(
    dir: &Path,
    key: &[u8; 32],
    rows: &[(String, String, String)],
    query: &str,
) -> Result<Vec<String>, CliError> {
    let Some(match_query) = fts_query(query) else {
        return Err(CliError::message("query has no words"));
    };
    let path = dir.join("search.db");
    let conn = open_db(&path, key)?;
    conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(id UNINDEXED, title, body, tokenize = 'unicode61');
         DELETE FROM notes_fts;",
    )
    .map_err(|_| CliError::message("could not build search index"))?;
    {
        let mut insert = conn
            .prepare("INSERT INTO notes_fts (id, title, body) VALUES (?1, ?2, ?3)")
            .map_err(|_| CliError::message("could not build search index"))?;
        for (id, title, body) in rows {
            insert
                .execute(params![id, title, body])
                .map_err(|_| CliError::message("could not build search index"))?;
        }
    }
    let mut select = conn
        .prepare("SELECT id FROM notes_fts WHERE notes_fts MATCH ?1")
        .map_err(|_| CliError::message("could not search notes"))?;
    let ids = select
        .query_map(params![match_query], |row| row.get(0))
        .map_err(|_| CliError::message("could not search notes"))?;
    let mut out = Vec::new();
    for id in ids {
        out.push(id.map_err(|_| CliError::message("could not search notes"))?);
    }
    Ok(out)
}

fn open_db(path: &Path, key: &[u8; 32]) -> Result<Connection, CliError> {
    match try_open(path, key) {
        Ok(conn) => Ok(conn),
        Err(err) if path.exists() => {
            std::fs::remove_file(path).map_err(|_| err)?;
            try_open(path, key)
        }
        Err(err) => Err(err),
    }
}

fn try_open(path: &Path, key: &[u8; 32]) -> Result<Connection, CliError> {
    let conn = Connection::open(path).map_err(|_| CliError::message("could not open search index"))?;
    let pragma = format!("PRAGMA key = \"x'{}'\";", hex_key(key));
    conn.execute_batch(&pragma)
        .map_err(|_| CliError::message("could not open search index"))?;
    conn.execute_batch("SELECT count(*) FROM sqlite_master;")
        .map_err(|_| CliError::message("could not open search index"))?;
    Ok(conn)
}

fn hex_key(key: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in key {
        out.push(hex_nibble(byte >> 4));
        out.push(hex_nibble(byte & 0x0f));
    }
    out
}

fn hex_nibble(nibble: u8) -> char {
    match nibble {
        0..=9 => char::from(b'0' + nibble),
        _ => char::from(b'a' + nibble.saturating_sub(10)),
    }
}

/// Quote each word so FTS5 cannot treat the user's text as an operator.
fn fts_query(raw: &str) -> Option<String> {
    let mut parts = Vec::new();
    let mut word = String::new();
    for ch in raw.chars() {
        if ch.is_alphanumeric() {
            word.push(ch);
        } else if !word.is_empty() {
            parts.push(format!("\"{word}\""));
            word.clear();
        }
    }
    if !word.is_empty() {
        parts.push(format!("\"{word}\""));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" AND "))
    }
}
