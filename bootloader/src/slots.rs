// Slots A and B and the two boot records (351-UPD-0006, docs/update/slots.md; MC-9.1, 9.3). Free of UEFI so the host
// tests (tests/boot_slots_host.rs) run the same choice the bootloader makes.

/// A boot record's size: one sector.
pub const RECORD: usize = 512;
pub const MAGIC: &[u8; 8] = b"MINDBOOT";
pub const FORMAT: u32 = 1;
const CONFIRMED: u8 = 1;
/// The largest sequence a record may hold (351-UPD-0015): a trial writes one more, and its failure one more again.
pub const SEQUENCE_LAST: u64 = u64::MAX - 2;

/// One boot record: which slot to boot, the slot to fall back to, the tries a trial has left, and whether the slot
/// was confirmed. The record with the higher sequence number counts; a writer always overwrites the other one.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Record { pub sequence: u64, pub slot: u8, pub fallback: u8, pub tries: u8, pub confirmed: bool }

/// CRC-32 (IEEE 802.3, as zlib computes it).
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 { crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 }; }
    }
    !crc
}

fn is_slot(slot: u8) -> bool { slot == b'A' || slot == b'B' }

impl Record {
    /// The record in `data`, or None for anything else: another size, magic or format, a bad CRC (a torn write), an
    /// unknown slot or flag, a sequence above `SEQUENCE_LAST`.
    pub fn parse(data: &[u8]) -> Option<Record> {
        if data.len() != RECORD || &data[..8] != MAGIC || data[8..12] != FORMAT.to_le_bytes() { return None; }
        if data[RECORD - 4..] != crc32(&data[..RECORD - 4]).to_le_bytes() { return None; }
        let (slot, fallback, tries, flags) = (data[20], data[21], data[22], data[23]);
        if !is_slot(slot) || !(fallback == 0 || is_slot(fallback)) || flags & !CONFIRMED != 0 || data[24..RECORD - 4].iter().any(|&b| b != 0) { return None; }
        let sequence = u64::from_le_bytes(data[12..20].try_into().ok()?);
        if sequence > SEQUENCE_LAST { return None; }
        Some(Record { sequence, slot, fallback, tries, confirmed: flags & CONFIRMED != 0 })
    }

    pub fn encode(&self) -> [u8; RECORD] {
        let mut data = [0u8; RECORD];
        data[..8].copy_from_slice(MAGIC);
        data[8..12].copy_from_slice(&FORMAT.to_le_bytes());
        data[12..20].copy_from_slice(&self.sequence.to_le_bytes());
        data[20..24].copy_from_slice(&[self.slot, self.fallback, self.tries, if self.confirmed { CONFIRMED } else { 0 }]);
        let crc = crc32(&data[..RECORD - 4]);
        data[RECORD - 4..].copy_from_slice(&crc.to_le_bytes());
        data
    }
}

/// What the bootloader does with the records it read (None: missing or damaged).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Plan {
    /// The record it follows (0 or 1), if one is valid.
    pub chosen: Option<usize>,
    /// The slots to try in turn; 0 for none.
    pub order: [u8; 2],
    /// The first slot boots on trial, once `write` is on the disk.
    pub trial: bool,
    /// A record to write before anything is loaded, and the record file it goes to.
    pub write: Option<(usize, Record)>,
}

pub fn plan(records: [Option<Record>; 2]) -> Plan {
    // A record that could not count down is as good as missing (parse refuses it too).
    let records = records.map(|r| r.filter(|r| r.sequence <= SEQUENCE_LAST));
    let chosen = match records {
        [Some(a), Some(b)] => if b.sequence > a.sequence { 1 } else { 0 },
        [Some(_), None] => 0,
        [None, Some(_)] => 1,
        // No record to follow: either slot that verifies.
        [None, None] => return Plan { chosen: None, order: [b'A', b'B'], trial: false, write: None },
    };
    let record = records[chosen].unwrap();
    let fallback = if record.fallback != record.slot { record.fallback } else { 0 };
    let (order, trial, write) = if record.confirmed {
        ([record.slot, fallback], false, None)
    } else if record.tries > 0 {
        // One try fewer on the disk before the slot runs: a trial that never confirms ends after its tries.
        ([record.slot, fallback], true, Some((1 - chosen, Record { sequence: record.sequence + 1, tries: record.tries - 1, ..record })))
    } else if fallback != 0 {
        ([fallback, 0], false, None)
    } else {
        ([record.slot, 0], false, None)
    };
    Plan { chosen: Some(chosen), order, trial, write }
}

/// After a trial slot failed to load: the record that leaves it no tries, and the record file it goes to.
pub fn spent(written: (usize, Record)) -> (usize, Record) {
    let (file, record) = written;
    (1 - file, Record { sequence: record.sequence + 1, tries: 0, ..record })
}
