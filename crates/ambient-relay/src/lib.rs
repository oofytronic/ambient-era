//! An in-memory mailbox store and the spec-04 operations on it.
//!
//! The binary binds `127.0.0.1` and speaks cleartext WebSocket. That listener
//! is the owner's own machine. A relay other people can reach has to be TLS.
//! This process does not parse blobs.

use std::collections::BTreeMap;
use std::fmt;
use std::io::ErrorKind;
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use ambient_transport::{Item, Request, Response, decode_request, encode_response};
use tungstenite::Message;

/// Four mebibytes, from spec 04.
pub const MAX_BLOB: usize = 4_194_304;

/// What a mailbox operation or the listener can refuse.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelayError {
    /// The blob is over the limit.
    TooLarge,
    /// The listener could not bind or accept.
    Io,
}

impl fmt::Display for RelayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => formatter.write_str("too_large"),
            Self::Io => formatter.write_str("relay io"),
        }
    }
}

impl std::error::Error for RelayError {}

struct Slot {
    next: u64,
    items: Vec<(u64, Vec<u8>)>,
}

/// Mailboxes keyed by the 32-byte id. Possession of the id is the only capability.
#[derive(Default)]
pub struct Relay {
    mailboxes: BTreeMap<[u8; 32], Slot>,
}

impl Relay {
    /// Empty relay.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply one JSON line and return the JSON line to write back.
    ///
    /// When the line was a stored put, `posted` is that mailbox and cursor.
    #[must_use]
    pub fn handle_line(&mut self, line: &str) -> (String, Option<([u8; 32], Item)>) {
        let Some(request) = decode_request(line.trim()) else {
            return (
                encode_response(&Response::Err {
                    code: "bad_request".to_owned(),
                }),
                None,
            );
        };
        match request {
            Request::Put { mailbox, blob } => self.put(mailbox, blob),
            Request::Get {
                mailbox,
                since,
                limit,
            } => (
                encode_response(&Response::Blobs {
                    items: self.get(mailbox, since, limit),
                }),
                None,
            ),
            Request::Delete { mailbox, up_to } => {
                self.delete(mailbox, up_to);
                (encode_response(&Response::Ok { cursor: None }), None)
            }
            Request::Subscribe { mailbox, since } => (
                encode_response(&Response::Blobs {
                    items: self.get(mailbox, since, 500),
                }),
                None,
            ),
        }
    }

    fn put(&mut self, mailbox: [u8; 32], blob: Vec<u8>) -> (String, Option<([u8; 32], Item)>) {
        if blob.len() > MAX_BLOB {
            return (
                encode_response(&Response::Err {
                    code: "too_large".to_owned(),
                }),
                None,
            );
        }
        let slot = self.mailboxes.entry(mailbox).or_insert(Slot {
            next: 1,
            items: Vec::new(),
        });
        let cursor = slot.next;
        slot.next = slot.next.saturating_add(1);
        slot.items.push((cursor, blob.clone()));
        let item = Item { cursor, blob };
        (
            encode_response(&Response::Ok {
                cursor: Some(cursor),
            }),
            Some((mailbox, item)),
        )
    }

    fn get(&self, mailbox: [u8; 32], since: u64, limit: u64) -> Vec<Item> {
        let cap = usize::try_from(limit.clamp(1, 500)).unwrap_or(500);
        let Some(slot) = self.mailboxes.get(&mailbox) else {
            return Vec::new();
        };
        slot.items
            .iter()
            .filter(|(cursor, _)| *cursor > since)
            .take(cap)
            .map(|(cursor, blob)| Item {
                cursor: *cursor,
                blob: blob.clone(),
            })
            .collect()
    }

    fn delete(&mut self, mailbox: [u8; 32], up_to: u64) {
        if let Some(slot) = self.mailboxes.get_mut(&mailbox) {
            slot.items.retain(|(cursor, _)| *cursor > up_to);
        }
    }
}

struct Sub {
    mailbox: [u8; 32],
    tx: Sender<String>,
}

struct Hub {
    relay: Mutex<Relay>,
    subs: Mutex<Vec<Sub>>,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Bind `addr` and serve until the process stops.
///
/// # Errors
///
/// Returns [`RelayError::Io`] when the address cannot be bound.
pub fn serve(addr: &str) -> Result<(), RelayError> {
    let listener = TcpListener::bind(addr).map_err(|_| RelayError::Io)?;
    serve_listener(&listener)
}

/// Accept on a listener that is already bound. Tests use port 0.
///
/// # Errors
///
/// Returns [`RelayError::Io`] when accept fails.
pub fn serve_listener(listener: &TcpListener) -> Result<(), RelayError> {
    let hub = Arc::new(Hub {
        relay: Mutex::new(Relay::new()),
        subs: Mutex::new(Vec::new()),
    });
    for stream in listener.incoming() {
        let stream = stream.map_err(|_| RelayError::Io)?;
        let hub = Arc::clone(&hub);
        thread::spawn(move || {
            let _ = connection(stream, &hub);
        });
    }
    Ok(())
}

fn connection(stream: TcpStream, hub: &Arc<Hub>) -> Result<(), RelayError> {
    let mut socket = tungstenite::accept(stream).map_err(|_| RelayError::Io)?;
    socket
        .get_ref()
        .set_read_timeout(Some(Duration::from_millis(200)))
        .map_err(|_| RelayError::Io)?;
    let (tx, rx) = mpsc::channel::<String>();
    loop {
        while let Ok(pushed) = rx.try_recv() {
            if socket.send(Message::Text(pushed.into())).is_err() {
                return Ok(());
            }
        }
        match socket.read() {
            Ok(Message::Text(text)) => {
                for line in text.lines() {
                    if line.is_empty() {
                        continue;
                    }
                    let (reply, posted) = lock(&hub.relay).handle_line(line);
                    if socket.send(Message::Text(reply.into())).is_err() {
                        return Ok(());
                    }
                    if let Some(Request::Subscribe { mailbox, .. }) = decode_request(line) {
                        lock(&hub.subs).push(Sub {
                            mailbox,
                            tx: tx.clone(),
                        });
                    }
                    if let Some((mailbox, item)) = posted {
                        let line = encode_response(&Response::Blobs { items: vec![item] });
                        lock(&hub.subs).retain(|sub| {
                            sub.mailbox != mailbox || sub.tx.send(line.clone()).is_ok()
                        });
                    }
                }
            }
            Ok(Message::Close(_)) => return Ok(()),
            Ok(_) => {}
            Err(tungstenite::Error::Io(error))
                if error.kind() == ErrorKind::WouldBlock || error.kind() == ErrorKind::TimedOut => {
            }
            Err(_) => return Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Relay;
    use ambient_transport::{Request, Response, decode_response, encode_request};

    #[test]
    fn put_get_and_unknown_mailbox() {
        let mut relay = Relay::new();
        let mailbox = [9_u8; 32];
        let (line, posted) = relay.handle_line(&encode_request(&Request::Put {
            mailbox,
            blob: b"sealed".to_vec(),
        }));
        let Response::Ok { cursor: Some(1) } = decode_response(&line).expect("ok") else {
            panic!("cursor");
        };
        assert!(posted.is_some());
        let (line, _) = relay.handle_line(&encode_request(&Request::Get {
            mailbox,
            since: 0,
            limit: 10,
        }));
        let Response::Blobs { items } = decode_response(&line).expect("blobs") else {
            panic!("blobs");
        };
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].blob, b"sealed");
        let (line, _) = relay.handle_line(&encode_request(&Request::Get {
            mailbox: [1; 32],
            since: 0,
            limit: 10,
        }));
        let Response::Blobs { items } = decode_response(&line).expect("empty") else {
            panic!("empty");
        };
        assert!(items.is_empty());
    }

    #[test]
    fn oversized_blob_is_not_stored() {
        let mut relay = Relay::new();
        let (line, posted) = relay.handle_line(&encode_request(&Request::Put {
            mailbox: [2; 32],
            blob: vec![0; super::MAX_BLOB + 1],
        }));
        assert!(posted.is_none());
        let Response::Err { code } = decode_response(&line).expect("err") else {
            panic!("err");
        };
        assert_eq!(code, "too_large");
    }
}
