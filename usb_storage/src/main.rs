#![no_std]
#![no_main]
// Ring 3 USB mass storage class driver: the first mass storage interface usb_host gives it (idl/usb.wit; its client
// with the storage badge in SLOT_DEV0 can claim only such interfaces, issue 164), Bulk-Only Transport, SCSI READ(10),
// WRITE(10) and SYNCHRONIZE CACHE(10); MODE SENSE(6) tells a write-protected medium. Writes only for clients with the
// write badge (mind::block_protocol). If usb_host restarts, the interface is claimed and set up again and the request
// repeated (reads and rewrites of the same data are idempotent, MC-6.6).
use mind::abi::{BootInfo, BLOCK_KIND_USB, SLOT_DEV0};
use mind::block::{self, Driver};
use mind::ipc::Endpoint;
use mind::usb::Host;

const CBW: usize = 0; const CSW: usize = 64; const DATA: usize = 4096; // in the buffer lent to usb_host
const CLAIM_TRIES: usize = 20; // usb_host may still be setting the device up

// The claimed interface and its Bulk-Only Transport state.
struct Bot { handle: u32, out: u8, input: u8, tag: u32 }

struct Storage { host: Host, bot: Bot, sectors: u64, protected: bool }

// One BOT cycle: CBW on bulk OUT, data in or out (already at DATA when sent), CSW on bulk IN. Returns bytes moved.
fn bot_cycle(host: &mut Host, bot: &mut Bot, command: &[u8], length: usize, send: bool) -> Option<usize> {
    bot.tag = bot.tag.wrapping_add(1);
    let cbw = &mut host.buffer_mut()[CBW..CBW + 31];
    cbw.fill(0);
    cbw[..4].copy_from_slice(b"USBC"); cbw[4..8].copy_from_slice(&bot.tag.to_le_bytes()); cbw[8..12].copy_from_slice(&(length as u32).to_le_bytes());
    cbw[12] = if length > 0 && !send { 0x80 } else { 0 }; cbw[14] = command.len() as u8; cbw[15..15 + command.len()].copy_from_slice(command);
    host.bulk(bot.handle, bot.out, CBW, 31).ok()?;
    let moved = if length == 0 { 0 } else {
        match host.bulk(bot.handle, if send { bot.out } else { bot.input }, DATA, length) {
            Ok(moved) => moved,
            Err(mind::sys::Error::NotFound | mind::sys::Error::Peer) => return None, // the interface is gone
            Err(_) => 0, // a stalled data stage still ends with a CSW
        }
    };
    host.bulk(bot.handle, bot.input, CSW, 13).ok()?;
    let status = &host.buffer()[CSW..CSW + 13];
    (&status[..4] == b"USBS" && status[4..8] == bot.tag.to_le_bytes() && status[12] == 0).then_some(moved)
}

// The next mass storage interface (SCSI over Bulk-Only: 08/06/50) and its bulk endpoints.
fn claim(host: &mut Host) -> Option<Bot> {
    for _ in 0..CLAIM_TRIES {
        match host.claim() {
            Ok((handle, info)) => {
                let out = info.endpoints().iter().find(|e| e.is_bulk() && !e.is_in()).map(|e| e.address);
                let input = info.endpoints().iter().find(|e| e.is_bulk() && e.is_in()).map(|e| e.address);
                if let (6, 0x50, Some(out), Some(input)) = (info.subclass, info.protocol, out, input) { return Some(Bot { handle, out, input, tag: 0 }); }
                let _ = host.release(handle);
            }
            Err(_) => { mind::time::sleep(100); }
        }
    }
    None
}

// TEST UNIT READY (with REQUEST SENSE after "unit attention"), then READ CAPACITY(10); 512-byte sectors required.
fn capacity(host: &mut Host, bot: &mut Bot) -> Option<u64> {
    for _ in 0..5 {
        if bot_cycle(host, bot, &[0x00, 0, 0, 0, 0, 0], 0, false).is_some() { break; }
        let _ = bot_cycle(host, bot, &[0x03, 0, 0, 0, 18, 0], 18, false);
        mind::time::sleep(50);
    }
    bot_cycle(host, bot, &[0x25, 0, 0, 0, 0, 0, 0, 0, 0, 0], 8, false)?;
    let reply = &host.buffer()[DATA..DATA + 8];
    let last = u32::from_be_bytes(reply[..4].try_into().unwrap()) as u64; let size = u32::from_be_bytes(reply[4..8].try_into().unwrap());
    (size == 512).then_some(last + 1)
}

impl Storage {
    fn probe() -> Option<Self> {
        let mut host = Host::new(Endpoint(SLOT_DEV0)).ok()?;
        let mut bot = claim(&mut host)?;
        let sectors = capacity(&mut host, &mut bot)?;
        // MODE SENSE(6), all pages, the 4-byte header: WP is bit 7 of byte 2. A device that does not answer is taken as
        // writable; a write it refuses fails anyway.
        let protected = bot_cycle(&mut host, &mut bot, &[0x1A, 0, 0x3F, 0, 4, 0], 4, false).is_some() && host.buffer()[DATA + 2] & 0x80 != 0;
        Some(Self { host, bot, sectors, protected })
    }

    // A command; if it fails because usb_host lost the interface, the interface is claimed again and it is repeated.
    fn run(&mut self, command: &[u8], length: usize, send: bool) -> Option<usize> {
        if let Some(moved) = bot_cycle(&mut self.host, &mut self.bot, command, length, send) { return Some(moved); }
        let mut bot = claim(&mut self.host)?;
        let sectors = capacity(&mut self.host, &mut bot)?;
        if sectors != self.sectors { return None; } // another medium
        self.bot = bot;
        bot_cycle(&mut self.host, &mut self.bot, command, length, send)
    }
}

impl Driver for Storage {
    fn sectors(&self) -> u64 { self.sectors }
    fn read(&mut self, lba: u64, count: usize, out: &mut [u8]) -> bool {
        let lba = (lba as u32).to_be_bytes(); let blocks = (count as u16).to_be_bytes();
        let command = [0x28, 0, lba[0], lba[1], lba[2], lba[3], 0, blocks[0], blocks[1], 0];
        match self.run(&command, count * 512, false) {
            Some(got) if got == count * 512 => { out[..got].copy_from_slice(&self.host.buffer()[DATA..DATA + got]); true }
            _ => false,
        }
    }
    fn write(&mut self, lba: u64, count: usize, data: &[u8]) -> bool {
        let bytes = count * 512;
        let lba = (lba as u32).to_be_bytes(); let blocks = (count as u16).to_be_bytes();
        let command = [0x2A, 0, lba[0], lba[1], lba[2], lba[3], 0, blocks[0], blocks[1], 0];
        // A repeated command after a lost interface finds the data still in place: the buffer is ours.
        self.host.buffer_mut()[DATA..DATA + bytes].copy_from_slice(&data[..bytes]);
        self.run(&command, bytes, true) == Some(bytes)
    }
    fn flush(&mut self) -> bool { self.run(&[0x35, 0, 0, 0, 0, 0, 0, 0, 0, 0], 0, false).is_some() } // SYNCHRONIZE CACHE(10)
    fn read_only(&self) -> bool { self.protected }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut storage = Storage::probe();
    match &storage {
        Some(s) => mind::println!("[USB] MASS STORAGE: {} SECTORS{}", s.sectors, if s.protected { ", WRITE-PROTECTED" } else { "" }),
        None => mind::println!("[USB] NO MASS STORAGE DEVICE"),
    }
    block::serve(BLOCK_KIND_USB, storage.as_mut().map(|s| s as &mut dyn Driver));
}
