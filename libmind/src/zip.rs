//! A bounded ZIP reader (750-EFF-0005) for the Effector agent's packages: the central directory read whole, then an
//! entry's bytes handed to a sink, stored or deflated (`inflate`), checked against the sizes and CRC-32 the directory
//! names. An archive from the network may be a bomb, carry names that leave the target directory, or lie about its
//! sizes: every such case is refused, never trimmed. Encrypted entries, ZIP64 and archives on several disks are
//! refused too. No system calls: host-tested in tests/zip_host.rs.

const EOCD: u32 = 0x0605_4b50;
const CENTRAL: u32 = 0x0201_4b50;
const LOCAL: u32 = 0x0403_4b50;
const EOCD_LEN: usize = 22;
const CENTRAL_LEN: usize = 46;
const LOCAL_LEN: usize = 30;

/// Why an archive or an entry was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Not a ZIP archive, or its structures overlap, run past the data or contradict each other.
    Malformed,
    /// ZIP64, several disks, or a compression method other than stored (0) and deflate (8).
    Unsupported,
    /// An entry is encrypted: a package's authenticity rests on its signature, never on an archive password.
    Encrypted,
    /// A name that is empty, absolute, holds `..`, `.`, an empty part, `\`, `:` or a control character, or repeats.
    Name,
    /// More entries, a longer name, or more bytes than the limits allow, or a compression ratio past them.
    Limit,
    /// The bytes do not have the size or CRC-32 the directory names, or the deflate stream is broken.
    Corrupt,
    /// The sink refused a piece (it could not keep it): nothing is said about the archive.
    Sink,
}

/// The bounds an archive must keep.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub entries: usize,
    pub name: usize,
    /// The largest entry, uncompressed.
    pub entry: u64,
    /// All entries together, uncompressed.
    pub total: u64,
    /// Uncompressed bytes per compressed byte, at most.
    pub ratio: u64,
}

impl Default for Limits {
    fn default() -> Self { Self { entries: 256, name: 255, entry: 64 << 20, total: 256 << 20, ratio: 200 } }
}

fn u16_at(d: &[u8], at: usize) -> Option<u16> { d.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]])) }
fn u32_at(d: &[u8], at: usize) -> Option<u32> { d.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])) }

/// CRC-32 (IEEE 802.3, as ZIP uses it), computed bitwise: packages are small and this keeps no table.
pub fn crc32_update(mut crc: u32, data: &[u8]) -> u32 {
    crc = !crc;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 { crc = (crc >> 1) ^ (0xEDB8_8320 & (crc & 1).wrapping_neg()); }
    }
    !crc
}

/// Whether `name` may be written below a target directory: relative, `/`-separated, every part a real name.
pub fn name_ok(name: &str, limit: usize) -> bool {
    if name.is_empty() || name.len() > limit { return false; }
    if name.starts_with('/') || name.bytes().any(|c| c < 0x20 || c == 0x7f || c == b'\\' || c == b':') { return false; }
    let body = name.strip_suffix('/').unwrap_or(name);
    !body.is_empty() && body.split('/').all(|p| !p.is_empty() && p != "." && p != "..")
}

/// One entry of the central directory.
#[derive(Clone, Copy, Debug)]
pub struct Entry<'a> {
    pub name: &'a str,
    pub method: u16,
    pub crc32: u32,
    pub compressed: u64,
    pub size: u64,
    local: usize,
}

impl Entry<'_> {
    pub fn is_dir(&self) -> bool { self.name.ends_with('/') }
}

/// An archive whose central directory was read and checked whole.
pub struct Archive<'a> { data: &'a [u8], directory: usize, count: usize, limits: Limits }

impl<'a> Archive<'a> {
    /// Reads and checks the central directory: every entry's structure, name, method, sizes and the limits.
    pub fn open(data: &'a [u8], limits: Limits) -> Result<Self, Error> {
        let eocd = Self::find_eocd(data)?;
        let (disk, cd_disk) = (u16_at(data, eocd + 4).ok_or(Error::Malformed)?, u16_at(data, eocd + 6).ok_or(Error::Malformed)?);
        let (here, count) = (u16_at(data, eocd + 8).ok_or(Error::Malformed)?, u16_at(data, eocd + 10).ok_or(Error::Malformed)?);
        let (size, offset) = (u32_at(data, eocd + 12).ok_or(Error::Malformed)?, u32_at(data, eocd + 16).ok_or(Error::Malformed)?);
        if count == 0xFFFF || size == 0xFFFF_FFFF || offset == 0xFFFF_FFFF { return Err(Error::Unsupported); }
        if disk != 0 || cd_disk != 0 || here != count { return Err(Error::Unsupported); }
        let (directory, end) = (offset as usize, offset as usize + size as usize);
        if end > eocd { return Err(Error::Malformed); }
        if count as usize > limits.entries { return Err(Error::Limit); }
        let archive = Self { data, directory, count: count as usize, limits };
        // Walk it once: structure, names, duplicates, sizes and the total.
        let mut at = directory;
        let mut total = 0u64;
        for i in 0..archive.count {
            let (entry, next) = archive.entry_at(at)?;
            if next > end { return Err(Error::Malformed); }
            let mut seen = directory;
            for _ in 0..i { let (other, after) = archive.entry_at(seen)?; if other.name == entry.name { return Err(Error::Name); } seen = after; }
            total = total.checked_add(entry.size).ok_or(Error::Limit)?;
            if total > limits.total { return Err(Error::Limit); }
            at = next;
        }
        if at != end { return Err(Error::Malformed); }
        Ok(archive)
    }

    fn find_eocd(data: &[u8]) -> Result<usize, Error> {
        if data.len() < EOCD_LEN { return Err(Error::Malformed); }
        let lowest = data.len().saturating_sub(EOCD_LEN + 0xFFFF);
        let mut at = data.len() - EOCD_LEN;
        loop {
            if u32_at(data, at) == Some(EOCD) && at + EOCD_LEN + u16_at(data, at + 20).unwrap_or(0) as usize == data.len() { return Ok(at); }
            if at == lowest { return Err(Error::Malformed); }
            at -= 1;
        }
    }

    fn entry_at(&self, at: usize) -> Result<(Entry<'a>, usize), Error> {
        let d = self.data;
        if u32_at(d, at) != Some(CENTRAL) { return Err(Error::Malformed); }
        let flags = u16_at(d, at + 8).ok_or(Error::Malformed)?;
        let method = u16_at(d, at + 10).ok_or(Error::Malformed)?;
        let crc32 = u32_at(d, at + 16).ok_or(Error::Malformed)?;
        let compressed = u32_at(d, at + 20).ok_or(Error::Malformed)?;
        let size = u32_at(d, at + 24).ok_or(Error::Malformed)?;
        let name_len = u16_at(d, at + 28).ok_or(Error::Malformed)? as usize;
        let extra_len = u16_at(d, at + 30).ok_or(Error::Malformed)? as usize;
        let comment_len = u16_at(d, at + 32).ok_or(Error::Malformed)? as usize;
        let disk = u16_at(d, at + 34).ok_or(Error::Malformed)?;
        let local = u32_at(d, at + 42).ok_or(Error::Malformed)?;
        if flags & 1 != 0 || flags & 0x40 != 0 { return Err(Error::Encrypted); }
        if compressed == 0xFFFF_FFFF || size == 0xFFFF_FFFF || local == 0xFFFF_FFFF || disk != 0 { return Err(Error::Unsupported); }
        if method != 0 && method != 8 { return Err(Error::Unsupported); }
        if name_len > self.limits.name { return Err(Error::Limit); }
        let name_bytes = d.get(at + CENTRAL_LEN..at + CENTRAL_LEN + name_len).ok_or(Error::Malformed)?;
        let name = core::str::from_utf8(name_bytes).map_err(|_| Error::Name)?;
        if !name_ok(name, self.limits.name) { return Err(Error::Name); }
        let (compressed, size) = (compressed as u64, size as u64);
        if size > self.limits.entry { return Err(Error::Limit); }
        if method == 0 && compressed != size { return Err(Error::Malformed); }
        if size > compressed.max(1).saturating_mul(self.limits.ratio) { return Err(Error::Limit); }
        let next = at + CENTRAL_LEN + name_len + extra_len + comment_len;
        if next > d.len() { return Err(Error::Malformed); }
        Ok((Entry { name, method, crc32, compressed, size, local: local as usize }, next))
    }

    pub fn len(&self) -> usize { self.count }

    /// The entries in directory order.
    pub fn entries(&self) -> impl Iterator<Item = Entry<'a>> + '_ {
        let mut at = self.directory;
        (0..self.count).map(move |_| { let (e, next) = self.entry_at(at).expect("checked by open"); at = next; e })
    }

    /// The entry named `name`.
    pub fn find(&self, name: &str) -> Option<Entry<'a>> { self.entries().find(|e| e.name == name) }

    /// Hands the entry's bytes to `sink` a piece at a time, and checks their size and CRC-32 at the end. Bytes already
    /// given to the sink are not taken back on an error: the caller writes to a temporary name and keeps it only on Ok.
    pub fn read(&self, entry: &Entry<'_>, sink: &mut dyn FnMut(&[u8]) -> bool) -> Result<(), Error> {
        let d = self.data;
        let at = entry.local;
        if at >= self.directory || u32_at(d, at) != Some(LOCAL) { return Err(Error::Malformed); }
        let method = u16_at(d, at + 8).ok_or(Error::Malformed)?;
        let name_len = u16_at(d, at + 26).ok_or(Error::Malformed)? as usize;
        let extra_len = u16_at(d, at + 28).ok_or(Error::Malformed)? as usize;
        if method != entry.method || d.get(at + LOCAL_LEN..at + LOCAL_LEN + name_len) != Some(entry.name.as_bytes()) { return Err(Error::Malformed); }
        let start = at + LOCAL_LEN + name_len + extra_len;
        let end = start.checked_add(entry.compressed as usize).ok_or(Error::Malformed)?;
        if end > self.directory { return Err(Error::Malformed); }
        let body = &d[start..end];
        let (mut crc, mut produced, mut over, mut stopped) = (0u32, 0u64, false, false);
        let mut feed = |piece: &[u8]| -> bool {
            produced += piece.len() as u64;
            if produced > entry.size { over = true; return false; }
            crc = crc32_update(crc, piece);
            if !sink(piece) { stopped = true; return false; }
            true
        };
        match entry.method {
            0 => { feed(body); }
            _ => { if crate::inflate::inflate(body, &mut feed).is_err() && !over && !stopped { return Err(Error::Corrupt); } }
        }
        if stopped { return Err(Error::Sink); }
        if over || produced != entry.size || crc != entry.crc32 { return Err(Error::Corrupt); }
        Ok(())
    }
}
