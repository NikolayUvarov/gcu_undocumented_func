#![no_std]
#![no_main]
// SSH service `ssh` (351-NET-0027, Appendix B.6): files read over SFTP for programs, through flows they lend
// (idl/ssh.wit). The protocol is the `ssh` crate (351-NET-0026): curve25519-sha256 with strict key exchange, a server
// known by the SHA-256 of its ssh-ed25519 host key alone, chacha20-poly1305, one SFTP channel. The login is the device
// key's: the key service signs each one (`ssh-login`, idl/keystore.wit 1.2), so this service never holds the key.
// It has no network access of its own: every connection goes through the client's socket capability. Random bytes come
// from RDRAND only: without it every connection is refused. Holds: the key service's client with BADGE_KEY_SSH (slot 4).
extern crate alloc;

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, AtomicU8, Ordering};

use mind::abi::BootInfo;
use mind::idl::keystore::{self, Purpose};
use mind::idl::socket;
use mind::idl::ssh::{self as api, Error};
use mind::idl::wire;
use mind::ipc::{self, Endpoint};

const RECEIVED: usize = 9;
const KEYS: Endpoint = Endpoint(4);
const SESSIONS: usize = 4;
const FILES: usize = 8; // open files per session
const CHUNK: usize = 4096; // the most one socket call moves
const READ_MAX: usize = 32 * 1024; // idl/ssh.wit `read`
const WAIT_MS: u64 = 15_000; // longest wait for the server within one request

// The deadline of the request being served, and why the flow failed last (an api::Error code; 0xFF: nothing).
static DEADLINE: AtomicU64 = AtomicU64::new(0);
static FLOW_ERROR: AtomicU8 = AtomicU8::new(0xFF);

fn now() -> u64 { mind::time::uptime_ms() as u64 }

fn socket_error(error: socket::Error) -> Error {
    match error {
        socket::Error::NoNetwork | socket::Error::Unreachable => Error::NoNetwork,
        socket::Error::Refused => Error::Refused,
        socket::Error::Timeout | socket::Error::Again => Error::Timeout,
        socket::Error::Closed => Error::Closed,
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

// An I/O failure for the protocol, its reason kept for the client.
fn io_failed(error: Error) -> ssh::Error { FLOW_ERROR.store(error as u8, Ordering::Relaxed); ssh::Error::Io }

fn ssh_error(error: ssh::Error) -> Error {
    match error {
        ssh::Error::Io => Error::from_code(FLOW_ERROR.swap(0xFF, Ordering::Relaxed) as usize).unwrap_or(Error::Closed),
        ssh::Error::Closed | ssh::Error::Disconnected => Error::Closed,
        ssh::Error::Protocol | ssh::Error::Algorithms | ssh::Error::Signature | ssh::Error::Mac | ssh::Error::Rekey => Error::Protocol,
        ssh::Error::HostKey => Error::HostKey,
        ssh::Error::Refused => Error::Refused,
        ssh::Error::Status(2) => Error::NotFound,
        ssh::Error::Status(3) => Error::Permission,
        ssh::Error::Status(_) => Error::Failed,
        ssh::Error::NoKey => Error::NoKey,
        ssh::Error::Random => Error::NoEntropy,
    }
}

/// The TCP connection under a session, on the client's flow.
struct Link { flow: Endpoint, socket: u32 }

impl ssh::Io for Link {
    fn send(&mut self, mut data: &[u8]) -> Result<(), ssh::Error> {
        while !data.is_empty() {
            let taken = flow(socket::tcp_send(self.flow, self.socket, &data[..data.len().min(CHUNK)])).map_err(io_failed)? as usize;
            data = &data[taken.min(data.len())..];
            if taken == 0 {
                if now() > DEADLINE.load(Ordering::Relaxed) { return Err(io_failed(Error::Timeout)); }
                mind::time::sleep(2);
            }
        }
        Ok(())
    }

    fn receive(&mut self, buffer: &mut [u8]) -> Result<usize, ssh::Error> {
        let length = buffer.len().min(CHUNK);
        loop {
            match flow(socket::tcp_receive(self.flow, self.socket, length as u32, &mut buffer[..length])) {
                Ok(n) if n > 0 => return Ok(n),
                Err(Error::Closed) => return Ok(0),
                Ok(_) | Err(Error::Timeout) if now() <= DEADLINE.load(Ordering::Relaxed) => { mind::time::sleep(2); }
                Ok(_) | Err(Error::Timeout) => return Err(io_failed(Error::Timeout)),
                Err(error) => return Err(io_failed(error)),
            }
        }
    }
}

impl Drop for Link { fn drop(&mut self) { let _ = socket::close(self.flow, self.socket); } }

/// The device key, as the key service gives it: its public half, and a signature for each login.
struct Device { public: [u8; 32] }

impl ssh::Keys for Device {
    fn public(&self) -> [u8; 32] { self.public }
    fn sign(&mut self, data: &[u8]) -> Option<[u8; 64]> {
        let mut signature = [0u8; 64];
        match keystore::sign(KEYS, Purpose::SshLogin, data, &mut signature) { Ok(Ok(64)) => Some(signature), _ => None }
    }
}

fn random(out: &mut [u8]) -> bool { mind::random::fill(out) }

struct Session {
    id: u32,
    owner: u64,
    flow: usize,
    peer: (u32, u16),
    ssh: Option<ssh::Session<Link>>,
    files: Vec<(u32, ssh::Handle)>,
    next_file: u32,
}

struct Service { sessions: Vec<Box<Session>>, next: u32 }

fn dotted(address: u32) -> [u8; 4] { address.to_be_bytes() }

impl Service {
    fn session(&mut self, id: u32, pid: u64) -> Result<&mut Session, Error> {
        self.sessions.iter_mut().find(|s| s.id == id && s.owner == pid).map(|s| &mut **s).ok_or(Error::NotFound)
    }

    fn attach(&mut self, flow: usize, pid: u64) -> Result<u32, Error> {
        if self.sessions.len() >= SESSIONS { return Err(Error::Limit); }
        // A child of the lent capability: the client's revocation removes it too.
        let flow = ipc::mint(flow, u8::MAX, 0, 0).map_err(|_| Error::Limit)?;
        self.next = self.next.wrapping_add(1).max(1);
        self.sessions.push(Box::new(Session { id: self.next, owner: pid, flow, peer: (0, 0), ssh: None, files: Vec::new(), next_file: 0 }));
        Ok(self.next)
    }

    fn connect(&mut self, id: u32, pid: u64, address: u32, port: u16, user: &str, pin: &[u8], timeout: u32) -> Result<(), Error> {
        let pin: [u8; 32] = pin.try_into().map_err(|_| Error::Invalid)?;
        if !ssh::login::user_fits(user.as_bytes()) { return Err(Error::Invalid); }
        let session = self.session(id, pid)?;
        if session.ssh.is_some() { return Err(Error::Invalid); }
        let a = dotted(address);
        if !mind::random::available() {
            mind::println!("[SSH] SESSION {} OF PID {}: NO {}: REFUSED", id, pid, mind::random::SOURCE);
            return Err(Error::NoEntropy);
        }
        let public = match keystore::public_key(KEYS) {
            Ok(Ok(key)) if key.as_slice().len() == 32 => key.as_slice().try_into().unwrap_or([0; 32]),
            _ => { mind::println!("[SSH] SESSION {} OF PID {}: NO DEVICE KEY", id, pid); return Err(Error::NoKey); }
        };
        let timeout = timeout.clamp(100, 60_000);
        DEADLINE.store(now() + timeout as u64, Ordering::Relaxed);
        let endpoint = Endpoint(session.flow);
        let socket = flow(socket::tcp_connect(endpoint, address, port, timeout))
            .inspect_err(|e| mind::println!("[SSH] SESSION {} OF PID {}: {}.{}.{}.{}:{} NOT REACHED: {:?}", id, pid, a[0], a[1], a[2], a[3], port, e))?;
        let link = Link { flow: endpoint, socket };
        match ssh::connect(link, user, &pin, &mut Device { public }, random) {
            Ok(ssh) => {
                mind::println!("[SSH] SESSION {} OF PID {}: {}.{}.{}.{}:{} BY ITS PINNED HOST KEY, LOGGED IN AS {} WITH THE DEVICE KEY", id, pid, a[0], a[1], a[2], a[3], port, user);
                session.ssh = Some(ssh);
                session.peer = (address, port);
                Ok(())
            }
            Err(error) => {
                let why = match error {
                    ssh::Error::HostKey => "REFUSED: NOT THE PINNED HOST KEY, NOTHING SIGNED",
                    ssh::Error::Refused => "THE SERVER REFUSED THE LOGIN OR SFTP",
                    ssh::Error::NoKey => "NO SIGNATURE FROM THE KEY SERVICE",
                    _ => "FAILED",
                };
                let error = ssh_error(error);
                mind::println!("[SSH] SESSION {} OF PID {}: {}.{}.{}.{}:{} {} ({:?})", id, pid, a[0], a[1], a[2], a[3], port, why, error);
                Err(error)
            }
        }
    }

    // Runs `step` on a session's connection within WAIT_MS; a failure other than an SFTP status ends the connection.
    fn with<T>(&mut self, id: u32, pid: u64, step: impl FnOnce(&mut ssh::Session<Link>, &mut Vec<(u32, ssh::Handle)>) -> Result<T, ssh::Error>) -> Result<T, Error> {
        let session = self.session(id, pid)?;
        let Session { ssh, files, .. } = session;
        let connection = ssh.as_mut().ok_or(Error::Closed)?;
        DEADLINE.store(now() + WAIT_MS, Ordering::Relaxed);
        match step(connection, files) {
            Ok(value) => Ok(value),
            Err(error @ ssh::Error::Status(_)) => Err(ssh_error(error)),
            Err(error) => {
                let error = ssh_error(error);
                let a = dotted(session.peer.0);
                mind::println!("[SSH] SESSION {} OF PID {}: {}.{}.{}.{}:{} LOST ({:?})", id, pid, a[0], a[1], a[2], a[3], session.peer.1, error);
                session.files.clear();
                session.ssh = None;
                Err(error)
            }
        }
    }

    fn open(&mut self, id: u32, pid: u64, path: &str) -> Result<u32, Error> {
        if self.session(id, pid)?.files.len() >= FILES { return Err(Error::Limit); }
        let handle = self.with(id, pid, |ssh, _| ssh.open(path))?;
        let session = self.session(id, pid)?;
        session.next_file = session.next_file.wrapping_add(1).max(1);
        session.files.push((session.next_file, handle));
        Ok(session.next_file)
    }

    fn size(&mut self, id: u32, pid: u64, file: u32) -> Result<u64, Error> {
        let size = self.with(id, pid, |ssh, files| match files.iter().find(|(n, _)| *n == file) {
            Some((_, handle)) => ssh.size(handle).map(Some),
            None => Ok(None),
        })?;
        size.flatten().ok_or(Error::NotFound)
    }

    fn read(&mut self, id: u32, pid: u64, file: u32, offset: u64, length: u32, out: &mut [u8; READ_MAX]) -> Result<usize, Error> {
        let want = (length as usize).min(READ_MAX);
        self.with(id, pid, |ssh, files| match files.iter().find(|(n, _)| *n == file) {
            Some((_, handle)) => ssh.read(handle, offset, &mut out[..want]).map(Some),
            None => Ok(None),
        })?.ok_or(Error::NotFound)
    }

    fn close_file(&mut self, id: u32, pid: u64, file: u32) -> Result<(), Error> {
        let session = self.session(id, pid)?;
        let index = session.files.iter().position(|(n, _)| *n == file).ok_or(Error::NotFound)?;
        let (_, handle) = session.files.swap_remove(index);
        self.with(id, pid, |ssh, _| ssh.close(handle))
    }

    fn end(&mut self, index: usize) {
        let session = self.sessions.swap_remove(index);
        let flow = session.flow;
        // The connection's socket closes with it, then the lent flow goes.
        drop(session);
        let _ = ipc::drop_cap(flow);
    }

    fn close(&mut self, id: u32, pid: u64) -> Result<(), Error> {
        let index = self.sessions.iter().position(|s| s.id == id && s.owner == pid).ok_or(Error::NotFound)?;
        self.end(index);
        Ok(())
    }

    // Sessions of processes that ended.
    fn expire(&mut self) {
        while let Some(index) = self.sessions.iter().position(|s| !mind::process::alive(s.owner)) {
            mind::println!("[SSH] PROCESS {} ENDED: SESSION {} CLOSED", self.sessions[index].owner, self.sessions[index].id);
            self.end(index);
        }
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut service = Service { sessions: Vec::new(), next: 0 };
    let (entropy, outcome) = if mind::random::available() { ("RANDOM FROM ", "") } else { ("NO ", ": EVERY CONNECTION WILL BE REFUSED") };
    mind::println!("[SSH] READY: SFTP READS, LOGINS SIGNED BY THE KEY SERVICE, {}{}{}", entropy, mind::random::SOURCE, outcome);
    let mut scratch = Box::new([0u8; api::REQUEST_MAX]);
    let mut buffer = Box::new([0u8; READ_MAX]);
    let mut checked = 0u64;
    loop {
        if now() - checked >= 1000 { checked = now(); service.expire(); }
        let Ok(request) = Endpoint::SERVICE.recv_timeout(RECEIVED, 1000) else { continue };
        if !request.is_call { continue; }
        let pid = request.sender;
        let _ = match api::decode(&request, RECEIVED, &mut scratch) {
            Err(reason) => wire::reject(reason),
            Ok((api::Request::Attach { flow }, call)) => api::reply_attach(call, service.attach(flow, pid)),
            Ok((api::Request::Connect { session, address, port, user, pin, timeout_ms }, call)) => {
                api::reply_connect(call, service.connect(session, pid, address, port, user.as_str(), pin, timeout_ms))
            }
            Ok((api::Request::Open { session, path }, call)) => api::reply_open(call, service.open(session, pid, path.as_str())),
            Ok((api::Request::Size { session, file }, call)) => api::reply_size(call, service.size(session, pid, file)),
            Ok((api::Request::Read { session, file, offset, length }, call)) => {
                let result = service.read(session, pid, file, offset, length, &mut buffer);
                api::reply_read(call, result.map(|n| &buffer[..n]))
            }
            Ok((api::Request::CloseFile { session, file }, call)) => api::reply_close_file(call, service.close_file(session, pid, file)),
            Ok((api::Request::Close { session }, call)) => api::reply_close(call, service.close(session, pid)),
        };
    }
}
