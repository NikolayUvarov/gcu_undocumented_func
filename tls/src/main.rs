#![no_std]
#![no_main]
// TLS service (issue 103, Appendix B.6): TLS 1.3 client sessions (rustls, unbuffered API) over flows the clients lend
// (idl/tls.wit). The service has no network access of its own: every connection goes through the client's socket
// capability, so the network policy of the client holds. Server certificates are verified against tlsroots.pem on the
// boot disk, or a server is known by the SHA-256 of its public key (connect-pinned, 351-NET-0002); a client certificate
// is the device certificate, signed for by the key service. Random bytes come from
// RDRAND only: without it every connection is refused. Holds: an RTC client (slot 2, certificate times), a VFS client
// (slot 3, the root store), the key service's signer client (slot 4).
extern crate alloc;

mod device;
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
use rustls::client::{UnbufferedClientConnection, WebPkiServerVerifier};
use rustls::unbuffered::{ConnectionState, EncodeError, UnbufferedStatus};
use rustls::crypto::CryptoProvider;
use rustls::{ClientConfig, RootCertStore};

const RECEIVED: usize = 9;
const KEYS: Endpoint = Endpoint(4);
const ROOTS_FILE: &str = "tlsroots.pem";
const SESSIONS: usize = 8;
const CHUNK: usize = 4096; // largest send or receive (idl/tls.wit)
const IDLE_MS: u64 = 5000; // longest wait for the peer in send, receive and close
const PLAIN_MAX: usize = 64 * 1024; // decrypted bytes held for a client before the socket is read again

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

struct Session {
    id: u32, owner: u64, flow: usize,
    conn: Option<UnbufferedClientConnection>, link: Option<Link>, device: Option<Arc<device::Device>>,
    incoming: Vec<u8>, outgoing: Vec<u8>, plain: VecDeque<u8>, peer_closed: bool,
}

impl Session {
    // One state of the connection handled; `write` is done when the connection can take application data (the bool).
    fn step(&mut self, write: Option<Write>, deadline: u64) -> Result<(Step, bool), Failure> {
        let (Some(conn), Some(link)) = (self.conn.as_mut(), self.link.as_mut()) else { return Err(Error::Invalid.into()) };
        let (incoming, outgoing, plain) = (&mut self.incoming, &mut self.outgoing, &mut self.plain);
        let UnbufferedStatus { mut discard, state } = conn.process_tls_records(&mut incoming[..]);
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
            Ok(ConnectionState::PeerClosed) => { self.peer_closed = true; Ok(Step::Progress) }
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

// The root store from the boot disk, read for every connection so a changed file takes effect.
fn roots() -> RootCertStore {
    let mut store = RootCertStore::empty();
    let Ok(file) = mind::fs::File::open(ROOTS_FILE) else { return store };
    let mut text = alloc::vec![0u8; file.size().min(512 * 1024)];
    let length = file.read_at(0, &mut text).unwrap_or(0);
    let certificates = pem::certificates(core::str::from_utf8(&text[..length]).unwrap_or(""));
    store.add_parsable_certificates(certificates.into_iter().map(pki_types::CertificateDer::from));
    store
}

impl Service {
    fn session(&mut self, id: u32, pid: u64) -> Result<&mut Session, Error> {
        self.sessions.iter_mut().find(|s| s.id == id && s.owner == pid).map(|s| &mut **s).ok_or(Error::NotFound)
    }

    fn attach(&mut self, flow: usize, pid: u64) -> Result<u32, Error> {
        if self.sessions.len() >= SESSIONS { return Err(Error::Limit); }
        // A child of the lent capability: the client's revocation removes it too.
        let flow = ipc::mint(flow, u8::MAX, 0, 0).map_err(|_| Error::Limit)?;
        self.next = self.next.wrapping_add(1).max(1);
        self.sessions.push(Box::new(Session { id: self.next, owner: pid, flow, conn: None, link: None, device: None, incoming: Vec::new(), outgoing: Vec::new(), plain: VecDeque::new(), peer_closed: false }));
        Ok(self.next)
    }

    // The server verified against the root store, or by its pinned key alone.
    fn config(&self, device: Option<Arc<device::Device>>, pin: Option<[u8; 32]>) -> Result<ClientConfig, Error> {
        let builder = ClientConfig::builder_with_details(self.provider.clone(), Arc::new(provider::Rtc))
            .with_protocol_versions(&[&rustls::version::TLS13]).map_err(|_| Error::Handshake)?;
        let builder = match pin {
            Some(pin) => builder.dangerous().with_custom_certificate_verifier(Arc::new(pinned::Pinned { pin, provider: self.provider.clone() })),
            None => {
                let roots = roots();
                if roots.is_empty() { return Err(Error::NoRoots); }
                builder.with_webpki_verifier(WebPkiServerVerifier::builder_with_provider(Arc::new(roots), self.provider.clone()).build().map_err(|_| Error::NoRoots)?)
            }
        };
        Ok(match device { Some(device) => builder.with_client_cert_resolver(device), None => builder.with_no_client_auth() })
    }

    fn connect(&mut self, id: u32, pid: u64, name: &str, address: u32, port: u16, client_certificate: bool, pin: Option<[u8; 32]>, timeout: u32) -> Result<Peer, Error> {
        if !mind::random::available() { return Err(Error::NoEntropy); }
        let server = pki_types::ServerName::try_from(name).map_err(|_| Error::Invalid)?.to_owned();
        let device = if client_certificate { Some(Arc::new(device::Device::new(KEYS).ok_or(Error::Denied)?)) } else { None };
        let config = Arc::new(self.config(device.clone(), pin)?);
        let how = if pin.is_some() { " BY ITS PINNED KEY" } else { "" };
        let session = self.session(id, pid)?;
        if session.conn.is_some() { return Err(Error::Invalid); }
        let deadline = now() + timeout.clamp(100, 60_000) as u64;
        let endpoint = Endpoint(session.flow);
        let socket = flow(socket::tcp_connect(endpoint, address, port, timeout.clamp(100, 60_000)))?;
        session.link = Some(Link { flow: endpoint, socket, eof: false });
        session.device = device;
        let id = session.id;
        let result = match UnbufferedClientConnection::new(config, server) {
            Ok(conn) => { session.conn = Some(conn); session.handshake(deadline) }
            Err(error) => Err(Failure::Tls(error)),
        };
        match result {
            Ok(()) => {
                let conn = session.conn.as_ref().ok_or(Error::Invalid)?;
                let suite = conn.negotiated_cipher_suite().map_or(0, |s| u16::from(s.suite()));
                let group = conn.negotiated_key_exchange_group().map_or(0, |g| u16::from(g.name()));
                let sent = session.device.as_ref().is_some_and(|d| d.sent.load(Ordering::Relaxed));
                mind::println!("[TLS] SESSION {} OF PID {}: {} VERIFIED{}, SUITE {:04X}, GROUP {:04X}, CLIENT CERTIFICATE {}", id, pid, name, how, suite, group, if sent { "SENT" } else { "NOT SENT" });
                Ok(Peer { suite, group, client_certificate: sent })
            }
            Err(failure) => {
                let error = match &failure {
                    Failure::Tls(rustls::Error::InvalidCertificate(rustls::CertificateError::ApplicationVerificationFailure)) if pin.is_some() => {
                        mind::println!("[TLS] SESSION {} OF PID {}: {} REFUSED: NOT THE PINNED KEY", id, pid, name); Error::Certificate
                    }
                    Failure::Tls(rustls::Error::InvalidCertificate(why)) => { mind::println!("[TLS] SESSION {} OF PID {}: {} REFUSED: CERTIFICATE {:?}", id, pid, name, why); Error::Certificate }
                    Failure::Tls(error) => { mind::println!("[TLS] SESSION {} OF PID {}: HANDSHAKE WITH {} FAILED: {:?}", id, pid, name, error); Error::Handshake }
                    Failure::Client(error) => { mind::println!("[TLS] SESSION {} OF PID {}: {} FAILED: {:?}", id, pid, name, error); *error }
                };
                if let Some(link) = session.link.take() { let _ = socket::close(link.flow, link.socket); }
                session.conn = None;
                Err(error)
            }
        }
    }

    fn send(&mut self, id: u32, pid: u64, data: &[u8]) -> Result<u32, Error> {
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

    // Frees session `index`: close_notify if it can still be sent, the TCP connection, the lent flow.
    fn end(&mut self, index: usize) {
        let mut session = self.sessions.swap_remove(index);
        if session.conn.is_some() { let _ = session.pump(Some(Write::CloseNotify), now() + IDLE_MS); }
        if let Some(link) = session.link.take() { let _ = socket::close(link.flow, link.socket); }
        let _ = ipc::drop_cap(session.flow);
    }

    fn close(&mut self, id: u32, pid: u64) -> Result<(), Error> {
        let index = self.sessions.iter().position(|s| s.id == id && s.owner == pid).ok_or(Error::NotFound)?;
        self.end(index);
        Ok(())
    }

    // Sessions of processes that ended.
    fn expire(&mut self) {
        while let Some(index) = self.sessions.iter().position(|s| !mind::process::alive(s.owner)) {
            mind::println!("[TLS] PROCESS {} ENDED: SESSION {} CLOSED", self.sessions[index].owner, self.sessions[index].id);
            self.end(index);
        }
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut service = Service { provider: provider::provider(), sessions: Vec::new(), next: 0 };
    let (entropy, outcome) = if mind::random::available() { ("RANDOM FROM ", "") } else { ("NO ", ": EVERY CONNECTION WILL BE REFUSED") };
    mind::println!("[TLS] READY: TLS 1.3 CLIENT, ROOTS FROM {}, {}{}{}", ROOTS_FILE, entropy, mind::random::SOURCE, outcome);
    let mut scratch = Box::new([0u8; api::REQUEST_MAX]);
    let mut buffer = Box::new([0u8; CHUNK]);
    let mut checked = 0u64;
    loop {
        if now() - checked >= 1000 { checked = now(); service.expire(); }
        let Ok(request) = Endpoint::SERVICE.recv_timeout(RECEIVED, 1000) else { continue };
        if !request.is_call { continue; }
        let pid = request.sender;
        let _ = match api::decode(&request, RECEIVED, &mut scratch) {
            Err(reason) => wire::reject(reason),
            Ok((api::Request::Attach { flow }, call)) => api::reply_attach(call, service.attach(flow, pid)),
            Ok((api::Request::Connect { session, name, address, port, client_certificate, timeout_ms }, call)) => {
                let peer = service.connect(session, pid, name.as_str(), address, port, client_certificate, None, timeout_ms);
                api::reply_connect(call, peer.as_ref().map_err(|e| *e))
            }
            Ok((api::Request::ConnectPinned { session, name, address, port, pin, timeout_ms }, call)) => {
                let peer = match <[u8; 32]>::try_from(pin) {
                    Ok(pin) => service.connect(session, pid, name.as_str(), address, port, false, Some(pin), timeout_ms),
                    Err(_) => Err(Error::Invalid),
                };
                api::reply_connect_pinned(call, peer.as_ref().map_err(|e| *e))
            }
            Ok((api::Request::Send { session, data }, call)) => api::reply_send(call, service.send(session, pid, data)),
            Ok((api::Request::Receive { session, length }, call)) => {
                let result = service.receive(session, pid, length, &mut buffer);
                api::reply_receive(call, result.map(|n| &buffer[..n]))
            }
            Ok((api::Request::Close { session }, call)) => api::reply_close(call, service.close(session, pid)),
            Ok((api::Request::Certificate, call)) => {
                let result = match mind::idl::keystore::certificate(KEYS, &mut buffer[..]) { Ok(Ok(n)) => Ok(&buffer[..n]), Ok(Err(_)) | Err(_) => Err(Error::NotFound) };
                api::reply_certificate(call, result)
            }
        };
    }
}
