//! The hub's listener: a thread a connection within the limits, TLS 1.3 by `httpd`'s provider when a key is given,
//! one lock over the state and the journal, the next challenge in every answer, and the hub's own clock's moves.

use crate::api::{self, Context, Shared, CHALLENGE_HEADER};
use crate::http::{Conn, ReadError, Response};
use crate::util::{self, hex};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{IpAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone)]
pub struct Options {
    pub connections: usize,
    pub per_client: usize,
    pub timeout: Duration,
    pub requests_per_connection: usize,
    pub tls: Option<Arc<httpd::tls::Reloading>>,
    /// The hub's clock, so that tests move it; the system's otherwise.
    pub clock: Arc<dyn Fn() -> u64 + Send + Sync>,
}

impl Options {
    pub fn new() -> Options {
        Options { connections: 128, per_client: 16, timeout: Duration::from_secs(30), requests_per_connection: 1000, tls: None, clock: Arc::new(util::now_ms) }
    }
}

impl Default for Options { fn default() -> Options { Options::new() } }

#[derive(Default)]
struct Counts { all: usize, by: HashMap<IpAddr, usize> }

/// The hub of a data directory: its configuration, its journal replayed, its attachments' directory.
pub fn open(data: &std::path::Path) -> Result<Shared, String> {
    let (keys, settings) = crate::config::load(&data.join("hub.conf"))?;
    if keys.admins.is_empty() { return Err("hub.conf names no admin: no one could decide a registration (mindhub key, then admin NAME KEY)".into()); }
    let mut hub = crate::hub::Hub::new(keys, settings);
    let journal = crate::journal::Journal::open(&data.join("journal.jsonl"), &mut hub)?;
    let mut shared = Shared { hub, journal, challenges: api::Challenges::default(), attachments: data.join("attachments") };
    // A crash between a deletion and its erasure left texts in the journal: they leave it now.
    shared.erase().map_err(|e| format!("the deleted texts cannot leave the journal: {e}"))?;
    Ok(shared)
}

/// Serves until the listener fails; a connection over the limits is answered 503 and closed.
pub fn serve(listener: TcpListener, shared: Arc<Mutex<Shared>>, options: Options) -> std::io::Result<()> {
    let counts = Arc::new(Mutex::new(Counts::default()));
    {
        // The hub's own moves on its clock: a request with no verdict in 30 days (6.3).
        let (shared, clock) = (shared.clone(), options.clock.clone());
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(60));
            let mut s = shared.lock().unwrap();
            let events = s.hub.sweep(clock());
            if let Err(r) = s.commit(events) { eprintln!("MINDHUB: {}", r.detail); }
        });
    }
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let Ok(peer) = stream.peer_addr() else { continue };
        let ip = peer.ip();
        let admitted = {
            let mut c = counts.lock().unwrap();
            let mine = c.by.get(&ip).copied().unwrap_or(0);
            if c.all >= options.connections || mine >= options.per_client { false } else { c.all += 1; *c.by.entry(ip).or_default() += 1; true }
        };
        if !admitted {
            let mut stream = stream;
            let _ = stream.write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nRetry-After: 5\r\nConnection: close\r\n\r\n");
            continue;
        }
        let (shared, counts, options) = (shared.clone(), counts.clone(), options.clone());
        std::thread::spawn(move || {
            connection(stream, ip, &shared, &options);
            let mut c = counts.lock().unwrap();
            c.all -= 1;
            if let Some(n) = c.by.get_mut(&ip) { *n -= 1; if *n == 0 { c.by.remove(&ip); } }
        });
    }
    Ok(())
}

fn connection(stream: TcpStream, ip: IpAddr, shared: &Mutex<Shared>, options: &Options) {
    let _ = stream.set_read_timeout(Some(options.timeout));
    let _ = stream.set_write_timeout(Some(options.timeout));
    let _ = stream.set_nodelay(true);
    match &options.tls {
        Some(tls) => {
            let Ok(session) = rustls::ServerConnection::new(tls.config()) else { return };
            serve_conn(Conn::new(rustls::StreamOwned::new(session, stream)), ip, shared, options)
        }
        None => serve_conn(Conn::new(stream), ip, shared, options),
    }
}

fn serve_conn<S: Socket>(mut conn: Conn<S>, ip: IpAddr, shared: &Mutex<Shared>, options: &Options) {
    let network = util::network(ip);
    for _ in 0..options.requests_per_connection {
        let req = match conn.read(api::body_max) {
            Ok(r) => r,
            Err(ReadError::Bad(status, why)) => {
                let _ = conn.write(&Response::text(status, why), true);
                conn.stream.finish();
                linger(&mut conn.stream);
                return;
            }
            Err(_) => return,
        };
        let response = {
            let mut s = shared.lock().unwrap();
            let now = (options.clock)();
            let cx = Context { now, network: network.clone() };
            let resp = api::handle(&mut s, &req, &cx);
            let challenge = s.challenges.issue(now, &network);
            resp.with(CHALLENGE_HEADER, hex(&challenge))
        };
        if response.status >= 500 { eprintln!("MINDHUB: {} {} {} {}", network, req.method, req.path, response.status); }
        if conn.write(&response, req.close).is_err() || req.close { return; }
    }
}

/// After a refusal mid-request, what the client still sends is read and dropped for a moment, so that it reads the
/// refusal rather than a reset: at most LINGER_BYTES, for at most LINGER.
const LINGER_BYTES: usize = 16 * 1024 * 1024;
const LINGER: Duration = Duration::from_secs(2);

/// A connection's socket, plain or TLS: its answer ended, the rest of the client's bytes read for a moment.
pub trait Socket: Read + Write { fn finish(&mut self); }

impl Socket for TcpStream {
    fn finish(&mut self) {
        let _ = self.shutdown(std::net::Shutdown::Write);
        let _ = self.set_read_timeout(Some(LINGER));
    }
}

impl Socket for rustls::StreamOwned<rustls::ServerConnection, TcpStream> {
    fn finish(&mut self) {
        self.conn.send_close_notify();
        let _ = self.flush();
        let _ = self.sock.shutdown(std::net::Shutdown::Write);
        let _ = self.sock.set_read_timeout(Some(LINGER));
    }
}

fn linger<S: Read>(stream: &mut S) {
    let start = std::time::Instant::now();
    let mut buf = [0u8; 16384];
    let mut total = 0;
    while total < LINGER_BYTES && start.elapsed() < LINGER {
        match stream.read(&mut buf) { Ok(0) | Err(_) => break, Ok(n) => total += n }
    }
}
