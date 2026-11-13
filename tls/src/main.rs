#![no_std]
#![no_main]
// TLS service (issue 103, Appendix B.6): TLS 1.3 client sessions (rustls, unbuffered API) over flows the clients lend
// (idl/tls.wit). The service has no network access of its own: every connection goes through the client's socket
// capability, so the network policy of the client holds. Server certificates are verified against tlsroots.pem on the
// boot disk, or a server is known by the SHA-256 of its public key (connect-pinned, 351-NET-0002); a client certificate
// is the device certificate, signed for by the key service. The server role (1.3, 110-NET-0069): a session listens on
// its lent flow and each connection it accepts is a session served with the device certificate (or tlsserver.pem's
// chain for the device key), its handshake taken as far as the peer's bytes allow at each call, so no peer is waited
// for. 1.4 (111-NET-0073): a client session may offer one application protocol (ALPN), and any established session's
// client may derive keying material from its exporter (`exporter.rs`). Random bytes come from RDRAND only: without it
// every connection is refused. Holds: an RTC client (slot 2,
// certificate times), a VFS client (slot 3, the root store and the server's chain), the key service's signer client (slot 4).
extern crate alloc;

mod device;
mod exporter;
mod pem;
mod pinned;
mod provider;

use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::Ordering;

use mind::abi::BootInfo;
use mind::idl::socket;
use mind::idl::tls::{self as api, Error, Peer};
use mind::idl::wire;
use mind::ipc::{self, Endpoint};
use mind::network::BADGE_TLS_NO_DEVICE;
use pki_types::CertificateDer;
use rustls::client::{UnbufferedClientConnection, WebPkiServerVerifier};
use rustls::server::UnbufferedServerConnection;
use rustls::unbuffered::{ConnectionState, EncodeError, UnbufferedStatus};
use rustls::crypto::CryptoProvider;
use rustls::crypto::hash::HashAlgorithm;
use rustls::{ClientConfig, CommonState, KeyLog, RootCertStore, ServerConfig, SupportedCipherSuite};

const RECEIVED: usize = 9;
const KEYS: Endpoint = Endpoint(4);
const ROOTS_FILE: &str = "tlsroots.pem";
const SESSIONS: usize = 16; // in all (1.3; 8 before)
const PER_CLIENT: usize = 8; // of one client (1.3)
const SERVER_FILE: &str = "tlsserver.pem"; // the server's chain, when not the device certificate alone (1.3)
const HANDSHAKE_MS: u64 = 10_000; // longest a served handshake takes from its accept (1.3)
const CHUNK: usize = 4096; // largest send or receive (idl/tls.wit)
const IDLE_MS: u64 = 5000; // longest wait for the peer in send, receive and close
const PLAIN_MAX: usize = 64 * 1024; // decrypted bytes held for a client before the socket is read again

/// The key log of one connection: its exporter secret alone (1.4).
#[derive(Debug)]
struct Keys(Arc<exporter::Secret>);
impl KeyLog for Keys {
    fn will_log(&self, label: &str) -> bool { label == "EXPORTER_SECRET" }
    fn log(&self, label: &str, _client_random: &[u8], secret: &[u8]) { self.0.log(label, secret); }
}

/// Why a connection failed: a TLS error (logged with its details) or an error for the client.
enum Failure { Tls(rustls::Error), Client(Error) }
impl From<Error> for Failure { fn from(error: Error) -> Self { Failure::Client(error) } }

fn socket_error(error: socket::Error) -> Error {
    match error {
        socket::Error::NoNetwork | socket::Error::Unreachable => Error::NoNetwork,
        socket::Error::Refused => Error::Refused,
        socket::Error::Timeout => Error::Timeout,
        socket::Error::Closed => Error::Closed,
        socket::Error::Again => Error::Again,
        socket::Error::Denied => Error::Denied,
        socket::Error::NoSocket | socket::Error::Limit => Error::Limit,
        socket::Error::NotFound => Error::NotFound,
        socket::Error::Invalid => Error::Invalid,
    }
}

// The flow cannot be used (revoked lease, no stack): the client's network access is what failed.
fn flow<T>(result: mind::sys::Result<Result<T, socket::Error>>) -> Result<T, Error> {
    result.map_err(|_| Error::Denied)?.map_err(socket_error)
}

fn now() -> u64 { mind::time::uptime_ms() as u64 }

// A TLS error after the handshake: a close_notify ends the stream; anything else (a TLS 1.3 server refusing the client
// certificate sends its alert only now) is a failed handshake or a broken session, and is logged.
fn broken(id: u32, failure: Failure) -> Error {
    match failure {
        Failure::Client(error) => error,
        Failure::Tls(rustls::Error::AlertReceived(rustls::AlertDescription::CloseNotify)) => Error::Closed,
        Failure::Tls(error) => { mind::println!("[TLS] SESSION {}: {:?}", id, error); Error::Handshake }
    }
}

/// The TCP connection under a session.
struct Link { flow: Endpoint, socket: u32, eof: bool }

impl Link {
    fn transmit(&mut self, mut data: &[u8], deadline: u64) -> Result<(), Error> {
        while !data.is_empty() {
            let taken = flow(socket::tcp_send(self.flow, self.socket, &data[..data.len().min(CHUNK)]))? as usize;
            data = &data[taken.min(data.len())..];
            if taken == 0 { if now() > deadline { return Err(Error::Timeout); } mind::time::sleep(2); }
        }
        Ok(())
    }

    // Appends what has arrived to `incoming`; false when nothing has.
    fn fill(&mut self, incoming: &mut Vec<u8>) -> Result<bool, Error> {
        if self.eof { return Ok(false); }
        let mut buffer = [0u8; CHUNK];
        match flow(socket::tcp_receive(self.flow, self.socket, CHUNK as u32, &mut buffer)) {
            Ok(n) => { incoming.extend_from_slice(&buffer[..n]); Ok(n > 0) }
            Err(Error::Again) => Ok(false),
            Err(Error::Closed) => { self.eof = true; Ok(false) }
            Err(error) => Err(error),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step { Progress, NeedData, Ready, Closed }

#[derive(Clone, Copy)]
enum Write<'a> { Data(&'a [u8]), CloseNotify }

/// A session's TLS connection: a client's, or one it serves (1.3).
enum Conn { Client(UnbufferedClientConnection), Server(UnbufferedServerConnection) }

impl Conn {
    fn state(&self) -> &CommonState { match self { Conn::Client(c) => c, Conn::Server(s) => s } }
}

/// A listening session (1.3): its socket and the configuration of the sessions it accepts.
struct Listener { socket: u32, config: Arc<ServerConfig> }

/// An accepted session's handshake (1.3): the peer, when it must be done, and how it went once it is over.
struct Serving { address: u32, port: u16, until: u64, outcome: Option<Result<Peer, Error>> }

struct Session {
    id: u32, owner: u64, flow: usize,
    conn: Option<Conn>, link: Option<Link>, device: Option<Arc<device::Device>>, secret: Option<Arc<exporter::Secret>>,
    incoming: Vec<u8>, outgoing: Vec<u8>, plain: VecDeque<u8>, peer_closed: bool,
    listener: Option<Listener>, serving: Option<Serving>,
}

// One state of a connection of either side handled: what it gave, whether `write` went out (once the connection can
// take application data), and how many bytes of the records it was given are used.
fn handle<D>(status: UnbufferedStatus<'_, '_, D>, link: &mut Link, outgoing: &mut Vec<u8>, plain: &mut VecDeque<u8>, peer_closed: &mut bool,
             write: Option<Write>, deadline: u64) -> (Result<Step, Failure>, bool, usize) {
    let UnbufferedStatus { mut discard, state } = status;
    let mut wrote = false;
    let result = match state {
        Err(error) => Err(Failure::Tls(error)),
        Ok(ConnectionState::ReadTraffic(mut traffic)) => {
            let mut result = Ok(Step::Progress);
            while let Some(record) = traffic.next_record() {
                match record {
                    Ok(record) => { discard += record.discard; plain.extend(record.payload.iter()); }
                    Err(error) => { result = Err(Failure::Tls(error)); break; }
                }
            }
            result
        }
        Ok(ConnectionState::PeerClosed) => { *peer_closed = true; Ok(Step::Progress) }
        Ok(ConnectionState::Closed) => Ok(Step::Closed),
        Ok(ConnectionState::EncodeTlsData(mut data)) => {
            let used = outgoing.len();
            outgoing.resize(used + CHUNK, 0);
            loop {
                match data.encode(&mut outgoing[used..]) {
                    Ok(n) => { outgoing.truncate(used + n); break Ok(Step::Progress); }
                    Err(EncodeError::InsufficientSize(size)) => outgoing.resize(used + size.required_size, 0),
                    Err(_) => { outgoing.truncate(used); break Err(Failure::Client(Error::Handshake)); }
                }
            }
        }
        Ok(ConnectionState::TransmitTlsData(data)) => {
            let sent = link.transmit(outgoing, deadline);
            outgoing.clear();
            data.done();
            sent.map(|()| Step::Progress).map_err(Failure::Client)
        }
        Ok(ConnectionState::BlockedHandshake) => Ok(Step::NeedData),
        Ok(ConnectionState::WriteTraffic(mut traffic)) => {
            let mut buffer = if write.is_some() { alloc::vec![0u8; CHUNK + 64] } else { Vec::new() };
            let encrypted = match write {
                None => Ok(0),
                Some(Write::Data(data)) => traffic.encrypt(data, &mut buffer),
                Some(Write::CloseNotify) => traffic.queue_close_notify(&mut buffer),
            };
            match encrypted {
                Ok(n) => { wrote = write.is_some(); link.transmit(&buffer[..n], deadline).map(|()| Step::Ready).map_err(Failure::Client) }
                Err(_) => Err(Failure::Client(Error::Closed)),
            }
        }
        Ok(_) => Ok(Step::Progress),
    };
    (result, wrote, discard)
}

impl Session {
    fn new(id: u32, owner: u64, flow: usize) -> Box<Session> {
        Box::new(Session { id, owner, flow, conn: None, link: None, device: None, secret: None, incoming: Vec::new(), outgoing: Vec::new(), plain: VecDeque::new(),
                           peer_closed: false, listener: None, serving: None })
    }

    // One state of the connection handled; `write` is done when the connection can take application data (the bool).
    fn step(&mut self, write: Option<Write>, deadline: u64) -> Result<(Step, bool), Failure> {
        let (Some(conn), Some(link)) = (self.conn.as_mut(), self.link.as_mut()) else { return Err(Error::Invalid.into()) };
        let (incoming, outgoing, plain, closed) = (&mut self.incoming, &mut self.outgoing, &mut self.plain, &mut self.peer_closed);
        let (result, wrote, discard) = match conn {
            Conn::Client(c) => handle(c.process_tls_records(&mut incoming[..]), link, outgoing, plain, closed, write, deadline),
            Conn::Server(s) => handle(s.process_tls_records(&mut incoming[..]), link, outgoing, plain, closed, write, deadline),
        };
        incoming.drain(..discard.min(incoming.len()));
        result.map(|step| (step, wrote))
    }

    // Handles states until the connection waits for the peer or for application data; `write` goes out on the way.
    fn pump(&mut self, mut write: Option<Write>, deadline: u64) -> Result<(Step, bool), Failure> {
        let mut wrote = false;
        for _ in 0..256 {
            let (step, done) = self.step(write, deadline)?;
            if done { write = None; wrote = true; }
            if step != Step::Progress { return Ok((step, wrote)); }
        }
        Ok((Step::Progress, wrote))
    }

    // Reads what has arrived and decrypts it into `plain`, up to PLAIN_MAX: what the client has not taken yet stays in
    // the stack's receive window, so a slow client slows the sender instead of filling this service's memory.
    fn poll(&mut self) -> Result<Step, Failure> {
        let deadline = now() + IDLE_MS;
        loop {
            if self.plain.len() >= PLAIN_MAX { return Ok(Step::Progress); }
            let more = match (self.link.as_mut(), &mut self.incoming) { (Some(link), incoming) => link.fill(incoming)?, _ => return Err(Error::Invalid.into()) };
            let (step, _) = self.pump(None, deadline)?;
            if !more || step == Step::Closed { return Ok(step); }
        }
    }

    // An accepted session's handshake taken as far as the bytes that have arrived allow (1.3): whether it is done.
    fn advance(&mut self, until: u64) -> Result<bool, Failure> {
        loop {
            let more = match (self.link.as_mut(), &mut self.incoming) { (Some(link), incoming) => link.fill(incoming)?, _ => return Err(Error::Invalid.into()) };
            let (step, _) = self.pump(None, until)?;
            match self.conn.as_ref() { Some(conn) if !conn.state().is_handshaking() => return Ok(true), Some(_) => {}, None => return Err(Error::Invalid.into()) }
            if step == Step::Closed || self.link.as_ref().is_some_and(|l| l.eof) { return Err(Error::Closed.into()); }
            if !more { return Ok(false); }
        }
    }

    fn handshake(&mut self, deadline: u64) -> Result<(), Failure> {
        loop {
            match self.pump(None, deadline)?.0 {
                Step::Ready => return Ok(()),
                Step::Closed => return Err(Error::Handshake.into()),
                Step::NeedData | Step::Progress => {
                    let link = self.link.as_mut().ok_or(Error::Invalid)?;
                    if !link.fill(&mut self.incoming)? {
                        if link.eof { return Err(Error::Closed.into()); }
                        if now() > deadline { return Err(Error::Timeout.into()); }
                        mind::time::sleep(2);
                    }
                }
            }
        }
    }
}

struct Service { provider: Arc<CryptoProvider>, sessions: Vec<Box<Session>>, next: u32 }

// The suite and group a connection agreed on (IANA numbers).
fn negotiated(conn: &Conn) -> (u16, u16) {
    let state = conn.state();
    (state.negotiated_cipher_suite().map_or(0, |s| u16::from(s.suite())), state.negotiated_key_exchange_group().map_or(0, |g| u16::from(g.name())))
}

// The root store from the boot disk, read for every connection so a changed file takes effect; with the count of
// certificates in it that did not parse.
fn roots() -> (RootCertStore, usize) {
    let mut store = RootCertStore::empty();
    let Ok(file) = mind::fs::File::open(ROOTS_FILE) else { return (store, 0) };
    let mut text = alloc::vec![0u8; file.size().min(512 * 1024)];
    let length = file.read_at(0, &mut text).unwrap_or(0);
    let certificates = pem::certificates(core::str::from_utf8(&text[..length]).unwrap_or(""));
    let (_, refused) = store.add_parsable_certificates(certificates.into_iter().map(pki_types::CertificateDer::from));
    (store, refused)
}

impl Service {
    fn session(&mut self, id: u32, pid: u64) -> Result<&mut Session, Error> {
        self.sessions.iter_mut().find(|s| s.id == id && s.owner == pid).map(|s| &mut **s).ok_or(Error::NotFound)
    }

    // Whether client `pid` may have one more session: 16 in all, 8 of one client.
    fn room(&self, pid: u64) -> bool { self.sessions.len() < SESSIONS && self.sessions.iter().filter(|s| s.owner == pid).count() < PER_CLIENT }

    fn number(&mut self) -> u32 { self.next = self.next.wrapping_add(1).max(1); self.next }

    fn attach(&mut self, flow: usize, pid: u64) -> Result<u32, Error> {
        if !self.room(pid) { return Err(Error::Limit); }
        // A child of the lent capability: the client's revocation removes it too.
        let flow = ipc::mint(flow, u8::MAX, 0, 0).map_err(|_| Error::Limit)?;
        let id = self.number();
        self.sessions.push(Session::new(id, pid, flow));
        Ok(id)
    }

    // The server verified against the root store, or by its pins.
    fn config(&self, device: Option<Arc<device::Device>>, pins: Option<pinned::Pinned>, alpn: Option<&str>, secret: Arc<exporter::Secret>) -> Result<ClientConfig, Error> {
        let builder = ClientConfig::builder_with_details(self.provider.clone(), Arc::new(provider::Rtc))
            .with_protocol_versions(&[&rustls::version::TLS13]).map_err(|_| Error::Handshake)?;
        let builder = match pins {
            Some(pins) => builder.dangerous().with_custom_certificate_verifier(Arc::new(pins)),
            None => {
                let (roots, _) = roots();
                if roots.is_empty() { return Err(Error::NoRoots); }
                builder.with_webpki_verifier(WebPkiServerVerifier::builder_with_provider(Arc::new(roots), self.provider.clone()).build().map_err(|_| Error::NoRoots)?)
            }
        };
        let mut config = match device { Some(device) => builder.with_client_cert_resolver(device), None => builder.with_no_client_auth() };
        config.alpn_protocols = alpn.map(|p| alloc::vec![p.as_bytes().to_vec()]).unwrap_or_default();
        config.key_log = Arc::new(Keys(secret));
        Ok(config)
    }

    fn connect(&mut self, id: u32, pid: u64, name: &str, address: u32, port: u16, client_certificate: bool, pins: Option<pinned::Pinned>, alpn: Option<&str>,
               timeout: u32) -> Result<Peer, Error> {
        if !mind::random::available() { return Err(Error::NoEntropy); }
        let server = pki_types::ServerName::try_from(name).map_err(|_| Error::Invalid)?.to_owned();
        let device = if client_certificate { Some(Arc::new(device::Device::new(KEYS).ok_or(Error::Denied)?)) } else { None };
        let how = match &pins { Some(p) if p.checked => " BY ITS PIN", Some(_) => " BY ITS PINNED KEY", None => "" };
        // connect-pinned (1.1) logs its refusal as it always did; connect-pins (1.2) has one or two pins of either kind.
        let unmatched = match &pins { Some(p) if p.checked => "NOT A PINNED KEY OR CERTIFICATE", _ => "NOT THE PINNED KEY" };
        let pinned = pins.is_some();
        if alpn.is_some_and(|p| p.is_empty()) { return Err(Error::Invalid); }
        let secret = Arc::new(exporter::Secret::new());
        let config = Arc::new(self.config(device.clone(), pins, alpn, secret.clone())?);
        let session = self.session(id, pid)?;
        if session.conn.is_some() || session.listener.is_some() || session.serving.is_some() { return Err(Error::Invalid); }
        let deadline = now() + timeout.clamp(100, 60_000) as u64;
        let endpoint = Endpoint(session.flow);
        let socket = flow(socket::tcp_connect(endpoint, address, port, timeout.clamp(100, 60_000)))?;
        session.link = Some(Link { flow: endpoint, socket, eof: false });
        session.device = device;
        session.secret = Some(secret);
        let id = session.id;
        let result = match UnbufferedClientConnection::new(config, server) {
            Ok(conn) => { session.conn = Some(Conn::Client(conn)); session.handshake(deadline) }
            Err(error) => Err(Failure::Tls(error)),
        };
        // A server that did not agree on the protocol offered (1.4).
        let agreed = session.conn.as_ref().and_then(|c| c.state().alpn_protocol());
        let disagreed = result.is_ok() && alpn.is_some_and(|p| agreed != Some(p.as_bytes()));
        let result = if disagreed { Err(Failure::Client(Error::Handshake)) } else { result };
        let how = match alpn { Some(p) if agreed == Some(p.as_bytes()) => alloc::format!("{}, PROTOCOL {}", how, p), _ => how.into() };
        match result {
            Ok(()) => {
                let (suite, group) = negotiated(session.conn.as_ref().ok_or(Error::Invalid)?);
                let sent = session.device.as_ref().is_some_and(|d| d.sent.load(Ordering::Relaxed));
                mind::println!("[TLS] SESSION {} OF PID {}: {} VERIFIED{}, SUITE {:04X}, GROUP {:04X}, CLIENT CERTIFICATE {}", id, pid, name, how, suite, group, if sent { "SENT" } else { "NOT SENT" });
                Ok(Peer { suite, group, client_certificate: sent })
            }
            Err(failure) => {
                let error = match &failure {
                    Failure::Tls(rustls::Error::InvalidCertificate(rustls::CertificateError::ApplicationVerificationFailure)) if pinned => {
                        mind::println!("[TLS] SESSION {} OF PID {}: {} REFUSED: {}", id, pid, name, unmatched); Error::Certificate
                    }
                    Failure::Tls(rustls::Error::InvalidCertificate(why)) => { mind::println!("[TLS] SESSION {} OF PID {}: {} REFUSED: CERTIFICATE {:?}", id, pid, name, why); Error::Certificate }
                    Failure::Tls(error) => { mind::println!("[TLS] SESSION {} OF PID {}: HANDSHAKE WITH {} FAILED: {:?}", id, pid, name, error); Error::Handshake }
                    _ if disagreed => {
                        mind::println!("[TLS] SESSION {} OF PID {}: {} REFUSED: IT DID NOT AGREE ON THE PROTOCOL {}", id, pid, name, alpn.unwrap_or("")); Error::Handshake
                    }
                    Failure::Client(error) => { mind::println!("[TLS] SESSION {} OF PID {}: {} FAILED: {:?}", id, pid, name, error); *error }
                };
                if let Some(link) = session.link.take() { let _ = socket::close(link.flow, link.socket); }
                session.conn = None;
                session.secret = None;
                Err(error)
            }
        }
    }

    // `out.len()` bytes of the established session's exporter for `label` and `context` (1.4).
    fn export(&mut self, id: u32, pid: u64, label: &str, context: &[u8], out: &mut [u8]) -> Result<(), Error> {
        self.ready(id, pid)?;
        let session = self.session(id, pid)?;
        let state = session.conn.as_ref().ok_or(Error::Invalid)?.state();
        if state.is_handshaking() || out.is_empty() || out.len() > exporter::LENGTH_MAX { return Err(Error::Invalid); }
        let hash = match state.negotiated_cipher_suite() {
            Some(SupportedCipherSuite::Tls13(suite)) if suite.common.hash_provider.algorithm() == HashAlgorithm::SHA384 => exporter::Hash::Sha384,
            Some(SupportedCipherSuite::Tls13(_)) => exporter::Hash::Sha256,
            _ => return Err(Error::Invalid),
        };
        let secret = session.secret.as_ref().and_then(|s| s.get()).ok_or(Error::Invalid)?;
        let made = exporter::export(hash, &secret, label.as_bytes(), context, out);
        exporter::forget(secret);
        made.ok_or(Error::Invalid)
    }

    fn send(&mut self, id: u32, pid: u64, data: &[u8]) -> Result<u32, Error> {
        self.ready(id, pid)?;
        let session = self.session(id, pid)?;
        if session.conn.is_none() { return Err(Error::Invalid); }
        let _ = session.poll();
        match session.pump(Some(Write::Data(data)), now() + IDLE_MS) {
            Ok((_, true)) => Ok(data.len() as u32),
            Ok((Step::Closed, _)) => Err(Error::Closed),
            Ok(_) => Err(Error::Again),
            Err(failure) => Err(broken(id, failure)),
        }
    }

    fn receive(&mut self, id: u32, pid: u64, length: u32, out: &mut [u8; CHUNK]) -> Result<usize, Error> {
        self.ready(id, pid)?;
        let session = self.session(id, pid)?;
        if session.conn.is_none() { return Err(Error::Invalid); }
        let step = session.poll();
        if !session.plain.is_empty() {
            let n = session.plain.len().min(length as usize).min(CHUNK);
            for (slot, byte) in out.iter_mut().zip(session.plain.drain(..n)) { *slot = byte; }
            return Ok(n);
        }
        match step {
            Err(failure) => Err(broken(id, failure)),
            Ok(Step::Closed) => Err(Error::Closed),
            Ok(_) if session.peer_closed || session.link.as_ref().is_some_and(|l| l.eof) => Err(Error::Closed),
            Ok(_) => Err(Error::Again),
        }
    }

    // Frees session `index`: close_notify if it can still be sent, the TCP connection or the listening socket, the
    // lent flow.
    fn end(&mut self, index: usize) {
        let mut session = self.sessions.swap_remove(index);
        if session.conn.is_some() { let _ = session.pump(Some(Write::CloseNotify), now() + IDLE_MS); }
        if let Some(link) = session.link.take() { let _ = socket::close(link.flow, link.socket); }
        if let Some(listener) = session.listener.take() { let _ = socket::close(Endpoint(session.flow), listener.socket); }
        let _ = ipc::drop_cap(session.flow);
    }

    // The server's chain (1.3): tlsserver.pem's certificates when the file is there, the first holding the device key;
    // else the device certificate alone. With what the log calls it.
    fn chain(&self) -> Result<(Vec<CertificateDer<'static>>, alloc::string::String), Error> {
        let device = device::certificate(KEYS).ok_or(Error::Denied)?;
        let file = match mind::fs::File::open(SERVER_FILE) {
            Ok(file) => file,
            Err(mind::fs::Error::NotFound) => return Ok((alloc::vec![device], "THE DEVICE CERTIFICATE".into())),
            Err(error) => { mind::println!("[TLS] {} NOT READ: {:?}", SERVER_FILE, error); return Err(Error::Certificate); }
        };
        let mut text = alloc::vec![0u8; file.size().min(64 * 1024)];
        let length = file.read_at(0, &mut text).unwrap_or(0);
        let chain: Vec<CertificateDer<'static>> = pem::certificates(core::str::from_utf8(&text[..length]).unwrap_or("")).into_iter().map(CertificateDer::from).collect();
        let key = pinned::spki_sha256(&device);
        match chain.first() {
            None => { mind::println!("[TLS] {} HOLDS NO CERTIFICATE", SERVER_FILE); Err(Error::Certificate) }
            Some(leaf) if key.is_none() || pinned::spki_sha256(leaf) != key => {
                mind::println!("[TLS] {}: ITS FIRST CERTIFICATE DOES NOT HOLD THE DEVICE KEY", SERVER_FILE); Err(Error::Certificate)
            }
            Some(_) => { let name = alloc::format!("{} ({} CERTIFICATES)", SERVER_FILE, chain.len()); Ok((chain, name)) }
        }
    }

    // TLS 1.3 as the server with `chain`, every signature the key service's; no tickets, so no resumption.
    fn server_config(&self, chain: Vec<CertificateDer<'static>>) -> Result<Arc<ServerConfig>, Error> {
        let mut config = ServerConfig::builder_with_details(self.provider.clone(), Arc::new(provider::Rtc))
            .with_protocol_versions(&[&rustls::version::TLS13]).map_err(|_| Error::Handshake)?
            .with_no_client_auth().with_cert_resolver(Arc::new(device::Served::new(KEYS, chain)));
        config.send_tls13_tickets = 0;
        Ok(Arc::new(config))
    }

    // Session `id` listens on TCP `port` of its flow (1.3); the flow's listening grant decides whether it may.
    fn listen(&mut self, id: u32, pid: u64, port: u16, backlog: u8) -> Result<(), Error> {
        if !mind::random::available() { return Err(Error::NoEntropy); }
        let session = self.session(id, pid)?;
        if session.conn.is_some() || session.listener.is_some() || session.serving.is_some() { return Err(Error::Invalid); }
        let (chain, served) = self.chain()?;
        let config = self.server_config(chain)?;
        let session = self.session(id, pid)?;
        match flow(socket::tcp_listen(Endpoint(session.flow), port, backlog)) {
            Ok(socket) => {
                session.listener = Some(Listener { socket, config });
                mind::println!("[TLS] SESSION {} OF PID {} LISTENS ON TCP {}, SERVING {}", id, pid, port, served);
                Ok(())
            }
            Err(error) => { mind::println!("[TLS] SESSION {} OF PID {}: TCP {} NOT LISTENED ON: {:?}", id, pid, port, error); Err(error) }
        }
    }

    // The oldest connection listener `id` holds, as a new session whose handshake as the server has started (1.3).
    fn accept(&mut self, id: u32, pid: u64) -> Result<api::Accepted, Error> {
        let listener = self.session(id, pid)?;
        let (flow_slot, socket_id, config) = match listener.listener.as_ref() { Some(l) => (listener.flow, l.socket, l.config.clone()), None => return Err(Error::Invalid) };
        if !self.room(pid) { return Err(Error::Limit); }
        let taken = flow(socket::tcp_accept(Endpoint(flow_slot), socket_id))?;
        // A child of the listener's flow, so that either can end without the other; the client's revocation removes both.
        let child = match ipc::mint(flow_slot, u8::MAX, 0, 0) { Ok(child) => child, Err(_) => { let _ = socket::close(Endpoint(flow_slot), taken.socket); return Err(Error::Limit); } };
        let secret = Arc::new(exporter::Secret::new());
        let mut config = (*config).clone();
        config.key_log = Arc::new(Keys(secret.clone()));
        let conn = match UnbufferedServerConnection::new(Arc::new(config)) {
            Ok(conn) => conn,
            Err(error) => {
                mind::println!("[TLS] SESSION {} OF PID {}: NO SERVER CONNECTION: {:?}", id, pid, error);
                let _ = socket::close(Endpoint(child), taken.socket);
                let _ = ipc::drop_cap(child);
                return Err(Error::Handshake);
            }
        };
        let number = self.number();
        let mut session = Session::new(number, pid, child);
        session.conn = Some(Conn::Server(conn));
        session.secret = Some(secret);
        session.link = Some(Link { flow: Endpoint(child), socket: taken.socket, eof: false });
        session.serving = Some(Serving { address: taken.address, port: taken.port, until: now() + HANDSHAKE_MS, outcome: None });
        self.sessions.push(session);
        let _ = self.served(number, pid); // as far as the peer's bytes go now
        Ok(api::Accepted { session: number, address: taken.address, port: taken.port })
    }

    // Takes accepted session `id`'s handshake as far as the peer's bytes allow (1.3): `again` while it waits for them,
    // then how it went, logged once; a failed one's connection is closed.
    fn served(&mut self, id: u32, pid: u64) -> Result<Peer, Error> {
        let session = self.session(id, pid)?;
        let Some(serving) = session.serving.as_ref() else { return Err(Error::Invalid) };
        if let Some(outcome) = serving.outcome { return outcome; }
        let (until, address, port) = (serving.until, serving.address, serving.port);
        let result = match session.advance(until) {
            Ok(true) => Ok(()),
            Ok(false) if now() > until => Err(Failure::Client(Error::Timeout)),
            Ok(false) => return Err(Error::Again),
            Err(failure) => Err(failure),
        };
        let [a, b, c, d] = address.to_be_bytes();
        let outcome = match result {
            Ok(()) => {
                let (suite, group) = negotiated(session.conn.as_ref().ok_or(Error::Invalid)?);
                mind::println!("[TLS] SESSION {} OF PID {}: SERVED {}.{}.{}.{}:{}, SUITE {:04X}, GROUP {:04X}", id, pid, a, b, c, d, port, suite, group);
                Ok(Peer { suite, group, client_certificate: false })
            }
            Err(failure) => {
                let error = match failure {
                    Failure::Client(Error::Timeout) => { mind::println!("[TLS] SESSION {} OF PID {}: HANDSHAKE WITH {}.{}.{}.{}:{} TIMED OUT", id, pid, a, b, c, d, port); Error::Timeout }
                    Failure::Client(error) => { mind::println!("[TLS] SESSION {} OF PID {}: HANDSHAKE WITH {}.{}.{}.{}:{} FAILED: {:?}", id, pid, a, b, c, d, port, error); error }
                    Failure::Tls(error) => { mind::println!("[TLS] SESSION {} OF PID {}: HANDSHAKE WITH {}.{}.{}.{}:{} FAILED: {:?}", id, pid, a, b, c, d, port, error); Error::Handshake }
                };
                if let Some(link) = session.link.take() { let _ = socket::close(link.flow, link.socket); }
                session.conn = None;
                Err(error)
            }
        };
        if let Some(serving) = session.serving.as_mut() { serving.outcome = Some(outcome); }
        outcome
    }

    // Before `send` and `receive`: an accepted session's handshake done (`again` while it goes on); any other session as it is.
    fn ready(&mut self, id: u32, pid: u64) -> Result<(), Error> {
        if self.session(id, pid)?.serving.is_none() { return Ok(()); }
        self.served(id, pid).map(drop)
    }

    fn close(&mut self, id: u32, pid: u64) -> Result<(), Error> {
        let index = self.sessions.iter().position(|s| s.id == id && s.owner == pid).ok_or(Error::NotFound)?;
        self.end(index);
        Ok(())
    }

    // Sessions of processes that ended; served handshakes past their time (1.3), whose sockets are freed for other peers.
    fn expire(&mut self) {
        while let Some(index) = self.sessions.iter().position(|s| !mind::process::alive(s.owner)) {
            mind::println!("[TLS] PROCESS {} ENDED: SESSION {} CLOSED", self.sessions[index].owner, self.sessions[index].id);
            self.end(index);
        }
        let late: Vec<(u32, u64)> = self.sessions.iter()
            .filter(|s| s.serving.as_ref().is_some_and(|v| v.outcome.is_none() && now() > v.until)).map(|s| (s.id, s.owner)).collect();
        for (id, pid) in late { let _ = self.served(id, pid); }
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut service = Service { provider: provider::provider(), sessions: Vec::new(), next: 0 };
    let (entropy, outcome) = if mind::random::available() { ("RANDOM FROM ", "") } else { ("NO ", ": EVERY CONNECTION WILL BE REFUSED") };
    let (store, refused) = roots();
    let unparsed = if refused > 0 { alloc::format!(" ({} NOT PARSED)", refused) } else { alloc::string::String::new() };
    mind::println!("[TLS] READY: TLS 1.3 CLIENT, {} ROOTS FROM {}{}, {}{}{}", store.len(), ROOTS_FILE, unparsed, entropy, mind::random::SOURCE, outcome);
    drop(store);
    let mut scratch = Box::new([0u8; api::REQUEST_MAX]);
    let mut buffer = Box::new([0u8; CHUNK]);
    let mut checked = 0u64;
    loop {
        if now() - checked >= 1000 { checked = now(); service.expire(); }
        let Ok(request) = Endpoint::SERVICE.recv_timeout(RECEIVED, 1000) else { continue };
        if !request.is_call { continue; }
        let (pid, badge) = (request.sender, request.badge);
        let _ = match api::decode(&request, RECEIVED, &mut scratch) {
            Err(reason) => wire::reject(reason),
            Ok((api::Request::Attach { flow }, call)) => api::reply_attach(call, service.attach(flow, pid)),
            Ok((api::Request::Connect { session, name, address, port, client_certificate, timeout_ms }, call)) => {
                // A client badged BADGE_TLS_NO_DEVICE may not present the device certificate (351-NET-0022).
                let peer = if client_certificate && badge & BADGE_TLS_NO_DEVICE != 0 {
                    mind::println!("[TLS] PID {}: THE DEVICE CERTIFICATE REFUSED TO A CLIENT WITHOUT THE RIGHT TO IT", pid);
                    Err(Error::Denied)
                } else { service.connect(session, pid, name.as_str(), address, port, client_certificate, None, None, timeout_ms) };
                api::reply_connect(call, peer.as_ref().map_err(|e| *e))
            }
            Ok((api::Request::ConnectPinned { session, name, address, port, pin, timeout_ms }, call)) => {
                let peer = match <[u8; 32]>::try_from(pin) {
                    Ok(pin) => service.connect(session, pid, name.as_str(), address, port, false, Some(pinned::Pinned::key(pin, service.provider.clone())), None, timeout_ms),
                    Err(_) => Err(Error::Invalid),
                };
                api::reply_connect_pinned(call, peer.as_ref().map_err(|e| *e))
            }
            Ok((api::Request::ConnectPins { session, name, address, port, pins, timeout_ms }, call)) => {
                let list = pins.as_slice();
                let mut chosen = pinned::Pinned { pins: [(pinned::Kind::Leaf, [0; 32]); 2], count: 0, checked: true, provider: service.provider.clone() };
                let mut valid = (1..=2).contains(&list.len());
                for (slot, pin) in chosen.pins.iter_mut().zip(list) {
                    let kind = if pin.kind == api::PinKind::Leaf { pinned::Kind::Leaf } else { pinned::Kind::Spki };
                    match <[u8; 32]>::try_from(pin.digest.as_slice()) { Ok(digest) => *slot = (kind, digest), Err(_) => valid = false }
                }
                chosen.count = list.len().min(2);
                let peer = if valid { service.connect(session, pid, name.as_str(), address, port, false, Some(chosen), None, timeout_ms) } else { Err(Error::Invalid) };
                api::reply_connect_pins(call, peer.as_ref().map_err(|e| *e))
            }
            Ok((api::Request::ConnectAlpn { session, name, address, port, protocol, timeout_ms }, call)) => {
                let peer = service.connect(session, pid, name.as_str(), address, port, false, None, Some(protocol.as_str()), timeout_ms);
                api::reply_connect_alpn(call, peer.as_ref().map_err(|e| *e))
            }
            Ok((api::Request::Export { session, label, context, length }, call)) => {
                let mut out = [0u8; exporter::LENGTH_MAX];
                let length = (length as usize).min(exporter::LENGTH_MAX + 1);
                let result = match out.get_mut(..length) {
                    Some(part) if length > 0 => service.export(session, pid, label.as_str(), context, part).map(|()| length),
                    _ => Err(Error::Invalid),
                };
                let sent = api::reply_export(call, result.map(|n| &out[..n]));
                out.iter_mut().for_each(|b| unsafe { core::ptr::write_volatile(b, 0) });
                sent
            }
            Ok((api::Request::Send { session, data }, call)) => api::reply_send(call, service.send(session, pid, data)),
            Ok((api::Request::Receive { session, length }, call)) => {
                let result = service.receive(session, pid, length, &mut buffer);
                api::reply_receive(call, result.map(|n| &buffer[..n]))
            }
            Ok((api::Request::Close { session }, call)) => api::reply_close(call, service.close(session, pid)),
            Ok((api::Request::Listen { session, port, backlog }, call)) => {
                // The server role presents the device: not for a client badged BADGE_TLS_NO_DEVICE (1.3).
                let result = if badge & BADGE_TLS_NO_DEVICE != 0 {
                    mind::println!("[TLS] PID {}: THE SERVER ROLE REFUSED TO A CLIENT WITHOUT THE RIGHT TO THE DEVICE CERTIFICATE", pid);
                    Err(Error::Denied)
                } else { service.listen(session, pid, port, backlog) };
                api::reply_listen(call, result)
            }
            Ok((api::Request::Accept { listener }, call)) => api::reply_accept(call, service.accept(listener, pid).as_ref().map_err(|e| *e)),
            Ok((api::Request::Served { session }, call)) => api::reply_served(call, service.served(session, pid).as_ref().map_err(|e| *e)),
            Ok((api::Request::Certificate, call)) => {
                let result = match mind::idl::keystore::certificate(KEYS, &mut buffer[..]) { Ok(Ok(n)) => Ok(&buffer[..n]), Ok(Err(_)) | Err(_) => Err(Error::NotFound) };
                api::reply_certificate(call, result)
            }
        };
    }
}
