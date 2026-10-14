//! After the transport: public-key login (RFC 4252), one session channel with the `sftp` subsystem (RFC 4254), and SFTP
//! version 3 (draft-ietf-secsh-filexfer-02) to open a file for reading, learn its size, read it and close it.
use crate::login::{self, USERAUTH_REQUEST};
use crate::transport::Transport;
use crate::wire::{Reader, Writer};
use crate::{Error, Io, Keys};
use alloc::vec::Vec;

const SERVICE_REQUEST: u8 = 5;
const SERVICE_ACCEPT: u8 = 6;
const USERAUTH_FAILURE: u8 = 51;
const USERAUTH_SUCCESS: u8 = 52;
const USERAUTH_BANNER: u8 = 53;
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

const FXP_INIT: u8 = 1;
const FXP_VERSION: u8 = 2;
const FXP_OPEN: u8 = 3;
const FXP_CLOSE: u8 = 4;
const FXP_READ: u8 = 5;
const FXP_FSTAT: u8 = 8;
const FXP_STATUS: u8 = 101;
const FXP_HANDLE: u8 = 102;
const FXP_DATA: u8 = 103;
const FXP_ATTRS: u8 = 105;
const FX_OK: u32 = 0;
const FX_EOF: u32 = 1;
const FXF_READ: u32 = 1;
const ATTR_SIZE: u32 = 1;

/// The window this client gives the server, and when it gives more.
const WINDOW: u32 = 2 * 1024 * 1024;
const PACKET: u32 = 32 * 1024;
/// The most a READ asks for: OpenSSH's server answers at most this much.
pub const READ_MAX: usize = 32 * 1024;
/// The longest SFTP message taken.
const SFTP_MAX: usize = READ_MAX + 1024;

/// An open file on the server (its handle, at most 256 bytes).
pub struct Handle(Vec<u8>);

pub struct Session<I: Io> {
    t: Transport<I>,
    random: fn(&mut [u8]) -> bool,
    remote: u32,
    remote_window: u64,
    remote_packet: u32,
    window: u32,
    sftp: Vec<u8>,
    next_id: u32,
}

/// Logs in as `user` with `keys` on the server known by `pin` (the SHA-256 of its host key blob) and starts SFTP.
pub fn connect<I: Io>(io: I, user: &str, pin: &[u8; 32], keys: &mut dyn Keys, random: fn(&mut [u8]) -> bool) -> Result<Session<I>, Error> {
    let mut rnd = |b: &mut [u8]| random(b);
    let mut t = Transport::connect(io, pin, &mut rnd)?;
    let mut request = Writer::new(SERVICE_REQUEST);
    request.string(b"ssh-userauth");
    t.send(&request.data, &mut rnd)?;
    if t.next(&mut rnd)?.first() != Some(&SERVICE_ACCEPT) { return Err(Error::Protocol); }

    // What a public-key login signs (RFC 4252 section 7), then the request carrying the signature.
    let signed = login::signed_data(&t.session_id, user.as_bytes(), &keys.public());
    let signature = keys.sign(&signed).ok_or(Error::NoKey)?;
    let mut blob = Writer::default();
    blob.string(b"ssh-ed25519").string(&signature);
    let mut auth = Writer::new(USERAUTH_REQUEST);
    auth.raw(&signed[4 + t.session_id.len() + 1..]).string(&blob.data);
    t.send(&auth.data, &mut rnd)?;
    loop {
        match t.next(&mut rnd)?.first() {
            Some(&USERAUTH_SUCCESS) => break,
            Some(&USERAUTH_BANNER) => continue,
            Some(&USERAUTH_FAILURE) => return Err(Error::Refused),
            _ => return Err(Error::Protocol),
        }
    }

    let mut open = Writer::new(CHANNEL_OPEN);
    open.string(b"session").u32(0).u32(WINDOW).u32(PACKET);
    t.send(&open.data, &mut rnd)?;
    let (remote, remote_window, remote_packet) = loop {
        let p = t.next(&mut rnd)?;
        let mut r = Reader::new(&p[1..]);
        match p[0] {
            CHANNEL_OPEN_CONFIRMATION => { if r.u32()? != 0 { return Err(Error::Protocol); } break (r.u32()?, r.u32()?, r.u32()?); }
            CHANNEL_OPEN_FAILURE => return Err(Error::Refused),
            _ => return Err(Error::Protocol),
        }
    };
    let mut s = Session { t, random, remote, remote_window: remote_window as u64, remote_packet: remote_packet.max(1024), window: WINDOW, sftp: Vec::new(), next_id: 1 };
    let mut subsystem = Writer::new(CHANNEL_REQUEST);
    subsystem.u32(remote).string(b"subsystem").bool(true).string(b"sftp");
    s.send(&subsystem.data)?;
    loop {
        let p = s.channel_message()?;
        match p[0] {
            CHANNEL_SUCCESS => break,
            CHANNEL_FAILURE => return Err(Error::Refused),
            _ => return Err(Error::Protocol),
        }
    }
    let mut init = Writer::default();
    init.u32(5).u8(FXP_INIT).u32(3);
    s.channel_send(&init.data)?;
    let version = s.sftp_message()?;
    if version.first() != Some(&FXP_VERSION) || Reader::new(&version[1..]).u32()? != 3 { return Err(Error::Protocol); }
    Ok(s)
}

impl<I: Io> Session<I> {
    /// The SHA-256 of the server's host key blob.
    pub fn host_key(&self) -> [u8; 32] { self.t.host_key }

    fn send(&mut self, payload: &[u8]) -> Result<(), Error> { let random = self.random; self.t.send(payload, &mut |b| random(b)) }

    // The next message for the channel; window adjustments are taken on the way, and the window is given back.
    fn channel_message(&mut self) -> Result<Vec<u8>, Error> {
        loop {
            let random = self.random;
            let p = self.t.next(&mut |b| random(b))?;
            let mut r = Reader::new(&p[1..]);
            if p[0] < CHANNEL_WINDOW_ADJUST || p[0] > CHANNEL_FAILURE || r.u32()? != 0 { return Err(Error::Protocol); }
            match p[0] {
                CHANNEL_WINDOW_ADJUST => { self.remote_window += r.u32()? as u64; continue; }
                CHANNEL_EXTENDED_DATA => continue,
                CHANNEL_EOF | CHANNEL_CLOSE => return Err(Error::Closed),
                CHANNEL_REQUEST => {
                    let _ = r.string()?;
                    if r.bool()? { let mut f = Writer::new(CHANNEL_FAILURE); f.u32(self.remote); self.send(&f.data)?; }
                    continue;
                }
                _ => return Ok(p),
            }
        }
    }

    // Sends SFTP bytes in channel data, within the server's window and packet size.
    fn channel_send(&mut self, mut data: &[u8]) -> Result<(), Error> {
        while !data.is_empty() {
            while self.remote_window == 0 {
                let p = self.channel_message()?;
                if p[0] != CHANNEL_DATA { return Err(Error::Protocol); }
                self.take_data(&p)?;
            }
            let n = data.len().min(self.remote_window as usize).min(self.remote_packet as usize);
            let mut msg = Writer::new(CHANNEL_DATA);
            msg.u32(self.remote).string(&data[..n]);
            self.send(&msg.data)?;
            self.remote_window -= n as u64;
            data = &data[n..];
        }
        Ok(())
    }

    // Channel data into the SFTP buffer, the window given back once half of it is used.
    fn take_data(&mut self, p: &[u8]) -> Result<(), Error> {
        let mut r = Reader::new(&p[1..]);
        let _ = r.u32()?;
        let data = r.string()?;
        if data.len() as u32 > self.window || self.sftp.len() + data.len() > 2 * SFTP_MAX { return Err(Error::Protocol); }
        self.window -= data.len() as u32;
        self.sftp.extend_from_slice(data);
        if self.window < WINDOW / 2 {
            let mut adjust = Writer::new(CHANNEL_WINDOW_ADJUST);
            adjust.u32(self.remote).u32(WINDOW - self.window);
            self.send(&adjust.data)?;
            self.window = WINDOW;
        }
        Ok(())
    }

    // The next whole SFTP message (its type first).
    fn sftp_message(&mut self) -> Result<Vec<u8>, Error> {
        loop {
            if self.sftp.len() >= 4 {
                let length = u32::from_be_bytes([self.sftp[0], self.sftp[1], self.sftp[2], self.sftp[3]]) as usize;
                if length == 0 || length > SFTP_MAX { return Err(Error::Protocol); }
                if self.sftp.len() >= 4 + length {
                    let message: Vec<u8> = self.sftp.drain(..4 + length).skip(4).collect();
                    return Ok(message);
                }
            }
            let p = self.channel_message()?;
            if p[0] != CHANNEL_DATA { return Err(Error::Protocol); }
            self.take_data(&p)?;
        }
    }

    // Sends request `kind` with `body` after its id, and gives the answer to that id (its type first, the id taken out).
    fn request(&mut self, kind: u8, body: &[u8]) -> Result<Vec<u8>, Error> {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        let mut msg = Writer::default();
        msg.u32(1 + 4 + body.len() as u32).u8(kind).u32(id).raw(body);
        self.channel_send(&msg.data)?;
        let mut answer = self.sftp_message()?;
        if answer.len() < 5 || u32::from_be_bytes([answer[1], answer[2], answer[3], answer[4]]) != id { return Err(Error::Protocol); }
        answer.drain(1..5);
        Ok(answer)
    }

    fn status(answer: &[u8]) -> Result<u32, Error> {
        if answer.first() != Some(&FXP_STATUS) { return Err(Error::Protocol); }
        Reader::new(&answer[1..]).u32()
    }

    /// Opens `path` for reading.
    pub fn open(&mut self, path: &str) -> Result<Handle, Error> {
        let mut body = Writer::default();
        body.string(path.as_bytes()).u32(FXF_READ).u32(0);
        let answer = self.request(FXP_OPEN, &body.data)?;
        if answer.first() == Some(&FXP_HANDLE) {
            let handle = Reader::new(&answer[1..]).string()?;
            if handle.len() > 256 { return Err(Error::Protocol); }
            return Ok(Handle(handle.to_vec()));
        }
        Err(Error::Status(Self::status(&answer)?))
    }

    /// The file's size, if the server tells it.
    pub fn size(&mut self, file: &Handle) -> Result<Option<u64>, Error> {
        let mut body = Writer::default();
        body.string(&file.0);
        let answer = self.request(FXP_FSTAT, &body.data)?;
        if answer.first() != Some(&FXP_ATTRS) { return Err(Error::Status(Self::status(&answer)?)); }
        let mut r = Reader::new(&answer[1..]);
        let flags = r.u32()?;
        Ok(if flags & ATTR_SIZE != 0 { Some(r.u64()?) } else { None })
    }

    /// Reads from `offset` into `out` (at most READ_MAX): the bytes read, 0 at the end of the file.
    pub fn read(&mut self, file: &Handle, offset: u64, out: &mut [u8]) -> Result<usize, Error> {
        let want = out.len().min(READ_MAX) as u32;
        let mut body = Writer::default();
        body.string(&file.0).u64(offset).u32(want);
        let answer = self.request(FXP_READ, &body.data)?;
        if answer.first() == Some(&FXP_DATA) {
            let data = Reader::new(&answer[1..]).string()?;
            if data.len() > want as usize { return Err(Error::Protocol); }
            out[..data.len()].copy_from_slice(data);
            return Ok(data.len());
        }
        match Self::status(&answer)? { FX_EOF => Ok(0), code => Err(Error::Status(code)) }
    }

    /// Closes the file.
    pub fn close(&mut self, file: Handle) -> Result<(), Error> {
        let mut body = Writer::default();
        body.string(&file.0);
        let answer = self.request(FXP_CLOSE, &body.data)?;
        match Self::status(&answer)? { FX_OK => Ok(()), code => Err(Error::Status(code)) }
    }
}
