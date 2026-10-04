//! The ring of `logd`: 256 records of up to 200 bytes of text in fixed slots (64 KiB), numbered in sequence, and a
//! rate limit per sender (MC-10.2): what a sender writes beyond `RATE` records a second is refused and counted, and a
//! record saying how many were refused comes before its next one. Builds on the host (tests/logd_host.rs).

pub const SLOTS: usize = 256;
pub const TEXT: usize = 200;
pub const NAME: usize = 16;
/// Records a sender may write per second.
pub const RATE: u32 = 64;
const SENDERS: usize = 32;
pub const LEVEL_WARN: u8 = 2;

#[derive(Clone, Copy)]
pub struct Record { pub seq: u64, pub time_ms: u64, pub pid: u64, pub name: [u8; NAME], pub level: u8, len: u8, text: [u8; TEXT] }

impl Record {
    const EMPTY: Record = Record { seq: 0, time_ms: 0, pid: 0, name: [0; NAME], level: 0, len: 0, text: [0; TEXT] };
    pub fn text(&self) -> &str { core::str::from_utf8(&self.text[..self.len as usize]).unwrap_or("") }
    pub fn name(&self) -> &str {
        let end = self.name.iter().position(|&b| b == 0).unwrap_or(NAME);
        core::str::from_utf8(&self.name[..end]).unwrap_or("?")
    }
}

// The longest prefix of `text` that fits `max` bytes and ends on a character boundary, without line endings.
fn clip(text: &str, max: usize) -> &str {
    let text = text.trim_end_matches(['\n', '\r']);
    let mut end = text.len().min(max);
    while !text.is_char_boundary(end) { end -= 1; }
    &text[..end]
}

#[derive(Clone, Copy, Default)]
struct Sender { pid: u64, window: u64, count: u32, suppressed: u32 }

pub struct Ring { records: [Record; SLOTS], next: u64, dropped: u64, suppressed: u64, senders: [Sender; SENDERS] }

impl Default for Ring { fn default() -> Self { Self::new() } }

impl Ring {
    pub const fn new() -> Self { Self { records: [Record::EMPTY; SLOTS], next: 0, dropped: 0, suppressed: 0, senders: [Sender { pid: 0, window: 0, count: 0, suppressed: 0 }; SENDERS] } }
    /// The oldest record kept.
    pub fn first(&self) -> u64 { self.next.saturating_sub(SLOTS as u64) }
    /// The sequence number of the next record.
    pub fn next(&self) -> u64 { self.next }
    /// Records overwritten to make room.
    pub fn dropped(&self) -> u64 { self.dropped }
    /// Records refused because their sender wrote too fast.
    pub fn suppressed(&self) -> u64 { self.suppressed }

    pub fn get(&self, seq: u64) -> Option<&Record> {
        (seq >= self.first() && seq < self.next).then(|| &self.records[(seq % SLOTS as u64) as usize])
    }

    fn append(&mut self, time_ms: u64, pid: u64, name: &str, level: u8, text: &str) {
        if self.next >= SLOTS as u64 { self.dropped += 1; }
        let record = &mut self.records[(self.next % SLOTS as u64) as usize];
        let text = clip(text, TEXT);
        let name = clip(name, NAME);
        *record = Record { seq: self.next, time_ms, pid, name: [0; NAME], level: level.min(3), len: text.len() as u8, text: [0; TEXT] };
        record.name[..name.len()].copy_from_slice(name.as_bytes());
        record.text[..text.len()].copy_from_slice(text.as_bytes());
        self.next += 1;
    }

    /// Adds a record from `pid` (`name` as the kernel reports it); false if the sender is over its rate and the
    /// record was refused.
    pub fn push(&mut self, time_ms: u64, pid: u64, name: &str, level: u8, text: &str) -> bool {
        let window = time_ms / 1000;
        // The sender's entry, or the one used longest ago.
        let index = match self.senders.iter().position(|s| s.pid == pid) {
            Some(index) => index,
            None => {
                let index = (0..SENDERS).min_by_key(|&i| (self.senders[i].pid != 0, self.senders[i].window)).unwrap();
                self.senders[index] = Sender { pid, window, count: 0, suppressed: 0 };
                index
            }
        };
        let sender = &mut self.senders[index];
        let mut refused = 0;
        if sender.window != window { refused = sender.suppressed; *sender = Sender { pid, window, count: 0, suppressed: 0 }; }
        if sender.count >= RATE { sender.suppressed += 1; self.suppressed += 1; return false; }
        sender.count += 1;
        if refused > 0 {
            let mut note = [0u8; 64];
            let len = format_note(&mut note, refused);
            self.append(time_ms, pid, name, LEVEL_WARN, core::str::from_utf8(&note[..len]).unwrap_or(""));
        }
        self.append(time_ms, pid, name, level, text);
        true
    }
}

// Formats into a fixed buffer (no allocation in the service).
struct Buf<'a> { out: &'a mut [u8], len: usize }
impl core::fmt::Write for Buf<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let n = s.len().min(self.out.len() - self.len);
        self.out[self.len..self.len + n].copy_from_slice(&s.as_bytes()[..n]);
        self.len += n;
        Ok(())
    }
}

fn format_note(out: &mut [u8; 64], count: u32) -> usize {
    let mut buf = Buf { out, len: 0 };
    let _ = core::fmt::Write::write_fmt(&mut buf, format_args!("{} records refused: more than {} a second", count, RATE));
    buf.len
}
