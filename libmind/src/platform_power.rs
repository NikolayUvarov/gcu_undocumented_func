//! The platform's transmit power policy for a WLAN device (213-KRN-0122): whether the ACPI tables hold the SAR and
//! antenna-gain objects a radio must obey (`WRDS`, `RWRD`, `RWSI`, `RWGS`, `RTAG`). No AML is run: every DSDT and SSDT is
//! read whole and checked, and the five names are searched for as bytes. None anywhere is `Absent`; one anywhere, or a
//! table not read whole, is `Unknown`, which a radio takes as no transmission. No limits are read yet.

/// The objects that would carry a policy, as their ACPI names.
pub const NAMES: [&[u8; 4]; 5] = [b"WRDS", b"RWRD", b"RWSI", b"RWGS", b"RTAG"];
/// At most this many tables, and this many bytes in all, are searched; past either the answer is `Unknown`.
pub const TABLES_MAX: usize = 64;
pub const BYTES_MAX: usize = 16 << 20;
const HEADER: usize = 36;

/// Why the search could not say the policy is absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// No DSDT among the tables.
    NoDsdt,
    /// A table shorter than its header, or than the length it states (its signature and number among the tables).
    Truncated([u8; 4], usize),
    /// A table whose bytes do not sum to 0.
    Checksum([u8; 4], usize),
    /// One of the five names, in that table: its AML is not read yet.
    Named([u8; 4], [u8; 4]),
    /// More tables or bytes than the search reads.
    TooMuch,
    /// A table the platform has that could not be mapped (its number among the tables).
    Unreadable(usize),
}

/// The answer for the radio's device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Policy {
    /// Every AML table was read whole and checked, and none holds any of the five names.
    Absent { tables: usize },
    Unknown(Why),
}

/// The policy that the ACPI tables `tables` give (each a whole table, header first, in the platform's order).
pub fn search<'a>(tables: impl IntoIterator<Item = &'a [u8]>) -> Policy {
    let mut search = Search::new();
    for table in tables { search.feed(table); }
    search.finish()
}

/// The search a table at a time, as `init` maps them in turn.
pub struct Search { n: usize, aml: usize, dsdt: bool, bytes: usize, unknown: Option<Why> }

impl Search {
    pub const fn new() -> Self { Self { n: 0, aml: 0, dsdt: false, bytes: 0, unknown: None } }
    /// The next table, whole, header first.
    pub fn feed(&mut self, table: &[u8]) {
        let n = self.n;
        self.n += 1;
        if self.unknown.is_some() { return; }
        if n >= TABLES_MAX { self.unknown = Some(Why::TooMuch); return; }
        let signature = signature(table);
        if &signature != b"DSDT" && &signature != b"SSDT" { return; }
        let Some(length) = table.get(4..8).map(|l| u32::from_le_bytes([l[0], l[1], l[2], l[3]]) as usize) else { self.unknown = Some(Why::Truncated(signature, n)); return };
        if length < HEADER || table.len() < length { self.unknown = Some(Why::Truncated(signature, n)); return; }
        self.bytes += length;
        if self.bytes > BYTES_MAX { self.unknown = Some(Why::TooMuch); return; }
        let table = &table[..length];
        if table.iter().fold(0u8, |sum, &b| sum.wrapping_add(b)) != 0 { self.unknown = Some(Why::Checksum(signature, n)); return; }
        if let Some(name) = NAMES.iter().find(|name| table[HEADER..].windows(4).any(|w| w == &name[..])) { self.unknown = Some(Why::Named(signature, **name)); return; }
        self.dsdt |= &signature == b"DSDT";
        self.aml += 1;
    }
    /// A table the platform has but that could not be mapped: nothing can be said of what it holds.
    pub fn unreadable(&mut self) { if self.unknown.is_none() { self.unknown = Some(Why::Unreadable(self.n)); } self.n += 1; }
    pub fn finish(self) -> Policy {
        match self.unknown {
            Some(why) => Policy::Unknown(why),
            None if !self.dsdt => Policy::Unknown(Why::NoDsdt),
            None => Policy::Absent { tables: self.aml },
        }
    }
}

fn signature(table: &[u8]) -> [u8; 4] {
    let mut s = [b'?'; 4];
    for (d, &b) in s.iter_mut().zip(table.iter().take(4)) { *d = if b.is_ascii_graphic() { b } else { b'?' }; }
    s
}

impl Policy {
    /// The word a radio's driver is given (213-KRN-0122): `platform=absent` or `platform=unknown:<reason>`.
    pub fn word(&self, out: &mut [u8; 48]) -> usize {
        let mut w = Writer { out, n: 0 };
        match self {
            Policy::Absent { .. } => w.put(b"platform=absent"),
            Policy::Unknown(why) => {
                w.put(b"platform=unknown:");
                match why {
                    Why::NoDsdt => w.put(b"no-dsdt"),
                    Why::Truncated(s, _) => { w.put(b"truncated-"); w.put(s); }
                    Why::Checksum(s, _) => { w.put(b"checksum-"); w.put(s); }
                    Why::Named(s, name) => { w.put(name); w.put(b"-in-"); w.put(s); }
                    Why::TooMuch => w.put(b"too-many-tables"),
                    Why::Unreadable(_) => w.put(b"unreadable-table"),
                }
            }
        }
        w.n
    }
}

// The word is printable ASCII without a space, whatever bytes a table's signature holds: the driver reads no other.
struct Writer<'a> { out: &'a mut [u8; 48], n: usize }
impl Writer<'_> {
    fn put(&mut self, bytes: &[u8]) {
        for &b in bytes { if self.n < self.out.len() { self.out[self.n] = if b.is_ascii_graphic() { b } else { b'?' }; self.n += 1; } }
    }
}
