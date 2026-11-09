#![no_std]
#![no_main]
// LEGACY: the whole driver: primary IDE channel by PIO on ISA ports (docs/legacy.md).
// Ring 3 primary-channel ATA driver: PIO LBA28 without interrupts (nIEN), block protocol for vfs_server: READ SECTORS,
// WRITE SECTORS and FLUSH CACHE (writes only for clients with the write badge, see mind::block_protocol). Every wait
// ends at a deadline and fails the request, which the log names once (000-DRV-0064, ide.rs).
mod ide;

use ide::{Bus, Fault, Ide};
use mind::abi::{BootInfo, BLOCK_KIND_ATA, SLOT_DEV0, SLOT_DEV1};
use mind::block::{self, Driver};
use mind::dev::Ports;

// The command block's ports and the control port, each its own grant.
struct Channel { io: Ports, control: Ports }

impl Bus for Channel {
    fn in8(&self, port: u16) -> u8 { if port == ide::CONTROL { self.control.in8(port) } else { self.io.in8(port) } }
    fn out8(&self, port: u16, value: u8) { if port == ide::CONTROL { self.control.out8(port, value) } else { self.io.out8(port, value) } }
    fn read_words(&self, port: u16, words: &mut [u16]) -> bool { self.io.read_words(port, words).is_ok() }
    fn write_words(&self, port: u16, words: &[u16]) -> bool { self.io.write_words(port, words).is_ok() }
    fn now_ns(&self) -> u64 { mind::time::monotonic_ns() }
    fn sleep(&self) { mind::time::sleep(1); }
}

// `failing`: the fault the last request ended with, so that a run of them is logged once, and the drive's return.
struct Ata { ide: Ide<Channel>, failing: Option<Fault> }

impl Ata {
    fn done(&mut self, what: &str, lba: u64, count: usize, result: Result<(), Fault>) -> bool {
        match result {
            Ok(()) => { if self.failing.take().is_some() { mind::println!("[ATA] THE DRIVE ANSWERS AGAIN"); } true }
            Err(Fault::Refused) => { mind::println!("[ATA] {} OF {} SECTORS AT {} REFUSED: OUTSIDE ONE COMMAND, THE DRIVE OR THE BUFFER", what, count, lba); false }
            Err(fault) => {
                if self.failing.replace(fault) != Some(fault) {
                    mind::println!("[ATA] {} AT {} FAILED: {}", what, lba, match fault {
                        Fault::Busy => "THE DRIVE STAYED BUSY PAST ITS DEADLINE", Fault::NoData => "NO DATA CAME IN TIME", _ => "THE DRIVE REPORTED AN ERROR" });
                }
                false
            }
        }
    }
}

impl Driver for Ata {
    fn sectors(&self) -> u64 { self.ide.sectors }
    fn read(&mut self, lba: u64, count: usize, out: &mut [u8]) -> bool { let r = self.ide.read(lba, count, out); self.done("READ", lba, count, r) }
    fn write(&mut self, lba: u64, count: usize, data: &[u8]) -> bool { let r = self.ide.write(lba, count, data); self.done("WRITE", lba, count, r) }
    fn flush(&mut self) -> bool { let r = self.ide.flush(); self.done("FLUSH", 0, 0, r) }
    fn read_only(&self) -> bool { false }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut disk = Ide::probe(Channel { io: Ports(SLOT_DEV0), control: Ports(SLOT_DEV1) }).map(|ide| Ata { ide, failing: None });
    match &disk {
        Some(d) => mind::println!("[ATA] PRIMARY MASTER: {} SECTORS", d.ide.sectors),
        None => mind::println!("[ATA] NO DISK ON PRIMARY CHANNEL"),
    }
    block::serve(BLOCK_KIND_ATA, disk.as_mut().map(|d| d as &mut dyn Driver));
}
