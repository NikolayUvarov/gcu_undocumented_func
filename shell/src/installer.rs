//! The shell's `install` in words, without system calls (tests/shell_host.rs, 352-APP-0157): the command's arguments,
//! a disk as `install.wit` 1.0 lists it, its refusal, the installer's errors and the layout it wrote.

use alloc::format;
use alloc::string::String;

/// `install` alone lists the disks; `install N` installs onto disk N.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asked { List, Disk(u8) }

pub const USAGE: &str = "INSTALL: INSTALL LISTS THE DISKS; INSTALL N WRITES MIND CORE ONTO DISK N, ERASING IT";

pub fn parse(args: &str) -> Result<Asked, &'static str> {
    let words: alloc::vec::Vec<&str> = args.split_whitespace().collect();
    match words.as_slice() {
        [] => Ok(Asked::List),
        [n] => n.parse::<u8>().map(Asked::Disk).map_err(|_| USAGE),
        _ => Err(USAGE),
    }
}

/// A disk as the installer lists it; `bus`, `refusal` are `install.wit`'s codes.
pub struct Disk<'a> { pub number: u8, pub bus: usize, pub model: &'a str, pub mib: u32, pub signature: &'a str, pub partitions: u8, pub words: &'a str, pub refusal: usize }

pub fn bus(code: usize) -> &'static str { ["ATA", "AHCI", "USB", "VIRTIO", "NVME"].get(code).copied().unwrap_or("OTHER") }

/// Why a disk is not offered (`install.wit`'s `refusal`); None when it is.
pub fn refusal(code: usize) -> Option<&'static str> {
    match code {
        0 => None,
        1 => Some("THE MEDIUM THIS LIVE SYSTEM BOOTED FROM"),
        2 => Some("SMALLER THAN MIND CORE'S LAYOUT"),
        3 => Some("PAST 2 TIB, WHERE A GPT IS NEEDED (NOT MADE YET)"),
        4 => Some("READ-ONLY"),
        5 => Some("A VOLUME OF IT IS MOUNTED"),
        _ => Some("REFUSED FOR A REASON THIS SHELL DOES NOT KNOW"),
    }
}

// What the disk holds now: EMPTY, or its MBR's signature and how many partitions it lists.
fn holds(d: &Disk) -> String {
    if d.signature.eq_ignore_ascii_case("EMPTY") { String::from("EMPTY") }
    else { format!("MBR {}, {} PARTITION{}", d.signature, d.partitions, if d.partitions == 1 { "" } else { "S" }) }
}

/// One line of the list: the number, the bus, the model when the drive names one, the size, what it holds, then the
/// words to type back or why it is refused.
pub fn line(d: &Disk) -> String {
    let model = if d.model.is_empty() { String::new() } else { format!(" {}", d.model) };
    let tail = match refusal(d.refusal) { Some(why) => format!("REFUSED: {why}"), None => format!("TO INSTALL: INSTALL {}, THEN TYPE {}", d.number, d.words) };
    format!("  {:>2}  {}{model}  {} MIB  {}  {tail}", d.number, bus(d.bus), d.mib, holds(d))
}

/// The disk `install N` is about to erase, and what is lost with it.
pub fn shown(d: &Disk) -> String {
    let lost = if d.partitions > 0 { format!(": ITS {} PARTITION{} AND ALL THEIR FILES ARE ERASED", d.partitions, if d.partitions == 1 { "" } else { "S" }) } else { String::new() };
    let model = if d.model.is_empty() { String::new() } else { format!(" {}", d.model) };
    format!("INSTALL: DISK {}: {}{model}, {} MIB, {}{lost}", d.number, bus(d.bus), d.mib, holds(d))
}

/// The second question, for a disk that lists partitions.
pub fn erase_question(d: &Disk) -> String {
    format!("ERASE THE {} PARTITION{} OF DISK {}?", d.partitions, if d.partitions == 1 { "" } else { "S" }, d.number)
}

/// The installer's error (`install.wit`'s `error` code) in words. A failed step of the writing leaves the disk with no
/// ESP's entry, and says so.
pub fn error(code: usize) -> String {
    let (what, written) = match code {
        0 => ("THIS IS NOT A LIVE BOOT", false),
        1 => ("NO DISK OF THAT NUMBER", false),
        2 => ("THE DISK IS REFUSED", false),
        3 => ("THE WORDS TYPED ARE NOT THE DISK'S", false),
        4 => ("THE DISK HAS PARTITIONS, AND THEIR ERASING WAS NOT AGREED", false),
        5 => ("READING THE LIVE VOLUME FAILED", true),
        6 => ("CLEARING THE PARTITION TABLE FAILED", true),
        7 => ("COPYING THE BOOT VOLUME FAILED", true),
        8 => ("THE COPY READ BACK IS NOT THE BOOT VOLUME", true),
        9 => ("WRITING MIND LOG FAILED", true),
        10 => ("WRITING THE STORE'S PARTITION FAILED", true),
        11 => ("WRITING store.txt FAILED", true),
        12 => ("WRITING THE ESP'S ENTRY FAILED", true),
        13 => ("THE PARTITION TABLE READ BACK IS NOT WHAT WAS WRITTEN", true),
        14 => ("THE NEW ESP DOES NOT HOLD THE MANIFEST THE LOADER VERIFIED", true),
        _ => ("AN ERROR THIS SHELL DOES NOT KNOW", true),
    };
    if written { format!("INSTALL: NOT INSTALLED: {what}. THE DISK HAS NO ESP: INSTALL AGAIN") } else { format!("INSTALL: NOT INSTALLED: {what}") }
}

/// What was written, from the installer's `layout`.
pub fn layout(number: u8, signature: u32, partitions: [(u32, u32); 3]) -> String {
    let [esp, log, store] = partitions;
    format!("INSTALL: DONE ON DISK {number}, MBR {signature:08X}: THE ESP AT SECTOR {} ({} SECTORS), MIND LOG AT {} ({}), THE STORE AT {} ({}). \
             TAKE THE LIVE MEDIUM OUT AND RESTART TO BOOT IT", esp.0, esp.1, log.0, log.1, store.0, store.1)
}
