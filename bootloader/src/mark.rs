// The loader's mark on its volume (000-KRN-0211): how far the last boot got, for a boot that left no log. Free of UEFI so
// the host tests (tests/boot_mark_host.rs) check the one encoding; scripts/boot_switch.py reads it from Linux.

/// The file, in the slots' directory: no volume without slots gets one.
pub const FILE: &str = "MIND\\REACHED";
pub const DIR: &str = "MIND";
/// Its size: one line, padded with spaces, written whole in place.
pub const LEN: usize = 48;
const TAG: &str = "MIND-REACHED";
const FORMAT: &str = "1";

/// The points of a boot the mark records, in order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stage {
    /// The loader at EFI/BOOT/ started; the boot count is one more than the last.
    Started,
    /// The kernel and the boot services were read and checked against the signed manifest.
    Read,
    /// Boot services end next and the kernel starts.
    Handover,
}

impl Stage {
    pub fn name(self) -> &'static str {
        match self { Stage::Started => "STARTED", Stage::Read => "READ", Stage::Handover => "HANDOVER" }
    }
}

/// The boot count a mark holds; None for no mark, or one of another format.
pub fn count(data: &[u8]) -> Option<u32> {
    let line = data.split(|&b| b == b'\n').next()?;
    let mut words = core::str::from_utf8(line).ok()?.split_ascii_whitespace();
    if (words.next(), words.next()) != (Some(TAG), Some(FORMAT)) { return None; }
    let digits = words.next().filter(|d| d.bytes().all(|b| b.is_ascii_digit()))?;
    let count = digits.parse().ok()?;
    match words.next() { Some("STARTED" | "READ" | "HANDOVER") => {} _ => return None }
    words.next().is_none().then_some(count)
}

/// The count of the boot that starts after a mark holding `last`: never 0, which no mark holds.
pub fn next(last: Option<u32>) -> u32 { last.unwrap_or(0).checked_add(1).unwrap_or(1) }

/// The mark: `MIND-REACHED 1 <count> <stage>`, spaces to its size, and a newline.
pub fn encode(count: u32, stage: Stage) -> [u8; LEN] {
    let mut out = [b' '; LEN];
    out[LEN - 1] = b'\n';
    let mut n = 0;
    let mut put = |bytes: &[u8]| for &b in bytes { if n < LEN - 1 { out[n] = b; n += 1; } };
    put(TAG.as_bytes());
    put(b" ");
    put(FORMAT.as_bytes());
    put(b" ");
    let mut digits = [0u8; 10];
    let mut d = digits.len();
    let mut v = count;
    loop { d -= 1; digits[d] = b'0' + (v % 10) as u8; v /= 10; if v == 0 { break; } }
    put(&digits[d..]);
    put(b" ");
    put(stage.name().as_bytes());
    out
}
