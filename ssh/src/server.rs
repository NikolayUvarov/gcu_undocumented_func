//! The server's side after the transport (112-NET-0071): public-key logins (RFC 4252) with ssh-ed25519 keys the owner
//! lists, each with what it may do; one session channel at a time (RFC 4254) with a console (`shell` or `exec`) or the
//! SFTP subsystem (version 3, draft-ietf-secsh-filexfer-02) over the owner's files. No passwords, no
//! keyboard-interactive, no port forwarding, no agent or X11. The I/O, the host key, the console and the files are the
//! owner's (`Io`, `Keys`, `Owner`, `Files`), so `cargo test` runs it on the host against OpenSSH's client.
use crate::login::{self, USERAUTH_REQUEST};
use crate::transport::{Transport, DISCONNECT};
use crate::wire::{Reader, Writer};
use crate::{Error, Io, Keys};
use alloc::string::{String, ToString};
use alloc::vec::Vec;

const SERVICE_REQUEST: u8 = 5;
const SERVICE_ACCEPT: u8 = 6;
const USERAUTH_FAILURE: u8 = 51;
const USERAUTH_SUCCESS: u8 = 52;
const USERAUTH_PK_OK: u8 = 60;
const CHANNEL_OPEN: u8 = 90;
const CHANNEL_OPEN_CONFIRMATION: u8 = 91;
const CHANNEL_OPEN_FAILURE: u8 = 92;
const CHANNEL_WINDOW_ADJUST: u8 = 93;
const CHANNEL_DATA: u8 = 94;
const CHANNEL_EXTENDED_DATA: u8 = 95;
const CHANNEL_EOF: u8 = 96;
const CHANNEL_CLOSE: u8 = 97;
const CHANNEL_REQUEST: u8 = 98;
const CHANNEL_SUCCESS: u8 = 99;
const CHANNEL_FAILURE: u8 = 100;

/// Logins tried on one connection before it is ended.
pub const ATTEMPTS: u32 = 6;
/// The window the server gives the client, and the largest packet it takes.
const WINDOW: u32 = 2 * 1024 * 1024;
const PACKET: u32 = 32 * 1024;
/// The longest console line and SFTP message taken.
pub const LINE_MAX: usize = 1024;
pub const SFTP_MAX: usize = 256 * 1024;

/// What a key may do once logged in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rights { pub console: bool, pub files: bool }

/// What a session did, for the owner's log (MC-10.3): never a session's data.
#[derive(Debug)]
pub enum Event<'a> {
    /// A login: the user and the SHA-256 of its key's blob (OpenSSH's fingerprint).
    Login { user: &'a [u8], key: [u8; 32] },
    /// A login refused, and why.
    Refused { user: &'a [u8], why: &'static str },
    Command { line: &'a str },
    Wrote { path: &'a str },
    Removed { path: &'a str },
    /// The session's end, and why.
    End { why: &'static str },
}

/// A file's attributes as SFTP gives them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Attrs { pub size: u64, pub directory: bool, pub modified: u32 }

/// SFTP's status codes the files answer with.
pub const NO_SUCH_FILE: u32 = 2;
pub const PERMISSION_DENIED: u32 = 3;
pub const FAILURE: u32 = 4;
const OK: u32 = 0;
const EOF: u32 = 1;
const BAD_MESSAGE: u32 = 5;
const OP_UNSUPPORTED: u32 = 8;

/// The files the SFTP subsystem reaches, by normalized paths below its root (`/` is the root; no `.`, `..` or empty
/// name in them). An error is an SFTP status code.
pub trait Files {
    fn open(&mut self, path: &str, write: bool, create: bool, truncate: bool) -> Result<u32, u32>;
    /// Up to `out.len()` bytes at `offset`; 0 at the end.
    fn read(&mut self, handle: u32, offset: u64, out: &mut [u8]) -> Result<usize, u32>;
    fn write(&mut self, handle: u32, offset: u64, data: &[u8]) -> Result<(), u32>;
    fn close(&mut self, handle: u32) -> Result<(), u32>;
    fn stat(&mut self, path: &str) -> Result<Attrs, u32>;
    fn fstat(&mut self, handle: u32) -> Result<Attrs, u32>;
    /// A directory's entries, without `.` and `..`.
    fn list(&mut self, path: &str) -> Result<Vec<(String, Attrs)>, u32>;
    fn remove(&mut self, path: &str) -> Result<(), u32>;
    fn mkdir(&mut self, path: &str) -> Result<(), u32>;
    fn rmdir(&mut self, path: &str) -> Result<(), u32>;
    fn rename(&mut self, from: &str, to: &str) -> Result<(), u32>;
}

/// The owner's side of the server.
pub trait Owner {
    /// What ssh-ed25519 key `key` may do as `user`, or None: not listed.
    fn rights(&mut self, user: &[u8], key: &[u8; 32]) -> Option<Rights>;
    /// Runs one console line; what it prints goes to `out`. False: the console ends the session (`exit`).
    fn command(&mut self, line: &str, out: &mut dyn FnMut(&[u8])) -> bool;
    fn prompt(&self) -> &str;
    fn files(&mut self) -> &mut dyn Files;
    fn event(&mut self, event: Event<'_>);
}

/// `path` below the root, from `/` or relative to it: `.` dropped, `..` taken back; None when it would leave the root
/// or holds a byte outside printable ASCII.
pub fn normal(path: &str) -> Option<String> {
    if !path.bytes().all(|b| (0x20..0x7F).contains(&b)) { return None; }
    let mut names: Vec<&str> = Vec::new();
    for name in path.split('/') {
        match name { "" | "." => {}, ".." => { names.pop()?; }, n => names.push(n) }
    }
    let mut out = String::from("/");
    out.push_str(&names.join("/"));
    Some(out)
}

struct Channel { remote: u32, window: u64, packet: u32, received: u32, pty: bool, mode: Mode, input: Vec<u8>, after_cr: bool }

#[derive(PartialEq)]
enum Mode { Idle, Console, Sftp }

struct Server<'o, I: Io> {
    t: Transport<I>,
    random: fn(&mut [u8]) -> bool,
    owner: &'o mut dyn Owner,
    rights: Rights,
    channel: Option<Channel>,
    // SFTP: the open directories' listings, by handle (a handle with the top bit set).
    dirs: Vec<Option<Vec<(String, Attrs)>>>,
}

/// Serves one connection: the key exchange with `host`, a login, then its channels, until the client closes. Its
/// end, and why, goes to the owner's log.
pub fn serve<I: Io>(io: I, host: &mut dyn Keys, owner: &mut dyn Owner, random: fn(&mut [u8]) -> bool) -> Result<(), Error> {
    let mut rnd = |b: &mut [u8]| random(b);
    let t = Transport::accept(io, host, &mut rnd)?;
    let mut s = Server { t, random, owner, rights: Rights { console: false, files: false }, channel: None, dirs: Vec::new() };
    let result = s.login().and_then(|()| s.connection());
    let why = match result { Ok(()) | Err(Error::Closed) | Err(Error::Disconnected) => "closed", Err(Error::Refused) => "refused", Err(Error::Mac) => "bad packet", _ => "protocol" };
    s.owner.event(Event::End { why });
    match result { Err(Error::Closed) | Err(Error::Disconnected) => Ok(()), other => other }
}

impl<I: Io> Server<'_, I> {
    fn send(&mut self, payload: &[u8]) -> Result<(), Error> { let random = self.random; self.t.send(payload, &mut |b| random(b)) }
    fn next(&mut self) -> Result<Vec<u8>, Error> { let random = self.random; self.t.next(&mut |b| random(b)) }

    fn disconnect(&mut self, reason: u32, why: &str) -> Error {
        let mut w = Writer::new(DISCONNECT);
        w.u32(reason).string(why.as_bytes()).string(b"");
        let _ = self.send(&w.data);
        Error::Refused
    }

    // The ssh-userauth service and public-key logins, at most ATTEMPTS of them.
    fn login(&mut self) -> Result<(), Error> {
        let request = self.next()?;
        let mut r = Reader::new(request.get(1..).ok_or(Error::Protocol)?);
        if request[0] != SERVICE_REQUEST || r.string()? != b"ssh-userauth" { return Err(self.disconnect(7, "only ssh-userauth")); }
        let mut accept = Writer::new(SERVICE_ACCEPT);
        accept.string(b"ssh-userauth");
        self.send(&accept.data)?;
        let mut failure = Writer::new(USERAUTH_FAILURE);
        failure.string(b"publickey").bool(false);
        for _ in 0..ATTEMPTS {
            let p = self.next()?;
            if p.first() != Some(&USERAUTH_REQUEST) { return Err(Error::Protocol); }
            let mut r = Reader::new(&p[1..]);
            let (user, service, method) = (r.string()?, r.string()?, r.string()?);
            if service != b"ssh-connection" || !login::user_fits(user) { self.owner.event(Event::Refused { user, why: "not a user or service taken" }); self.send(&failure.data)?; continue; }
            if method != b"publickey" {
                // `none` asks which methods there are; the others (password, keyboard-interactive) are not taken.
                if method != b"none" { self.owner.event(Event::Refused { user, why: "a method other than publickey" }); }
                self.send(&failure.data)?;
                continue;
            }
            let signed = r.bool()?;
            let (algorithm, blob) = (r.string()?, r.string()?);
            let mut key = Reader::new(blob);
            let public: Option<[u8; 32]> = (algorithm == b"ssh-ed25519" && key.string().ok() == Some(&b"ssh-ed25519"[..])).then(|| key.string().ok()?.try_into().ok()).flatten();
            let Some(public) = public else { self.owner.event(Event::Refused { user, why: "not an ssh-ed25519 key" }); self.send(&failure.data)?; continue };
            let rights = self.owner.rights(user, &public);
            if !signed {
                // A query: whether this key would do, before the client signs with it.
                if rights.is_some() { let mut ok = Writer::new(USERAUTH_PK_OK); ok.string(algorithm).string(blob); self.send(&ok.data)?; }
                else { self.owner.event(Event::Refused { user, why: "a key not listed" }); self.send(&failure.data)?; }
                continue;
            }
            let mut sig = Reader::new(r.string()?);
            let verified = sig.string().ok() == Some(&b"ssh-ed25519"[..]) && sig.string().ok().and_then(|s| <[u8; 64]>::try_from(s).ok()).is_some_and(|s| {
                let data = login::signed_data(&self.t.session_id, user, &public);
                ed25519_dalek::VerifyingKey::from_bytes(&public).is_ok_and(|k| k.verify_strict(&data, &ed25519_dalek::Signature::from_bytes(&s)).is_ok())
            });
            match (rights, verified) {
                (Some(rights), true) => {
                    use sha2::Digest;
                    self.rights = rights;
                    let user: Vec<u8> = user.to_vec();
                    self.owner.event(Event::Login { user: &user, key: sha2::Sha256::digest(blob).into() });
                    return self.send(&[USERAUTH_SUCCESS]);
                }
                (None, _) => { self.owner.event(Event::Refused { user, why: "a key not listed" }); self.send(&failure.data)?; }
                (Some(_), false) => { self.owner.event(Event::Refused { user, why: "a signature that does not verify" }); self.send(&failure.data)?; }
            }
        }
        Err(self.disconnect(14, "too many logins tried"))
    }

    // The connection: one session channel at a time, until the client disconnects.
    fn connection(&mut self) -> Result<(), Error> {
        loop {
            let p = self.next()?;
            let mut r = Reader::new(p.get(1..).ok_or(Error::Protocol)?);
            match p[0] {
                CHANNEL_OPEN => {
                    let (kind, remote, window, packet) = (r.string()?, r.u32()?, r.u32()?, r.u32()?);
                    if kind != b"session" || self.channel.is_some() {
                        let mut no = Writer::new(CHANNEL_OPEN_FAILURE);
                        no.u32(remote).u32(1).string(b"one session channel at a time, and nothing else").string(b"");
                        self.send(&no.data)?;
                        continue;
                    }
                    self.channel = Some(Channel { remote, window: window as u64, packet: packet.clamp(1024, PACKET), received: 0, pty: false, mode: Mode::Idle, input: Vec::new(), after_cr: false });
                    let mut yes = Writer::new(CHANNEL_OPEN_CONFIRMATION);
                    yes.u32(remote).u32(0).u32(WINDOW).u32(PACKET);
                    self.send(&yes.data)?;
                }
                CHANNEL_WINDOW_ADJUST | CHANNEL_DATA | CHANNEL_EXTENDED_DATA | CHANNEL_EOF | CHANNEL_CLOSE | CHANNEL_REQUEST => {
                    if r.u32()? != 0 || self.channel.is_none() { return Err(Error::Protocol); }
                    self.on_channel(p[0], r)?;
                }
                // Anything else of the connection protocol (a global request is answered by `next`): not taken.
                _ => { let mut no = Writer::new(crate::transport::UNIMPLEMENTED); no.u32(0); self.send(&no.data)?; }
            }
        }
    }

    fn on_channel(&mut self, kind: u8, mut r: Reader<'_>) -> Result<(), Error> {
        match kind {
            CHANNEL_WINDOW_ADJUST => { let n = r.u32()? as u64; if let Some(c) = self.channel.as_mut() { c.window += n; } }
            CHANNEL_DATA => {
                let data = r.string()?.to_vec();
                self.took(data.len())?;
                self.data(&data)?;
            }
            CHANNEL_EXTENDED_DATA => { let _ = r.u32()?; let n = r.string()?.len(); self.took(n)?; }
            CHANNEL_EOF => { if self.channel.as_ref().is_some_and(|c| c.mode != Mode::Idle) { self.end(0)?; } }
            CHANNEL_CLOSE => {
                if let Some(c) = self.channel.take() { let mut close = Writer::new(CHANNEL_CLOSE); close.u32(c.remote); self.send(&close.data)?; }
                self.dirs.clear();
            }
            _ => self.request(r)?,
        }
        Ok(())
    }

    // The client's data used: more window once half of it is.
    fn took(&mut self, n: usize) -> Result<(), Error> {
        let Some(c) = self.channel.as_mut() else { return Ok(()) };
        c.received = c.received.saturating_add(n as u32);
        if c.received < WINDOW / 2 { return Ok(()); }
        let (remote, more) = (c.remote, c.received);
        c.received = 0;
        let mut adjust = Writer::new(CHANNEL_WINDOW_ADJUST);
        adjust.u32(remote).u32(more);
        self.send(&adjust.data)
    }

    fn request(&mut self, mut r: Reader<'_>) -> Result<(), Error> {
        let (kind, reply) = (r.string()?, r.bool()?);
        let Some(c) = self.channel.as_mut() else { return Err(Error::Protocol) };
        let idle = c.mode == Mode::Idle;
        let mut line = None;
        let taken = match kind {
            b"pty-req" if idle => { c.pty = true; true }
            b"shell" if idle && self.rights.console => { c.mode = Mode::Console; true }
            b"exec" if idle && self.rights.console => {
                // One console line, no longer than a typed one.
                line = core::str::from_utf8(r.string()?).ok().filter(|l| l.len() <= LINE_MAX).map(|l| l.trim().to_string());
                if line.is_some() { c.mode = Mode::Console; }
                line.is_some()
            }
            b"subsystem" if idle && self.rights.files => { let name = r.string()?; if name == b"sftp" { c.mode = Mode::Sftp; true } else { false } }
            _ => false,
        };
        let remote = c.remote;
        if reply { let mut w = Writer::new(if taken { CHANNEL_SUCCESS } else { CHANNEL_FAILURE }); w.u32(remote); self.send(&w.data)?; }
        match kind {
            b"shell" if taken => { let prompt = self.owner.prompt().as_bytes().to_vec(); self.output(&prompt) }
            b"exec" if taken => {
                self.run(&line.unwrap_or_default())?;
                self.end(0)
            }
            _ => Ok(()),
        }
    }

    // A channel's data, as its mode takes it.
    fn data(&mut self, data: &[u8]) -> Result<(), Error> {
        let Some(c) = self.channel.as_mut() else { return Ok(()) };
        match c.mode {
            Mode::Idle => Ok(()),
            Mode::Sftp => {
                c.input.extend_from_slice(data);
                if c.input.len() > SFTP_MAX + 4 { return Err(Error::Protocol); }
                loop {
                    let Some(c) = self.channel.as_mut() else { return Ok(()) };
                    if c.input.len() < 4 { return Ok(()); }
                    let length = u32::from_be_bytes([c.input[0], c.input[1], c.input[2], c.input[3]]) as usize;
                    if length == 0 || length > SFTP_MAX { return Err(Error::Protocol); }
                    if c.input.len() < 4 + length { return Ok(()); }
                    let message: Vec<u8> = c.input.drain(..4 + length).skip(4).collect();
                    let answer = self.sftp(&message);
                    let mut framed = Vec::with_capacity(4 + answer.len());
                    framed.extend_from_slice(&(answer.len() as u32).to_be_bytes());
                    framed.extend_from_slice(&answer);
                    self.output_raw(&framed)?;
                }
            }
            Mode::Console => self.console(data),
        }
    }

    // A console's keys: with a terminal, each echoed and a line edited; without one, lines as they come.
    fn console(&mut self, data: &[u8]) -> Result<(), Error> {
        for &b in data {
            let Some(c) = self.channel.as_mut() else { return Ok(()) };
            let pty = c.pty;
            let after_cr = core::mem::replace(&mut c.after_cr, b == b'\r');
            match b {
                b'\r' | b'\n' => {
                    // A terminal's Enter is CR, or CR LF: the LF after a CR ends no second line.
                    if b == b'\n' && after_cr { continue; }
                    let line = String::from_utf8_lossy(&core::mem::take(&mut c.input)).trim().to_string();
                    if pty { self.output_raw(b"\r\n")?; }
                    if !self.run(&line)? { return self.end(0); }
                    let prompt = self.owner.prompt().as_bytes().to_vec();
                    self.output(&prompt)?;
                }
                0x03 => { c.input.clear(); if pty { self.output_raw(b"^C\r\n")?; } let prompt = self.owner.prompt().as_bytes().to_vec(); self.output(&prompt)?; }
                0x04 if c.input.is_empty() => return self.end(0),
                0x7F | 0x08 => { if c.input.pop().is_some() && pty { self.output_raw(b"\x08 \x08")?; } }
                0x20..=0x7E if c.input.len() < LINE_MAX => { c.input.push(b); if pty { self.output_raw(&[b])?; } }
                _ => {}
            }
        }
        Ok(())
    }

    // Runs one line; false when the console ends the session.
    fn run(&mut self, line: &str) -> Result<bool, Error> {
        if line.is_empty() { return Ok(true); }
        self.owner.event(Event::Command { line });
        let mut printed = Vec::new();
        let go_on = self.owner.command(line, &mut |out| printed.extend_from_slice(out));
        self.output(&printed)?;
        Ok(go_on)
    }

    // exit-status, EOF and CLOSE: the channel's end, its mode done.
    fn end(&mut self, status: u32) -> Result<(), Error> {
        let Some(c) = self.channel.as_mut() else { return Ok(()) };
        if c.mode == Mode::Idle { return Ok(()); }
        c.mode = Mode::Idle;
        let remote = c.remote;
        let mut exit = Writer::new(CHANNEL_REQUEST);
        exit.u32(remote).string(b"exit-status").bool(false).u32(status);
        self.send(&exit.data)?;
        let mut eof = Writer::new(CHANNEL_EOF);
        eof.u32(remote);
        self.send(&eof.data)?;
        let mut close = Writer::new(CHANNEL_CLOSE);
        close.u32(remote);
        self.send(&close.data)
    }

    // Console output: a terminal's line ends as CR LF.
    fn output(&mut self, data: &[u8]) -> Result<(), Error> {
        if !self.channel.as_ref().is_some_and(|c| c.pty) { return self.output_raw(data); }
        let mut out = Vec::with_capacity(data.len() + data.len() / 16);
        for &b in data { if b == b'\n' { out.push(b'\r'); } out.push(b); }
        self.output_raw(&out)
    }

    // Data to the client within its window and packet size; waits for more window while it is used up.
    fn output_raw(&mut self, mut data: &[u8]) -> Result<(), Error> {
        while !data.is_empty() {
            let Some(c) = self.channel.as_ref() else { return Ok(()) };
            if c.window == 0 {
                let p = self.next()?;
                let mut r = Reader::new(p.get(1..).ok_or(Error::Protocol)?);
                match p[0] {
                    CHANNEL_WINDOW_ADJUST => { if r.u32()? != 0 { return Err(Error::Protocol); } let n = r.u32()? as u64; if let Some(c) = self.channel.as_mut() { c.window += n; } }
                    CHANNEL_CLOSE | CHANNEL_EOF => { self.channel = None; return Ok(()); }
                    _ => {} // the client's data while it does not read: dropped, as the window was its to keep
                }
                continue;
            }
            let n = data.len().min(c.packet as usize).min(c.window as usize);
            let mut w = Writer::new(CHANNEL_DATA);
            w.u32(c.remote).string(&data[..n]);
            if let Some(c) = self.channel.as_mut() { c.window -= n as u64; }
            self.send(&w.data)?;
            data = &data[n..];
        }
        Ok(())
    }
}

mod sftp {
    pub const INIT: u8 = 1;
    pub const VERSION: u8 = 2;
    pub const OPEN: u8 = 3;
    pub const CLOSE: u8 = 4;
    pub const READ: u8 = 5;
    pub const WRITE: u8 = 6;
    pub const LSTAT: u8 = 7;
    pub const FSTAT: u8 = 8;
    pub const SETSTAT: u8 = 9;
    pub const FSETSTAT: u8 = 10;
    pub const OPENDIR: u8 = 11;
    pub const READDIR: u8 = 12;
    pub const REMOVE: u8 = 13;
    pub const MKDIR: u8 = 14;
    pub const RMDIR: u8 = 15;
    pub const REALPATH: u8 = 16;
    pub const STAT: u8 = 17;
    pub const RENAME: u8 = 18;
    pub const STATUS: u8 = 101;
    pub const HANDLE: u8 = 102;
    pub const DATA: u8 = 103;
    pub const NAME: u8 = 104;
    pub const ATTRS: u8 = 105;
    pub const READ_FLAG: u32 = 1;
    pub const WRITE_FLAG: u32 = 2;
    pub const CREATE: u32 = 8;
    pub const TRUNCATE: u32 = 0x10;
    pub const DIRECTORY: u32 = 1 << 31;
    pub const READ_MAX: u32 = 64 * 1024;
}

// Attributes in a request, read past: SIZE, UIDGID, PERMISSIONS, ACMODTIME, EXTENDED.
fn skip_attrs(r: &mut Reader<'_>) -> Result<(), Error> {
    let flags = r.u32()?;
    if flags & 1 != 0 { r.u64()?; }
    if flags & 2 != 0 { r.u32()?; r.u32()?; }
    if flags & 4 != 0 { r.u32()?; }
    if flags & 8 != 0 { r.u32()?; r.u32()?; }
    if flags & 0x8000_0000 != 0 { for _ in 0..r.u32()? { r.string()?; r.string()?; } }
    Ok(())
}

fn put_attrs(w: &mut Writer, a: &Attrs) {
    let permissions = if a.directory { 0o040755 } else { 0o100644 };
    w.u32(1 | 4 | 8).u64(a.size).u32(permissions).u32(a.modified).u32(a.modified);
}

fn handle(r: &mut Reader<'_>) -> Result<u32, Error> { let h = r.string()?; Ok(u32::from_be_bytes(h.try_into().map_err(|_| Error::Protocol)?)) }

impl<I: Io> Server<'_, I> {
    // One SFTP request and its answer.
    fn sftp(&mut self, message: &[u8]) -> Vec<u8> {
        let kind = message[0];
        let mut r = Reader::new(&message[1..]);
        if kind == sftp::INIT { let mut w = Writer::new(sftp::VERSION); w.u32(3); return w.data; }
        let Ok(id) = r.u32() else { return status(0, BAD_MESSAGE) };
        self.sftp_request(kind, id, &mut r).unwrap_or_else(|code| status(id, code))
    }

    fn sftp_request(&mut self, kind: u8, id: u32, r: &mut Reader<'_>) -> Result<Vec<u8>, u32> {
        let path = |r: &mut Reader<'_>| -> Result<String, u32> {
            let p = core::str::from_utf8(r.string().map_err(|_| BAD_MESSAGE)?).map_err(|_| NO_SUCH_FILE)?;
            normal(p).ok_or(PERMISSION_DENIED)
        };
        let bad = |_| BAD_MESSAGE;
        let files = self.owner.files();
        let done = |result: Result<(), u32>| result.map(|()| status(id, OK));
        match kind {
            sftp::REALPATH => {
                let p = path(r)?;
                let mut w = Writer::new(sftp::NAME);
                w.u32(id).u32(1).string(p.as_bytes()).string(p.as_bytes()).u32(0);
                Ok(w.data)
            }
            sftp::STAT | sftp::LSTAT => { let a = files.stat(&path(r)?)?; let mut w = Writer::new(sftp::ATTRS); w.u32(id); put_attrs(&mut w, &a); Ok(w.data) }
            sftp::FSTAT => { let a = files.fstat(handle(r).map_err(bad)?)?; let mut w = Writer::new(sftp::ATTRS); w.u32(id); put_attrs(&mut w, &a); Ok(w.data) }
            sftp::OPEN => {
                let p = path(r)?;
                let flags = r.u32().map_err(bad)?;
                skip_attrs(r).map_err(bad)?;
                let write = flags & sftp::WRITE_FLAG != 0;
                if flags & (sftp::READ_FLAG | sftp::WRITE_FLAG) == 0 { return Err(BAD_MESSAGE); }
                let h = files.open(&p, write, flags & sftp::CREATE != 0, flags & sftp::TRUNCATE != 0)?;
                if write { self.owner.event(Event::Wrote { path: &p }); }
                let mut w = Writer::new(sftp::HANDLE);
                w.u32(id).string(&h.to_be_bytes());
                Ok(w.data)
            }
            sftp::READ => {
                let (h, offset, length) = (handle(r).map_err(bad)?, r.u64().map_err(bad)?, r.u32().map_err(bad)?);
                let mut buffer = alloc::vec![0u8; length.min(sftp::READ_MAX) as usize];
                let n = files.read(h, offset, &mut buffer)?;
                if n == 0 && !buffer.is_empty() { return Err(EOF); }
                let mut w = Writer::new(sftp::DATA);
                w.u32(id).string(&buffer[..n]);
                Ok(w.data)
            }
            sftp::WRITE => { let (h, offset) = (handle(r).map_err(bad)?, r.u64().map_err(bad)?); let data = r.string().map_err(bad)?; done(files.write(h, offset, data)) }
            sftp::CLOSE => {
                let h = handle(r).map_err(bad)?;
                if h & sftp::DIRECTORY != 0 { return match self.dirs.get_mut((h & !sftp::DIRECTORY) as usize).and_then(Option::take) { Some(_) => Ok(status(id, OK)), None => Err(FAILURE) }; }
                done(files.close(h))
            }
            sftp::OPENDIR => {
                let listing = files.list(&path(r)?)?;
                let slot = self.dirs.iter().position(Option::is_none).unwrap_or(self.dirs.len());
                if slot >= 16 { return Err(FAILURE); }
                if slot == self.dirs.len() { self.dirs.push(None); }
                self.dirs[slot] = Some(listing);
                let mut w = Writer::new(sftp::HANDLE);
                w.u32(id).string(&(slot as u32 | sftp::DIRECTORY).to_be_bytes());
                Ok(w.data)
            }
            sftp::READDIR => {
                let h = handle(r).map_err(bad)?;
                let listing = self.dirs.get_mut((h & !sftp::DIRECTORY) as usize).and_then(Option::as_mut).ok_or(FAILURE)?;
                if listing.is_empty() { return Err(EOF); }
                let batch: Vec<(String, Attrs)> = listing.drain(..listing.len().min(64)).collect();
                let mut w = Writer::new(sftp::NAME);
                w.u32(id).u32(batch.len() as u32);
                for (name, a) in &batch {
                    let long = alloc::format!("{} 1 mind mind {:>10} {}", if a.directory { "drwxr-xr-x" } else { "-rw-r--r--" }, a.size, name);
                    w.string(name.as_bytes()).string(long.as_bytes());
                    put_attrs(&mut w, a);
                }
                Ok(w.data)
            }
            sftp::REMOVE => { let p = path(r)?; let result = files.remove(&p); if result.is_ok() { self.owner.event(Event::Removed { path: &p }); } done(result) }
            sftp::MKDIR => { let p = path(r)?; skip_attrs(r).map_err(bad)?; done(files.mkdir(&p)) }
            sftp::RMDIR => { let p = path(r)?; let result = files.rmdir(&p); if result.is_ok() { self.owner.event(Event::Removed { path: &p }); } done(result) }
            sftp::RENAME => { let (from, to) = (path(r)?, path(r)?); let result = files.rename(&from, &to); if result.is_ok() { self.owner.event(Event::Wrote { path: &to }); } done(result) }
            // Times and permissions are not kept: taken, so that scp's and sftp's own SETSTAT succeed.
            sftp::SETSTAT | sftp::FSETSTAT => Ok(status(id, OK)),
            _ => Err(OP_UNSUPPORTED),
        }
    }
}

fn status(id: u32, code: u32) -> Vec<u8> {
    let text: &[u8] = match code { OK => b"ok", EOF => b"end of file", NO_SUCH_FILE => b"no such file", PERMISSION_DENIED => b"permission denied", BAD_MESSAGE => b"bad message", OP_UNSUPPORTED => b"not supported", _ => b"failure" };
    let mut w = Writer::new(sftp::STATUS);
    w.u32(id).u32(code).string(text).string(b"en");
    w.data
}
