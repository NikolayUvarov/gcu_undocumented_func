#![no_std]
#![no_main]
// Ring 3 USB mass storage class driver: the first mass storage interface usb_host gives it (idl/usb.wit; its client
// with the storage badge in SLOT_DEV0 can claim only such interfaces, issue 164), Bulk-Only Transport, SCSI READ(10),
// WRITE(10) and SYNCHRONIZE CACHE(10); MODE SENSE(6) tells a write-protected medium. Writes only for clients with the
// write badge (mind::block_protocol). If usb_host restarts, the interface is claimed and set up again and the request
// repeated (reads and rewrites of the same data are idempotent, MC-6.6). A refused command is explained by REQUEST SENSE
// in the log; a lost device, its return and a reset it reports are logged (211-DRV-0019).
use mind::abi::{BootInfo, BLOCK_KIND_USB, SLOT_DEV0};
use mind::block::{self, Driver};
use mind::ipc::Endpoint;
use mind::usb::Host;

const CBW: usize = 0; const CSW: usize = 64; const DATA: usize = 4096; // in the buffer lent to usb_host
const PROBE: usize = 512; // probes and sense data: never where a write's data waits for a retry
const CLAIM_TRIES: usize = 20; // usb_host may still be setting the device up

// The claimed interface and its Bulk-Only Transport state.
struct Bot { handle: u32, out: u8, input: u8, tag: u32 }

// `lost`: the device stopped answering and was not claimed again; `seen`: refusals already logged.
struct Storage { host: Host, bot: Bot, sectors: u64, protected: bool, lost: bool, seen: [(u8, Sense); SEEN], seen_count: usize }

// How a BOT cycle ended: done with the bytes moved, refused by the device (CHECK CONDITION), or the interface is gone.
enum Cycle { Done(usize), Refused, Gone }

// One BOT cycle: CBW on bulk OUT, data in or out at `at` (already there when sent), CSW on bulk IN.
fn bot_cycle(host: &mut Host, bot: &mut Bot, command: &[u8], length: usize, send: bool, at: usize) -> Cycle {
    bot.tag = bot.tag.wrapping_add(1);
    let cbw = &mut host.buffer_mut()[CBW..CBW + 31];
    cbw.fill(0);
    cbw[..4].copy_from_slice(b"USBC"); cbw[4..8].copy_from_slice(&bot.tag.to_le_bytes()); cbw[8..12].copy_from_slice(&(length as u32).to_le_bytes());
    cbw[12] = if length > 0 && !send { 0x80 } else { 0 }; cbw[14] = command.len() as u8; cbw[15..15 + command.len()].copy_from_slice(command);
    if host.bulk(bot.handle, bot.out, CBW, 31).is_err() { return Cycle::Gone; }
    let moved = if length == 0 { 0 } else {
        match host.bulk(bot.handle, if send { bot.out } else { bot.input }, at, length) {
            Ok(moved) => moved,
            Err(mind::sys::Error::NotFound | mind::sys::Error::Peer) => return Cycle::Gone,
            Err(_) => 0, // a stalled data stage still ends with a CSW
        }
    };
    if host.bulk(bot.handle, bot.input, CSW, 13).is_err() { return Cycle::Gone; }
    let status = &host.buffer()[CSW..CSW + 13];
    if &status[..4] != b"USBS" || status[4..8] != bot.tag.to_le_bytes() { return Cycle::Gone; }
    match status[12] { 0 => Cycle::Done(moved), 1 => Cycle::Refused, _ => Cycle::Gone } // 2, a phase error: start over
}

// What REQUEST SENSE says about a refusal: the sense key, ASC and ASCQ.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Sense { key: u8, asc: u8, ascq: u8 }
impl Sense {
    fn attention(self) -> bool { self.key == 6 }
    fn reset(self) -> bool { self.key == 6 && self.asc == 0x29 }
    fn meaning(self) -> &'static str {
        match (self.key, self.asc) {
            (6, 0x29) => " (THE DEVICE WAS RESET)", (6, 0x28) => " (THE MEDIUM MAY HAVE CHANGED)", (6, _) => " (UNIT ATTENTION)",
            (5, 0x20) => " (NOT SUPPORTED)", (5, _) => " (ILLEGAL REQUEST)", (2, _) => " (NOT READY)", (3, _) => " (MEDIUM ERROR)",
            (4, _) => " (HARDWARE ERROR)", (7, _) => " (WRITE PROTECTED)", _ => "",
        }
    }
}

fn sense(host: &mut Host, bot: &mut Bot) -> Option<Sense> {
    match bot_cycle(host, bot, &[0x03, 0, 0, 0, 18, 0], 18, false, PROBE) {
        Cycle::Done(n) if n >= 14 => { let b = &host.buffer()[PROBE..PROBE + 14]; Some(Sense { key: b[2] & 0x0F, asc: b[12], ascq: b[13] }) }
        _ => None,
    }
}

fn name(operation: u8) -> &'static str {
    match operation { 0x00 => "TEST UNIT READY", 0x1A => "MODE SENSE", 0x25 => "READ CAPACITY", 0x28 => "READ", 0x2A => "WRITE", 0x35 => "SYNCHRONIZE CACHE", _ => "A COMMAND" }
}

// The next mass storage interface (SCSI over Bulk-Only: 08/06/50) and its bulk endpoints, in up to CLAIM_TRIES tries
// 100 ms apart.
fn claim(host: &mut Host) -> Option<Bot> {
    for attempt in 0..CLAIM_TRIES {
        match host.claim() {
            Ok((handle, info)) => {
                let out = info.endpoints().iter().find(|e| e.is_bulk() && !e.is_in()).map(|e| e.address);
                let input = info.endpoints().iter().find(|e| e.is_bulk() && e.is_in()).map(|e| e.address);
                if let (6, 0x50, Some(out), Some(input)) = (info.subclass, info.protocol, out, input) { return Some(Bot { handle, out, input, tag: 0 }); }
                let _ = host.release(handle);
            }
            Err(_) => if attempt + 1 < CLAIM_TRIES { mind::time::sleep(100); },
        }
    }
    None
}

// TEST UNIT READY until ready (REQUEST SENSE after each refusal), then READ CAPACITY(10); 512-byte sectors required.
// Returns the sector count and whether the device said it had been reset.
fn capacity(host: &mut Host, bot: &mut Bot) -> Option<(u64, bool)> {
    let mut reset = false;
    for _ in 0..5 {
        match bot_cycle(host, bot, &[0x00, 0, 0, 0, 0, 0], 0, false, PROBE) {
            Cycle::Done(_) => break,
            Cycle::Refused => if sense(host, bot).is_some_and(Sense::reset) { reset = true; },
            Cycle::Gone => return None,
        }
        mind::time::sleep(50);
    }
    let Cycle::Done(_) = bot_cycle(host, bot, &[0x25, 0, 0, 0, 0, 0, 0, 0, 0, 0], 8, false, PROBE) else { return None };
    let reply = &host.buffer()[PROBE..PROBE + 8];
    let last = u32::from_be_bytes(reply[..4].try_into().unwrap()) as u64; let size = u32::from_be_bytes(reply[4..8].try_into().unwrap());
    (size == 512).then_some((last + 1, reset))
}

const SEEN: usize = 16;

impl Storage {
    fn probe() -> Option<Self> {
        let mut host = Host::new(Endpoint(SLOT_DEV0)).ok()?;
        let mut bot = claim(&mut host)?;
        let (sectors, _) = capacity(&mut host, &mut bot)?; // a reset at power-on is expected
        // MODE SENSE(6), all pages, the 4-byte header: WP is bit 7 of byte 2. A device that does not answer is taken as
        // writable; a write it refuses fails anyway.
        let protected = matches!(bot_cycle(&mut host, &mut bot, &[0x1A, 0, 0x3F, 0, 4, 0], 4, false, PROBE), Cycle::Done(_)) && host.buffer()[PROBE + 2] & 0x80 != 0;
        Some(Self { host, bot, sectors, protected, lost: false, seen: [(0, Sense { key: 0, asc: 0, ascq: 0 }); SEEN], seen_count: 0 })
    }

    // A refusal in the log, each kind once (211-DRV-0019).
    fn refused(&mut self, operation: u8, sense: Option<Sense>) {
        let Some(sense) = sense else { mind::println!("[USB] STORAGE: {} REFUSED, NO SENSE DATA", name(operation)); return };
        if self.seen[..self.seen_count].contains(&(operation, sense)) { return; }
        if self.seen_count < SEEN { self.seen[self.seen_count] = (operation, sense); self.seen_count += 1; }
        mind::println!("[USB] STORAGE: {} REFUSED: SENSE KEY {:X} ASC {:02X} ASCQ {:02X}{}{}", name(operation), sense.key, sense.asc, sense.ascq, sense.meaning(),
                       if operation == 0x35 && sense.key == 5 { "; A FLUSH CANNOT EMPTY THE DEVICE'S CACHE" } else { "" });
    }

    // A command. A refusal is explained in the log and, after a unit attention, the command is repeated once. If the
    // interface was lost, it is claimed again and the command repeated; the loss, the return and a reset are logged.
    fn run(&mut self, command: &[u8], length: usize, send: bool, at: usize) -> Option<usize> {
        match bot_cycle(&mut self.host, &mut self.bot, command, length, send, at) {
            Cycle::Done(moved) => return Some(moved),
            Cycle::Refused => {
                let sense = sense(&mut self.host, &mut self.bot);
                self.refused(command[0], sense);
                if !sense.is_some_and(Sense::attention) { return None; }
                return match bot_cycle(&mut self.host, &mut self.bot, command, length, send, at) { Cycle::Done(moved) => Some(moved), _ => None };
            }
            Cycle::Gone => {}
        }
        if !self.lost { mind::println!("[USB] STORAGE: NO ANSWER TO {}: THE DEVICE IS GONE OR WAS RESET; CLAIMING IT AGAIN", name(command[0])); }
        // Every request gets the tries: a device plugged in again or reset may still be set up by usb_host (211-DRV-0021).
        let found = claim(&mut self.host).and_then(|mut bot| capacity(&mut self.host, &mut bot).map(|c| (bot, c)));
        let Some((bot, (sectors, reset))) = found else {
            if !self.lost { mind::println!("[USB] STORAGE: THE DEVICE IS GONE"); self.lost = true; }
            return None;
        };
        if sectors != self.sectors {
            if !self.lost { mind::println!("[USB] STORAGE: ANOTHER MEDIUM ({} SECTORS) IS NOT USED", sectors); self.lost = true; }
            let _ = self.host.release(bot.handle);
            return None;
        }
        mind::println!("[USB] STORAGE: THE DEVICE IS BACK, THE SAME CAPACITY{}", if reset { "; IT WAS RESET: WRITES IT HAD NOT MADE DURABLE MAY BE LOST" } else { "" });
        self.lost = false;
        self.bot = bot;
        match bot_cycle(&mut self.host, &mut self.bot, command, length, send, at) { Cycle::Done(moved) => Some(moved), _ => None }
    }
}

impl Driver for Storage {
    fn sectors(&self) -> u64 { self.sectors }
    fn read(&mut self, lba: u64, count: usize, out: &mut [u8]) -> bool {
        let lba = (lba as u32).to_be_bytes(); let blocks = (count as u16).to_be_bytes();
        let command = [0x28, 0, lba[0], lba[1], lba[2], lba[3], 0, blocks[0], blocks[1], 0];
        match self.run(&command, count * 512, false, DATA) {
            Some(got) if got == count * 512 => { out[..got].copy_from_slice(&self.host.buffer()[DATA..DATA + got]); true }
            _ => false,
        }
    }
    fn write(&mut self, lba: u64, count: usize, data: &[u8]) -> bool {
        let bytes = count * 512;
        let lba = (lba as u32).to_be_bytes(); let blocks = (count as u16).to_be_bytes();
        let command = [0x2A, 0, lba[0], lba[1], lba[2], lba[3], 0, blocks[0], blocks[1], 0];
        // A repeated write finds its data still in place: probes and sense data use PROBE, not DATA (211-DRV-0019).
        self.host.buffer_mut()[DATA..DATA + bytes].copy_from_slice(&data[..bytes]);
        self.run(&command, bytes, true, DATA) == Some(bytes)
    }
    fn flush(&mut self) -> bool { self.run(&[0x35, 0, 0, 0, 0, 0, 0, 0, 0, 0], 0, false, PROBE).is_some() } // SYNCHRONIZE CACHE(10)
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
