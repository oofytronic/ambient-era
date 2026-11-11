//! A mailbox store and the spec-04 operations on it.
//!
//! `Relay::new` is memory only. `Relay::open` keeps the same mailboxes in a
//! directory, so a restart still has a blob the other device has not pulled.
//! The file is not a wire format. This process does not parse blobs.
//!
//! The binary binds `127.0.0.1` and speaks cleartext WebSocket. That listener
//! is one machine. A relay other people can reach has to be TLS.

use std::collections::BTreeMap;
use std::fmt;
use std::io::ErrorKind;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use ambient_transport::{Item, Request, Response, decode_request, encode_response};
use rustls::ServerConfig;
use tungstenite::Message;

mod disk;

/// Four mebibytes, from spec 04.
pub const MAX_BLOB: usize = 4_194_304;

/// What a mailbox operation or the listener can refuse.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelayError {
    /// The blob is over the limit.
    TooLarge,
    /// The listener could not bind or accept.
    Io,
    /// The mailbox file could not be read or written. Nothing new was stored.
    Store,
}

impl fmt::Display for RelayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => formatter.write_str("too_large"),
            Self::Io => formatter.write_str("relay io"),
            Self::Store => formatter.write_str("could not keep the mailbox store"),
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
    /// Set by [`Relay::open`]. Absent means a put is not written anywhere.
    dir: Option<PathBuf>,
}

impl Relay {
    /// Empty relay. Puts are not written to disk.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Load `dir/mailboxes`, or start empty when that file is absent.
    ///
    /// A file that does not parse is [`RelayError::Store`]. It is left in place.
    ///
    /// # Errors
    ///
    /// Returns [`RelayError::Store`] when the directory or the file cannot be used.
    pub fn open(dir: &Path) -> Result<Self, RelayError> {
        fs_open(dir)
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
        let created = !self.mailboxes.contains_key(&mailbox);
        let slot = self.mailboxes.entry(mailbox).or_insert(Slot {
            next: 1,
            items: Vec::new(),
        });
        let cursor = slot.next;
        slot.next = slot.next.saturating_add(1);
        slot.items.push((cursor, blob.clone()));
        if self.persist().is_err() {
            self.undo_put(mailbox, cursor, created);
            return (
                encode_response(&Response::Err {
                    code: "unavailable".to_owned(),
                }),
                None,
            );
        }
        let item = Item { cursor, blob };
        (
            encode_response(&Response::Ok {
                cursor: Some(cursor),
            }),
            Some((mailbox, item)),
        )
    }

    fn undo_put(&mut self, mailbox: [u8; 32], cursor: u64, created: bool) {
        if created {
            self.mailboxes.remove(&mailbox);
            return;
        }
        if let Some(slot) = self.mailboxes.get_mut(&mailbox) {
            slot.items.pop();
            slot.next = cursor;
        }
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
        let Some(slot) = self.mailboxes.get(&mailbox) else {
            return;
        };
        if slot.items.iter().all(|(cursor, _)| *cursor > up_to) {
            return;
        }
        let previous = slot.items.clone();
        if let Some(slot) = self.mailboxes.get_mut(&mailbox) {
            slot.items.retain(|(cursor, _)| *cursor > up_to);
        }
        if self.persist().is_err()
            && let Some(slot) = self.mailboxes.get_mut(&mailbox)
        {
            slot.items = previous;
        }
    }

    fn persist(&self) -> Result<(), RelayError> {
        let Some(dir) = &self.dir else {
            return Ok(());
        };
        disk::save(dir, &self.mailboxes)
    }
}

fn fs_open(dir: &Path) -> Result<Relay, RelayError> {
    let mailboxes = disk::load(dir)?;
    disk::save(dir, &mailboxes)?;
    Ok(Relay {
        mailboxes,
        dir: Some(dir.to_path_buf()),
    })
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

/// Bind `addr` and serve `dir` until the process stops.
///
/// # Errors
///
/// Returns [`RelayError::Io`] when the address cannot be bound, or [`RelayError::Store`]
/// when `dir` cannot be opened.
pub fn serve(addr: &str, dir: &Path) -> Result<(), RelayError> {
    if !loopback_addr(addr) {
        return Err(RelayError::Io);
    }
    let relay = Relay::open(dir)?;
    let listener = TcpListener::bind(addr).map_err(|_| RelayError::Io)?;
    accept_blocking(&listener, relay)
}

/// Bind `addr` and serve `dir` over TLS. The certificate is the operator's PEM.
///
/// Cleartext is refused on a non-loopback address. This function is the path
/// another computer is allowed to use.
///
/// # Errors
///
/// Returns [`RelayError::Io`] when the certificate, key, or listener cannot be used,
/// or [`RelayError::Store`] when `dir` cannot be opened.
pub fn serve_tls(
    addr: &str,
    dir: &Path,
    cert_pem: &[u8],
    key_pem: &[u8],
) -> Result<(), RelayError> {
    let relay = Relay::open(dir)?;
    let listener = TcpListener::bind(addr).map_err(|_| RelayError::Io)?;
    accept_tls(&listener, relay, cert_pem, key_pem)
}

/// TLS accept on a listener that is already bound. Tests use port 0.
///
/// # Errors
///
/// Returns [`RelayError::Io`] when the certificate cannot be read or accept fails.
pub fn serve_tls_listener(
    listener: &TcpListener,
    cert_pem: &[u8],
    key_pem: &[u8],
) -> Result<(), RelayError> {
    accept_tls(listener, Relay::new(), cert_pem, key_pem)
}

/// Accept on a listener that is already bound. The mailboxes are not written.
/// Tests use this so a run does not keep another run's blobs.
///
/// # Errors
///
/// Returns [`RelayError::Io`] when accept fails.
pub fn serve_listener(listener: &TcpListener) -> Result<(), RelayError> {
    accept_blocking(listener, Relay::new())
}

/// Accept until `generation` is no longer `mine`. Mailboxes are the file in `dir`.
///
/// The listener is set non-blocking.
///
/// # Errors
///
/// Returns [`RelayError::Store`] when `dir` cannot be opened, or [`RelayError::Io`]
/// when the listener cannot be made non-blocking, or accept fails.
pub fn serve_until(
    listener: &TcpListener,
    generation: &AtomicU64,
    mine: u64,
    dir: &Path,
) -> Result<(), RelayError> {
    let relay = Relay::open(dir)?;
    listener.set_nonblocking(true).map_err(|_| RelayError::Io)?;
    let hub = hub(relay);
    loop {
        if generation.load(Ordering::SeqCst) != mine {
            return Ok(());
        }
        match listener.accept() {
            Ok((stream, _)) => {
                let _ = stream.set_nonblocking(false);
                let hub = Arc::clone(&hub);
                thread::spawn(move || {
                    let _ = connection(stream, &hub);
                });
            }
            Err(err) if err.kind() == ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(_) => return Err(RelayError::Io),
        }
    }
}

fn accept_tls(
    listener: &TcpListener,
    relay: Relay,
    cert_pem: &[u8],
    key_pem: &[u8],
) -> Result<(), RelayError> {
    let config = server_config(cert_pem, key_pem)?;
    let hub = hub(relay);
    for stream in listener.incoming() {
        let stream = stream.map_err(|_| RelayError::Io)?;
        let hub = Arc::clone(&hub);
        let config = Arc::clone(&config);
        thread::spawn(move || {
            let _ = connection_tls(stream, &config, &hub);
        });
    }
    Ok(())
}

fn server_config(cert_pem: &[u8], key_pem: &[u8]) -> Result<Arc<ServerConfig>, RelayError> {
    tls_provider();
    let mut certs = Vec::new();
    for cert in rustls_pemfile::certs(&mut std::io::BufReader::new(cert_pem)) {
        certs.push(cert.map_err(|_| RelayError::Io)?);
    }
    if certs.is_empty() {
        return Err(RelayError::Io);
    }
    let key = rustls_pemfile::private_key(&mut std::io::BufReader::new(key_pem))
        .map_err(|_| RelayError::Io)?
        .ok_or(RelayError::Io)?;
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|_| RelayError::Io)?;
    Ok(Arc::new(config))
}

fn tls_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

fn loopback_addr(addr: &str) -> bool {
    let host = addr.rsplit_once(':').map_or(addr, |(host, _)| host);
    matches!(host, "127.0.0.1" | "localhost" | "[::1]" | "::1")
}

fn accept_blocking(listener: &TcpListener, relay: Relay) -> Result<(), RelayError> {
    let hub = hub(relay);
    for stream in listener.incoming() {
        let stream = stream.map_err(|_| RelayError::Io)?;
        let hub = Arc::clone(&hub);
        thread::spawn(move || {
            let _ = connection(stream, &hub);
        });
    }
    Ok(())
}

fn hub(relay: Relay) -> Arc<Hub> {
    Arc::new(Hub {
        relay: Mutex::new(relay),
        subs: Mutex::new(Vec::new()),
    })
}

fn connection(stream: TcpStream, hub: &Arc<Hub>) -> Result<(), RelayError> {
    stream
        .set_read_timeout(Some(Duration::from_millis(200)))
        .map_err(|_| RelayError::Io)?;
    let socket = tungstenite::accept(stream).map_err(|_| RelayError::Io)?;
    drive(socket, hub)
}

fn connection_tls(
    stream: TcpStream,
    config: &Arc<ServerConfig>,
    hub: &Arc<Hub>,
) -> Result<(), RelayError> {
    stream
        .set_read_timeout(Some(Duration::from_millis(200)))
        .map_err(|_| RelayError::Io)?;
    let session = rustls::ServerConnection::new(Arc::clone(config)).map_err(|_| RelayError::Io)?;
    let tls = rustls::StreamOwned::new(session, stream);
    let socket = tungstenite::accept(tls).map_err(|_| RelayError::Io)?;
    drive(socket, hub)
}

fn drive<S: Read + Write>(
    mut socket: tungstenite::WebSocket<S>,
    hub: &Arc<Hub>,
) -> Result<(), RelayError> {
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

    #[test]
    fn a_restart_keeps_the_blob_and_the_next_cursor() {
        let dir = scratch();
        let mailbox = [4_u8; 32];
        let mut relay = Relay::open(&dir).expect("open");
        let (line, _) = relay.handle_line(&encode_request(&Request::Put {
            mailbox,
            blob: b"sealed".to_vec(),
        }));
        let Response::Ok { cursor: Some(1) } = decode_response(&line).expect("ok") else {
            panic!("cursor");
        };
        drop(relay);
        let mut relay = Relay::open(&dir).expect("reopen");
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
        let (line, _) = relay.handle_line(&encode_request(&Request::Put {
            mailbox,
            blob: b"again".to_vec(),
        }));
        let Response::Ok { cursor: Some(2) } = decode_response(&line).expect("next") else {
            panic!("next cursor");
        };
        drop(relay);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_does_not_reuse_a_cursor_after_restart() {
        let dir = scratch();
        let mailbox = [5_u8; 32];
        let mut relay = Relay::open(&dir).expect("open");
        for blob in [b"one".to_vec(), b"two".to_vec()] {
            let _ = relay.handle_line(&encode_request(&Request::Put { mailbox, blob }));
        }
        let _ = relay.handle_line(&encode_request(&Request::Delete { mailbox, up_to: 1 }));
        drop(relay);
        let mut relay = Relay::open(&dir).expect("reopen");
        let (line, _) = relay.handle_line(&encode_request(&Request::Get {
            mailbox,
            since: 0,
            limit: 10,
        }));
        let Response::Blobs { items } = decode_response(&line).expect("blobs") else {
            panic!("blobs");
        };
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].cursor, 2);
        let (line, _) = relay.handle_line(&encode_request(&Request::Put {
            mailbox,
            blob: b"three".to_vec(),
        }));
        let Response::Ok { cursor: Some(3) } = decode_response(&line).expect("cursor") else {
            panic!("cursor");
        };
        drop(relay);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_bad_file_is_refused_and_left_in_place() {
        let dir = scratch();
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("mailboxes");
        std::fs::write(&path, b"nope").expect("write");
        assert!(Relay::open(&dir).is_err());
        assert_eq!(std::fs::read(&path).expect("still there"), b"nope");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tls_put_round_trips() {
        let key = rcgen::KeyPair::generate().expect("key");
        let params = rcgen::CertificateParams::new(vec!["localhost".to_owned()]).expect("params");
        let cert = params.self_signed(&key).expect("cert");
        let cert_pem = cert.pem();
        let key_pem = key.serialize_pem();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        std::thread::spawn(move || {
            let _ = super::serve_tls_listener(&listener, cert_pem.as_bytes(), key_pem.as_bytes());
        });
        super::tls_provider();
        let mut roots = rustls::RootCertStore::empty();
        roots.add(cert.der().clone()).expect("root");
        let config = rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        let tcp = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connect");
        let (mut socket, _) = tungstenite::client_tls_with_config(
            format!("wss://localhost:{port}/"),
            tcp,
            None,
            Some(tungstenite::Connector::Rustls(std::sync::Arc::new(config))),
        )
        .expect("handshake");
        let line = encode_request(&Request::Put {
            mailbox: [7; 32],
            blob: b"sealed".to_vec(),
        });
        socket
            .send(tungstenite::Message::Text(line.into()))
            .expect("send");
        let reply = socket.read().expect("read");
        let text = reply.to_text().expect("text");
        let Response::Ok { cursor: Some(1) } = decode_response(text).expect("ok") else {
            panic!("cursor");
        };
    }

    fn scratch() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("ambient-relay-{nanos}-{n}"))
    }
}
