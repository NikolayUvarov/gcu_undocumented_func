#![no_std]
#![no_main]
// TPM service (351-DRV-0015, idl/tpm.wit, MC-11.9): the only holder of the TPM 2.0's registers, which init gives it from
// the firmware's tables (SLOT_DEV0; without them it answers no-tpm). It speaks the CRB or the FIFO interface, locality
// 0, one command at a time, and offers its clients sealing, not raw commands: a secret sealed under the storage key the
// TPM derives from its seed (libmind/src/tpm.rs) opens only in this TPM. Seal and unseal need mind::tpm::BADGE_SEAL
// (the key service's client); every seal, unseal and refusal is logged with the client's PID.
use mind::abi::{BootInfo, CAP_KIND_MMIO, SLOT_DEV0};
use mind::dev::Mmio;
use mind::idl::tpm::{self as api, Error, Info};
use mind::idl::wire;
use mind::ipc::Endpoint;
use mind::tpm::{self, BUFFER};

const RECEIVED: usize = 9;
const COMMAND_MS: usize = 120_000; // the longest a command may take (key generation in a slow TPM)

fn now() -> usize { mind::time::uptime_ms() }

// Waits until `done`, or gives up after `ms`.
fn wait(ms: usize, mut done: impl FnMut() -> bool) -> bool {
    let deadline = now() + ms;
    while !done() { if now() > deadline { return false; } mind::time::sleep(1); }
    true
}

// CRB registers of locality 0 (TCG PC Client Platform TPM Profile, 6.5).
const LOC_CTRL: usize = 0x08; const LOC_STS: usize = 0x0C; const INTERFACE_ID: usize = 0x30; const CTRL_REQ: usize = 0x40;
const CTRL_START: usize = 0x4C; const CMD_SIZE: usize = 0x58; const CMD_LADDR: usize = 0x5C; const RSP_SIZE: usize = 0x64; const RSP_ADDR: usize = 0x68;
// FIFO registers of locality 0.
const ACCESS: usize = 0x00; const STS: usize = 0x18; const BURST: usize = 0x19; const FIFO: usize = 0x24;
const ACCESS_REQUEST: u8 = 0x02; const ACCESS_ACTIVE: u8 = 0x20;
const STS_VALID: u8 = 0x80; const STS_READY: u8 = 0x40; const STS_GO: u8 = 0x20; const STS_AVAIL: u8 = 0x10;

struct Tpm { regs: Mmio, crb: bool, buffer: [u8; BUFFER] }

impl Tpm {
    // One command and its response, through the CRB: the buffer's offset in our window from its address' low bits.
    fn crb(&mut self, command: &[u8]) -> Option<usize> {
        let r = &self.regs;
        r.write32(LOC_CTRL, 1);
        if !wait(1000, || r.read32(LOC_STS) & 1 != 0) { return None; }
        r.write32(CTRL_REQ, 1); // cmdReady
        let ready = wait(1000, || r.read32(CTRL_REQ) & 1 == 0);
        let (at, size) = (r.read32(CMD_LADDR) as usize & 0xFFF, r.read32(CMD_SIZE) as usize);
        let fits = |at: usize, n: usize| at.checked_add(n).is_some_and(|end| end <= r.len());
        let result = (|| {
            if !ready || command.len() > size || !fits(at, command.len()) { return None; }
            for (i, &byte) in command.iter().enumerate() { r.write8(at + i, byte); }
            r.write32(CTRL_START, 1);
            if !wait(COMMAND_MS, || r.read32(CTRL_START) & 1 == 0) { return None; }
            let (back, room) = (r.read64(RSP_ADDR) as usize & 0xFFF, r.read32(RSP_SIZE) as usize);
            if !fits(back, 10.min(room)) { return None; }
            for i in 0..10 { self.buffer[i] = r.read8(back + i); }
            let n = tpm::response_size(&self.buffer[..10]).filter(|&n| n <= room && fits(back, n))?;
            for i in 10..n { self.buffer[i] = r.read8(back + i); }
            Some(n)
        })();
        r.write32(CTRL_REQ, 2); // goIdle
        r.write32(LOC_CTRL, 2); // relinquish
        result
    }

    // The FIFO: the command in bursts the TPM allows, then the response the same way.
    fn fifo(&mut self, command: &[u8]) -> Option<usize> {
        let r = &self.regs;
        let burst = || r.read8(BURST) as usize | (r.read8(BURST + 1) as usize) << 8;
        r.write8(ACCESS, ACCESS_REQUEST);
        if !wait(1000, || r.read8(ACCESS) & ACCESS_ACTIVE != 0) { return None; }
        let result = (|| {
            r.write8(STS, STS_READY);
            if !wait(1000, || r.read8(STS) & STS_READY != 0) { return None; }
            let mut sent = 0;
            while sent < command.len() {
                let mut room = 0;
                if !wait(1000, || { room = burst(); room > 0 }) { return None; }
                for &byte in &command[sent..(sent + room).min(command.len())] { r.write8(FIFO, byte); }
                sent = (sent + room).min(command.len());
            }
            r.write8(STS, STS_GO);
            if !wait(COMMAND_MS, || r.read8(STS) & (STS_VALID | STS_AVAIL) == STS_VALID | STS_AVAIL) { return None; }
            let mut got = 0;
            let mut total = 10;
            while got < total {
                let mut room = 0;
                if !wait(1000, || { room = burst(); room > 0 }) { return None; }
                for _ in 0..room.min(total - got) { self.buffer[got] = r.read8(FIFO); got += 1; }
                if got >= 10 && total == 10 { total = tpm::response_size(&self.buffer[..10])?; }
            }
            Some(total)
        })();
        r.write8(STS, STS_READY);
        r.write8(ACCESS, ACCESS_ACTIVE); // relinquish
        result
    }

    // A command's response, in self.buffer.
    fn send(&mut self, command: Result<tpm::CommandBytes, tpm::Error>) -> Result<&[u8], Error> {
        let command = command.map_err(|_| Error::Invalid)?;
        let n = if self.crb { self.crb(command.as_bytes()) } else { self.fifo(command.as_bytes()) };
        match n {
            Some(n) => Ok(&self.buffer[..n]),
            None => { mind::println!("[TPM] THE TPM DID NOT ANSWER"); Err(Error::NoTpm) }
        }
    }

    fn refused(error: tpm::Error) -> Error {
        match error {
            tpm::Error::Tpm(code) => { mind::println!("[TPM] THE TPM REFUSED: RESPONSE CODE {:#X}", code); Error::Refused }
            tpm::Error::TooLarge | tpm::Error::Malformed => Error::Invalid,
        }
    }

    // The storage key, made for each request and flushed after it: the TPM gives the same key from the same template, and
    // a restarted service leaves no object loaded behind.
    fn with_primary<T>(&mut self, work: impl FnOnce(&mut Self, u32) -> Result<T, Error>) -> Result<T, Error> {
        let parent = tpm::handle(self.send(tpm::create_primary())?).map_err(Self::refused)?;
        let result = work(self, parent);
        let _ = self.send(tpm::flush(parent));
        result
    }

    fn seal(&mut self, secret: &[u8], blob: &mut [u8]) -> Result<usize, Error> {
        if secret.is_empty() || secret.len() > tpm::SECRET_MAX { return Err(Error::Invalid); }
        self.with_primary(|tpm_, parent| tpm::parse_created(tpm_.send(tpm::create_sealed(parent, secret))?, blob).map_err(Self::refused))
    }

    fn unseal(&mut self, blob: &[u8], secret: &mut [u8]) -> Result<usize, Error> {
        let (private, public) = tpm::split_blob(blob).map_err(|_| Error::Invalid)?;
        self.with_primary(|tpm_, parent| {
            let item = tpm::handle(tpm_.send(tpm::load(parent, private, public))?).map_err(Self::refused)?;
            let result = tpm_.send(tpm::unseal(item)).and_then(|response| tpm::parse_unsealed(response, secret).map_err(Self::refused));
            let _ = tpm_.send(tpm::flush(item));
            result
        })
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut device = if mind::dev::cap_info(SLOT_DEV0).0 == CAP_KIND_MMIO { Mmio::map(SLOT_DEV0).ok() } else { None }.map(|regs| {
        // The interface's type in TPM_INTERFACE_ID: 1 is the CRB; the FIFO otherwise.
        let crb = regs.read32(INTERFACE_ID) & 0xF == 1;
        Tpm { regs, crb, buffer: [0; BUFFER] }
    });
    let info = device.as_mut().and_then(|t| {
        // The firmware may have started it already.
        match t.send(tpm::startup()).map(tpm::Response::parse) { Ok(Ok(_)) | Ok(Err(tpm::Error::Tpm(tpm::RC_INITIALIZE))) => {} _ => return None }
        let manufacturer = tpm::parse_manufacturer(t.send(tpm::manufacturer()).ok()?).ok()?;
        Some(Info { manufacturer, crb: t.crb })
    });
    match &info {
        Some(info) => {
            let name = info.manufacturer.to_be_bytes();
            mind::println!("[TPM] READY: TPM 2.0 BY {}, {} INTERFACE", core::str::from_utf8(&name).unwrap_or("?").trim_matches(|c: char| c == '\0' || c == ' '), if info.crb { "CRB" } else { "FIFO" });
        }
        None if device.is_some() => { mind::println!("[TPM] A TPM THAT DOES NOT ANSWER: NONE USED"); device = None; }
        None => mind::println!("[TPM] NO TPM"),
    }
    let mut scratch = [0u8; api::REQUEST_MAX];
    let mut out = [0u8; 1024];
    loop {
        let Ok(request) = Endpoint::SERVICE.recv(RECEIVED) else { continue };
        if !request.is_call { continue; }
        let (pid, badge) = (request.sender, request.badge);
        let allowed = |what: &str| { let ok = badge & tpm::BADGE_SEAL != 0; if !ok { mind::println!("[TPM] REFUSED {} FOR PID {} (BADGE {})", what, pid, badge); } ok };
        let _ = match api::decode(&request, RECEIVED, &mut scratch) {
            Err(reason) => wire::reject(reason),
            Ok((api::Request::Info, call)) => api::reply_info(call, info.as_ref().ok_or(Error::NoTpm)),
            Ok((api::Request::Seal { secret }, call)) => {
                let result = match device.as_mut() {
                    None => Err(Error::NoTpm),
                    Some(_) if !allowed("SEAL") => Err(Error::Rights),
                    Some(tpm) => tpm.seal(secret, &mut out),
                };
                if let Ok(n) = result { mind::println!("[TPM] SEALED {} BYTES FOR PID {}", secret.len(), pid); api::reply_seal(call, Ok(&out[..n])) } else { api::reply_seal(call, result.map(|_| &out[..0])) }
            }
            Ok((api::Request::Unseal { blob }, call)) => {
                let result = match device.as_mut() {
                    None => Err(Error::NoTpm),
                    Some(_) if !allowed("UNSEAL") => Err(Error::Rights),
                    Some(tpm) => tpm.unseal(blob, &mut out),
                };
                if result.is_ok() { mind::println!("[TPM] UNSEALED FOR PID {}", pid); }
                api::reply_unseal(call, result.map(|n| &out[..n]))
            }
        };
    }
}
