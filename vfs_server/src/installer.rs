// The installer's service (idl/install.wit, 352-KRN-0269): on a live boot, the disks vfs_server holds beside the boot
// volume's, offered to the shell's `install` command and written by install.rs from the volume the loader verified.
use crate::disk::Shared;
use crate::fat::{Sectors, Volume, SECTOR};
use crate::install::{self, Step};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use mind::abi::{BootInfo, BLOCK_KIND_AHCI, BLOCK_KIND_ATA, BLOCK_KIND_NVME, BLOCK_KIND_USB, BLOCK_KIND_VIRTIO};
use mind::idl::codec::Text;
use mind::idl::install::{Bus, Disk, Error, Layout, Refusal};

const ISO_DESCRIPTOR: u32 = 64; // ISO 9660's first volume descriptor, block 16 of 2048 bytes, in 512-byte sectors

// A disk the installer may write: its MBR signature, whether sector 0 is blank, the entries its MBR lists, and whether it
// is the live medium (an ISO 9660 image whose volume at 1 MiB is the one the loader read into memory), and the drive's model
// as its driver gives it (block.wit 2.1, 352-KRN-0294), asked once.
pub struct Target { disk: Shared, signature: [u8; 4], blank: bool, partitions: u8, live: bool, model: Text<40> }

// The disks, and the boot volume's disk the system is copied from (ramdisk#2's).
pub struct Live { targets: Vec<Target>, source: Shared }

impl Live {
    pub fn new(disks: Vec<Shared>, source: Shared) -> Self {
        let mut live = Live { targets: Vec::new(), source };
        for disk in disks {
            let model = disk.model();
            let mut target = Target { disk, signature: [0; 4], blank: false, partitions: 0, live: false, model };
            if live.look(&mut target) { live.targets.push(target); }
        }
        live
    }

    fn look(&mut self, target: &mut Target) -> bool {
        let mut first = [0u8; SECTOR];
        if !target.disk.read(0, &mut first) { return false; }
        target.signature = [first[440], first[441], first[442], first[443]];
        target.blank = first.iter().all(|&b| b == 0);
        target.partitions = if first[510..] == [0x55, 0xAA] { (0..4).filter(|i| first[446 + i * 16 + 4] != 0).count() as u8 } else { 0 };
        let (mut descriptor, mut theirs, mut ours) = ([0u8; SECTOR], [0u8; SECTOR], [0u8; SECTOR]);
        target.live = target.disk.read(ISO_DESCRIPTOR, &mut descriptor) && &descriptor[1..6] == b"CD001"
            && target.disk.read(install::ALIGN, &mut theirs) && self.source.read(0, &mut ours) && theirs == ours;
        true
    }

    pub fn count(&self) -> usize { self.targets.len() }

    // Whether a volume vfs_server mounted is on one of the disks.
    fn refusal(&self, target: &Target, mounted: &dyn Fn(&Shared) -> bool) -> Refusal {
        if target.live { return Refusal::Live; }
        if mounted(&target.disk) { return Refusal::Mounted; }
        if !target.disk.writable() { return Refusal::ReadOnly; }
        match install::volume_sectors(&mut self.source.clone()).map(|volume| install::layout(target.disk.sectors(), volume)) {
            Some(Ok(_)) => Refusal::None,
            Some(Err(Step::TooLarge)) => Refusal::Large,
            _ => Refusal::Small,
        }
    }

    // As `disks` answers: numbered from 1 in the order vfs_server found them.
    pub fn disks(&self, mounted: &dyn Fn(&Shared) -> bool) -> Vec<Disk> {
        self.targets.iter().enumerate().map(|(index, target)| {
            let (bus, _) = bus(target.disk.kind());
            Disk { number: index as u8 + 1, bus, model: target.model, mib: (target.disk.sectors() >> 11).min(u32::MAX as u64) as u32, signature: Text::new(&signature_text(target)).unwrap_or_default(),
                   partitions: target.partitions, words: Text::new(&words(target)).unwrap_or_default(), refusal: self.refusal(target, mounted) }
        }).collect()
    }

    /// Writes the system onto disk `number`, with the words typed back; `check` says whether a volume holds the manifest the
    /// loader verified. Every outcome is logged.
    pub fn install(&mut self, number: u8, typed: &str, erase: bool, stamp: u32, seed: u64, mounted: &dyn Fn(&Shared) -> bool,
                   check: &dyn Fn(&mut Volume<Shared>, [u8; 4]) -> bool) -> Result<Layout, Error> {
        let index = (number as usize).checked_sub(1).filter(|&i| i < self.targets.len()).ok_or(Error::NoDisk)?;
        let target = &self.targets[index];
        let said = words(target);
        let refusal = self.refusal(target, mounted);
        if refusal != Refusal::None { mind::println!("[VFS] INSTALL: DISK {} ({}) REFUSED: {:?}", number, said, refusal); return Err(Error::Refused); }
        if !same_words(typed, &said) { mind::println!("[VFS] INSTALL: DISK {}: THE WORDS TYPED ARE NOT {}", number, said); return Err(Error::Mismatch); }
        if target.partitions > 0 && !erase { mind::println!("[VFS] INSTALL: DISK {} LISTS {} PARTITIONS, NOT CONFIRMED ERASED", number, target.partitions); return Err(Error::Partitions); }
        let signature = self.new_signature(seed);
        mind::println!("[VFS] INSTALL: ONTO DISK {} ({}), MBR SIGNATURE {:08X}: STARTED", number, said, signature);
        let mut disk = self.targets[index].disk.clone();
        let result = install::install(&mut disk, &mut self.source.clone(), signature, stamp);
        let result = result.map_err(error_of).and_then(|layout| {
            // The new ESP read through the FAT code: the slot's manifest the loader verified is there.
            let Ok(mut volume) = Volume::mount_at(disk.clone(), layout.esp.0) else { return Err(Error::Manifest) };
            if check(&mut volume, signature.to_le_bytes()) { Ok(layout) } else { Err(Error::Manifest) }
        });
        // The same drive: its model stays.
        let model = self.targets[index].model;
        let mut target = Target { disk, signature: [0; 4], blank: false, partitions: 0, live: false, model };
        self.look(&mut target);
        self.targets[index] = target;
        match result {
            Ok(l) => {
                mind::println!("[VFS] INSTALL: DISK {} WRITTEN: THE ESP {} MB FROM {}, {} {} MB FROM {}, THE STORE'S {} MB FROM {} (BLANK, NAMED IN STORE.TXT)",
                               number, l.esp.1 >> 11, l.esp.0, install::LOG_LABEL, l.log.1 >> 11, l.log.0, l.store.1 >> 11, l.store.0);
                Ok(Layout { signature, esp_first: l.esp.0, esp_sectors: l.esp.1, log_first: l.log.0, log_sectors: l.log.1, store_first: l.store.0, store_sectors: l.store.1 })
            }
            Err(error) => { mind::println!("[VFS] INSTALL: DISK {}: {:?} FAILED, THE DISK HAS NO ESP: INSTALL AGAIN", number, error); Err(error) }
        }
    }

    // Not 0, and none of the disks' own.
    fn new_signature(&self, seed: u64) -> u32 {
        (0u32..).map(|n| { let h = mind::sha256::digest(&[seed.to_le_bytes(), (n as u64).to_le_bytes()].concat()); u32::from_le_bytes([h[0], h[1], h[2], h[3]]) })
            .find(|&s| s != 0 && self.targets.iter().all(|t| u32::from_le_bytes(t.signature) != s)).unwrap_or(1)
    }
}

fn bus(kind: usize) -> (Bus, &'static str) {
    match kind { BLOCK_KIND_ATA => (Bus::Ata, "ATA"), BLOCK_KIND_AHCI => (Bus::Ahci, "AHCI"), BLOCK_KIND_USB => (Bus::Usb, "USB"), BLOCK_KIND_VIRTIO => (Bus::Virtio, "VIRTIO"), BLOCK_KIND_NVME => (Bus::Nvme, "NVME"), _ => (Bus::Other, "DISK") }
}

fn signature_text(target: &Target) -> String {
    let s = target.signature;
    if target.blank { String::from("EMPTY") } else { format!("{:02X}{:02X}{:02X}{:02X}", s[3], s[2], s[1], s[0]) }
}

// The words typed back: the bus, the size in MiB, and the signature's first four digits or EMPTY.
fn words(target: &Target) -> String {
    let signature = signature_text(target);
    format!("{} {} {}", bus(target.disk.kind()).1, target.disk.sectors() >> 11, &signature[..signature.len().min(if target.blank { 5 } else { 4 })])
}

fn same_words(typed: &str, said: &str) -> bool {
    let (mut a, mut b) = (typed.split_ascii_whitespace(), said.split_ascii_whitespace());
    loop {
        match (a.next(), b.next()) {
            (None, None) => return true,
            (Some(x), Some(y)) if x.eq_ignore_ascii_case(y) => {}
            _ => return false,
        }
    }
}

fn error_of(step: Step) -> Error {
    match step {
        Step::TooSmall | Step::TooLarge | Step::ReadOnly => Error::Refused,
        Step::Source => Error::Source, Step::Clear => Error::Clear, Step::Copy => Error::Copy, Step::Verify => Error::Verify,
        Step::Log => Error::Log, Step::Store => Error::Store, Step::Named => Error::Named, Step::Table => Error::Table, Step::ReadBack => Error::ReadBack,
    }
}

// The boot volume's identity names no disk on a live boot: the loader's manifest is all `check` compares.
pub fn holds_manifest(volume: &mut Volume<Shared>, signature: [u8; 4], info: &BootInfo) -> bool { crate::is_boot_volume(volume, signature, info) }
