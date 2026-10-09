//! Release metadata (351-UPD-0013, docs/update/publishing.md): a channel file and a boot manifest. The parser service
//! reads them (`parse_channel`, `Manifest`); the updater, which holds the update zone, parses none of it and accepts
//! what the parser says only if it encodes again to exactly the signed bytes (`encode_channel`, `encode_line`,
//! `encode_file`), so a parser that lies is caught (MC-11.5, 11.11). Both formats have one encoding, which `parse_*`
//! demands: anything else is malformed.

/// The largest channel file and boot manifest read.
pub const CHANNEL_MAX: usize = 1024;
pub const MANIFEST_MAX: usize = 32768;
/// A channel's name, an architecture's name, a manifest's path: their longest.
pub const NAME_MAX: usize = 32;
pub const ARCH_MAX: usize = 16;
pub const PATH_MAX: usize = 96;
/// Architectures a channel names, at most; header lines of a manifest (its format line included), at most, and their length.
pub const ARCHES: usize = 4;
pub const HEADERS: usize = 8;
pub const HEADER_MAX: usize = 128;
/// The first line of a manifest of format 1.
pub const FORMAT_LINE: &str = "MIND-MANIFEST 1";
/// A file line encodes to at most this many bytes, without its newline.
pub const FILE_LINE_MAX: usize = 5 + PATH_MAX + 1 + 20 + 1 + 64 + 1 + 8 + 1 + 10;

/// The bytes are not of the form asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Malformed;

/// A short ASCII text of at most N bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Text<const N: usize> { bytes: [u8; N], len: usize }
impl<const N: usize> Text<N> {
    pub const EMPTY: Self = Self { bytes: [0; N], len: 0 };
    /// `text`, if it fits and every byte is printable ASCII other than a space.
    pub fn new(text: &[u8]) -> Option<Self> {
        if text.len() > N || !text.iter().all(|b| b.is_ascii_graphic()) { return None; }
        let mut bytes = [0; N];
        bytes[..text.len()].copy_from_slice(text);
        Some(Self { bytes, len: text.len() })
    }
    pub fn as_bytes(&self) -> &[u8] { &self.bytes[..self.len] }
    pub fn as_str(&self) -> &str { core::str::from_utf8(self.as_bytes()).unwrap_or("") }
}

/// What a channel file says: its latest version, the minimum a device may run, when it expires
/// (`YYYY-MM-DDTHH:MM:SSZ`, UTC), and the SHA-256 of each architecture's boot manifest, by architecture in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Channel {
    pub name: Text<NAME_MAX>,
    pub version: u64,
    pub minimum: u64,
    pub expires: [u8; 20],
    pub manifests: [(Text<ARCH_MAX>, [u8; 32]); ARCHES],
    pub count: usize,
}

impl Channel {
    /// The manifest digest for `arch`.
    pub fn manifest(&self, arch: &str) -> Option<[u8; 32]> {
        self.manifests[..self.count].iter().find(|(a, _)| a.as_bytes() == arch.as_bytes()).map(|&(_, digest)| digest)
    }
}

/// Where the signature is: it covers the file's first `body` bytes (its first line with the newline).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Signed { pub body: usize, pub signature: [u8; 64] }

// Name characters: those json.dumps writes as they are and a file name holds anywhere.
fn word_byte(b: u8) -> bool { b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-' }

fn hex_value(b: u8) -> Option<u8> {
    match b { b'0'..=b'9' => Some(b - b'0'), b'a'..=b'f' => Some(b - b'a' + 10), _ => None }
}

// Lower-case hex of exactly 2 × N digits.
fn hex<const N: usize>(text: &[u8]) -> Option<[u8; N]> {
    if text.len() != 2 * N { return None; }
    let mut out = [0u8; N];
    for (i, pair) in text.chunks_exact(2).enumerate() { out[i] = hex_value(pair[0])? << 4 | hex_value(pair[1])?; }
    Some(out)
}

// A decimal of at most 20 digits without a leading zero (0 itself is "0").
fn decimal(text: &[u8]) -> Option<u64> {
    if text.is_empty() || text.len() > 20 || (text.len() > 1 && text[0] == b'0') || !text.iter().all(u8::is_ascii_digit) { return None; }
    text.iter().try_fold(0u64, |n, &d| n.checked_mul(10)?.checked_add((d - b'0') as u64))
}

// `YYYY-MM-DDTHH:MM:SSZ` with every field in range (days by month, leap years included).
fn timestamp(text: &[u8]) -> Option<[u8; 20]> {
    let t: [u8; 20] = text.try_into().ok()?;
    let shape = t.iter().enumerate().all(|(i, &b)| match i { 4 | 7 => b == b'-', 10 => b == b'T', 13 | 16 => b == b':', 19 => b == b'Z', _ => b.is_ascii_digit() });
    if !shape { return None; }
    let n = |a: usize, b: usize| t[a..b].iter().fold(0u32, |n, &d| n * 10 + (d - b'0') as u32);
    let (year, month, day) = (n(0, 4), n(5, 7), n(8, 10));
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month { 1 | 3 | 5 | 7 | 8 | 10 | 12 => 31, 4 | 6 | 9 | 11 => 30, 2 if leap => 29, 2 => 28, _ => return None };
    (day >= 1 && day <= days && n(11, 13) < 24 && n(14, 16) < 60 && n(17, 19) < 60).then_some(t)
}

/// Seconds since 1970-01-01T00:00:00Z of a channel's expiry, to compare with the clock.
pub fn expires_unix(expires: &[u8; 20]) -> u64 {
    let n = |a: usize, b: usize| expires[a..b].iter().fold(0i64, |n, &d| n * 10 + (d - b'0') as i64);
    // Days from civil (Howard Hinnant's algorithm).
    let (y, m, d) = (n(0, 4) - (n(5, 7) <= 2) as i64, n(5, 7), n(8, 10));
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let days = era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719468;
    (days * 86400 + n(11, 13) * 3600 + n(14, 16) * 60 + n(17, 19)).max(0) as u64
}

// A cursor over bytes that must be exactly what is expected.
struct Scan<'a> { text: &'a [u8], at: usize }
impl<'a> Scan<'a> {
    fn literal(&mut self, s: &[u8]) -> Option<()> {
        (self.text.get(self.at..self.at + s.len())? == s).then(|| self.at += s.len())
    }
    fn until(&mut self, stop: u8) -> Option<&'a [u8]> {
        let rest = &self.text[self.at..];
        let n = rest.iter().position(|&b| b == stop)?;
        self.at += n;
        Some(&rest[..n])
    }
    fn digits(&mut self) -> Option<u64> {
        let rest = &self.text[self.at..];
        let n = rest.iter().position(|b| !b.is_ascii_digit()).unwrap_or(rest.len());
        self.at += n;
        decimal(&rest[..n])
    }
    fn string<const N: usize>(&mut self) -> Option<Text<N>> {
        self.literal(b"\"")?;
        let word = self.until(b'"')?;
        self.literal(b"\"")?;
        if word.is_empty() || !word.iter().all(|&b| word_byte(b)) { return None; }
        Text::new(word)
    }
}

/// A channel file: `{"channel":…,"expires":…,"manifests":{…},"minimum":…,"version":…}`, keys and architectures in order,
/// no spaces, then `ed25519 <128 hex digits>`, each line ending in a newline and nothing after. Its version and minimum
/// are checked as `scripts/release.py` makes them (1 ≤ minimum ≤ version).
pub fn parse_channel(file: &[u8]) -> Result<(Channel, Signed), Malformed> {
    let body = file.iter().position(|&b| b == b'\n').ok_or(Malformed)? + 1;
    let signature = file[body..].strip_prefix(b"ed25519 ").and_then(|s| s.strip_suffix(b"\n")).and_then(hex::<64>).ok_or(Malformed)?;
    let mut s = Scan { text: &file[..body], at: 0 };
    let parsed = (|| {
        s.literal(b"{\"channel\":")?;
        let name = s.string::<NAME_MAX>()?;
        s.literal(b",\"expires\":\"")?;
        let expires = timestamp(s.until(b'"')?)?;
        s.literal(b"\",\"manifests\":{")?;
        let mut manifests = [(Text::EMPTY, [0u8; 32]); ARCHES];
        let mut count = 0;
        loop {
            if count == ARCHES { return None; }
            let arch = s.string::<ARCH_MAX>()?;
            if count > 0 && arch.as_bytes() <= manifests[count - 1].0.as_bytes() { return None; }
            s.literal(b":\"")?;
            let digest = hex::<32>(s.until(b'"')?)?;
            s.literal(b"\"")?;
            manifests[count] = (arch, digest);
            count += 1;
            if s.literal(b"}").is_some() { break; }
            s.literal(b",")?;
        }
        s.literal(b",\"minimum\":")?;
        let minimum = s.digits()?;
        s.literal(b",\"version\":")?;
        let version = s.digits()?;
        s.literal(b"}\n")?;
        (s.at == body && 1 <= minimum && minimum <= version).then_some(Channel { name, version, minimum, expires, manifests, count })
    })();
    parsed.map(|channel| (channel, Signed { body, signature })).ok_or(Malformed)
}

// Bytes into a buffer, failing when it is full.
struct Out<'a> { buf: &'a mut [u8], len: usize }
impl Out<'_> {
    fn put(&mut self, bytes: &[u8]) -> Option<()> {
        let end = self.len.checked_add(bytes.len()).filter(|&end| end <= self.buf.len())?;
        self.buf[self.len..end].copy_from_slice(bytes);
        self.len = end;
        Some(())
    }
    fn hex(&mut self, bytes: &[u8]) -> Option<()> {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        bytes.iter().try_for_each(|&b| self.put(&[DIGITS[(b >> 4) as usize], DIGITS[(b & 15) as usize]]))
    }
    fn decimal(&mut self, mut n: u64) -> Option<()> {
        let mut digits = [0u8; 20];
        let mut at = digits.len();
        loop { at -= 1; digits[at] = b'0' + (n % 10) as u8; n /= 10; if n == 0 { break; } }
        self.put(&digits[at..])
    }
}

/// The first line of the channel's file, with its newline, in its one encoding: what the release key signs.
pub fn encode_channel(channel: &Channel, out: &mut [u8]) -> Option<usize> {
    let mut o = Out { buf: out, len: 0 };
    o.put(b"{\"channel\":\"")?; o.put(channel.name.as_bytes())?;
    o.put(b"\",\"expires\":\"")?; o.put(&channel.expires)?;
    o.put(b"\",\"manifests\":{")?;
    for (i, (arch, digest)) in channel.manifests[..channel.count.min(ARCHES)].iter().enumerate() {
        if i > 0 { o.put(b",")?; }
        o.put(b"\"")?; o.put(arch.as_bytes())?; o.put(b"\":\"")?; o.hex(digest)?; o.put(b"\"")?;
    }
    o.put(b"},\"minimum\":")?; o.decimal(channel.minimum)?;
    o.put(b",\"version\":")?; o.decimal(channel.version)?;
    o.put(b"}\n")?;
    Some(o.len)
}

/// A file line of a boot manifest: `file <path> <size> <sha256> <flags in 8 hex digits> <MiB>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct File { pub path: Text<PATH_MAX>, pub size: u64, pub digest: [u8; 32], pub flags: u32, pub mib: u32 }

fn parse_file(line: &[u8]) -> Option<File> {
    let mut fields = line.split(|&b| b == b' ');
    if fields.next()? != b"file" { return None; }
    let path = fields.next()?;
    if path.is_empty() || path.starts_with(b"/") || path.split(|&b| b == b'/').any(|p| p.is_empty() || p == b"." || p == b".." || !p.iter().all(|&b| word_byte(b))) { return None; }
    let file = File {
        path: Text::new(path)?,
        size: decimal(fields.next()?)?,
        digest: hex::<32>(fields.next()?)?,
        flags: u32::from_be_bytes(hex::<4>(fields.next()?)?),
        mib: u32::try_from(decimal(fields.next()?)?).ok()?,
    };
    fields.next().is_none().then_some(file)
}

// The path of a file line that parsed.
fn file_path(line: &[u8]) -> &[u8] { line[5..].split(|&b| b == b' ').next().unwrap_or(b"") }

/// A file line, without its newline, in its one encoding.
pub fn encode_file(file: &File, out: &mut [u8]) -> Option<usize> {
    let mut o = Out { buf: out, len: 0 };
    o.put(b"file ")?; o.put(file.path.as_bytes())?; o.put(b" ")?; o.decimal(file.size)?; o.put(b" ")?;
    o.hex(&file.digest)?; o.put(b" ")?; o.hex(&file.flags.to_be_bytes())?; o.put(b" ")?; o.decimal(file.mib as u64)?;
    Some(o.len)
}

/// A boot manifest of format 1, checked whole: its format line, then up to `HEADERS` - 1 more header lines
/// (`key value`, each of printable ASCII), then its file lines, in strictly increasing order of their paths' components
/// (`scripts/sign_manifest.py`'s order). Every line ends in
/// a newline. Header lines are kept as text: the bootloader and the updater read none but the format, and the updater
/// writes the manifest into the slot as it was signed.
pub struct Manifest<'a> { text: &'a [u8], headers: usize, files: usize }

impl<'a> Manifest<'a> {
    pub fn parse(text: &'a [u8]) -> Result<Self, Malformed> {
        if text.len() > MANIFEST_MAX || !text.ends_with(b"\n") { return Err(Malformed); }
        let (mut headers, mut files) = (0, 0);
        let mut last: Option<&[u8]> = None;
        for line in text[..text.len() - 1].split(|&b| b == b'\n') {
            if headers == 0 {
                if line != FORMAT_LINE.as_bytes() { return Err(Malformed); }
                headers = 1;
            } else if line.starts_with(b"file ") {
                parse_file(line).ok_or(Malformed)?;
                // The build's order: by path components, as Python sorts paths (`voice/model.bin` before `voice.elf`).
                let parts = |path: &'a [u8]| path.split(|&b| b == b'/');
                if last.is_some_and(|previous| parts(previous).cmp(parts(file_path(line))) != core::cmp::Ordering::Less) { return Err(Malformed); }
                last = Some(file_path(line));
                files += 1;
            } else {
                // A header line after a file line, or one that is not `key value` of printable ASCII.
                let key = line.split(|&b| b == b' ').next().unwrap_or(b"");
                if files > 0 || headers == HEADERS || line.len() > HEADER_MAX || key.is_empty() || !key.iter().all(|&b| word_byte(b)) || key.len() == line.len()
                    || !line.iter().all(|&b| b == b' ' || b.is_ascii_graphic()) { return Err(Malformed); }
                headers += 1;
            }
        }
        Ok(Self { text, headers, files })
    }

    fn lines(&self) -> impl Iterator<Item = &'a [u8]> { self.text[..self.text.len() - 1].split(|&b| b == b'\n') }
    pub fn headers(&self) -> usize { self.headers }
    pub fn files(&self) -> usize { self.files }
    /// Header line `i` (0: the format line), without its newline.
    pub fn header(&self, i: usize) -> Option<&'a [u8]> { (i < self.headers).then(|| self.lines().nth(i)).flatten() }
    /// File line `i`, in the manifest's order.
    pub fn file(&self, i: usize) -> Option<File> { (i < self.files).then(|| self.lines().nth(self.headers + i).and_then(parse_file)).flatten() }
}
