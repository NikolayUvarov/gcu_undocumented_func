//! One bounded, printable audit line per gateway request, written before the reply (750-EFF-0003, MC-10.3, MC-10.6).
//! The gateway never holds the agent's token or keys, so nothing it audits can carry them.

pub const LINE_MAX: usize = 256;

/// What one request was and how it ended. `decision` is `allow` or the denial; `result` the outcome or the error.
#[derive(Clone, Copy, Debug)]
pub struct Entry<'a> {
    pub uptime_ms: u64,
    pub command: &'a str,
    pub request: &'a str,
    pub decision: &'a str,
    pub result: &'a str,
    pub initiator: &'a str,
    pub reason: &'a str,
}

/// A formatted line: `EFFECTOR <ms> <command> <request> <decision> <result> by="<initiator>" why="<reason>"`.
#[derive(Clone, Copy)]
pub struct Line { buf: [u8; LINE_MAX], len: usize, pub cut: bool }
impl Line {
    pub fn as_bytes(&self) -> &[u8] { &self.buf[..self.len] }
    pub fn as_str(&self) -> &str { core::str::from_utf8(self.as_bytes()).unwrap_or("") }
    fn push(&mut self, b: u8) -> bool {
        if self.len == LINE_MAX { self.cut = true; return false; }
        self.buf[self.len] = b; self.len += 1; true
    }
    fn raw(&mut self, s: &str) { for &b in s.as_bytes() { if !self.push(b) { return; } } }
    /// A field: printable ASCII kept, `"`, `\` and anything else shown as `?`, at most `max` bytes, `~` when cut.
    fn field(&mut self, s: &str, max: usize, bare: bool) {
        if s.is_empty() && bare { self.raw("-"); return; }
        let mut n = 0;
        for c in s.chars() {
            if n == max { self.cut = true; self.push(b'~'); return; }
            let keep = (c.is_ascii_graphic() && c != '"' && c != '\\') || (!bare && c == ' ');
            let b = if keep { c as u8 } else { b'?' };
            if !self.push(b) { return; }
            n += 1;
        }
    }
}

fn decimal(mut v: u64, out: &mut [u8; 20]) -> &str {
    let mut i = out.len();
    loop { i -= 1; out[i] = b'0' + (v % 10) as u8; v /= 10; if v == 0 { break; } }
    core::str::from_utf8(&out[i..]).unwrap()
}

pub fn format(e: &Entry<'_>) -> Line {
    let mut l = Line { buf: [0; LINE_MAX], len: 0, cut: false };
    let mut digits = [0u8; 20];
    l.raw("EFFECTOR ");
    l.raw(decimal(e.uptime_ms, &mut digits));
    for (s, max) in [(e.command, 64), (e.request, 48), (e.decision, 16), (e.result, 32)] {
        l.raw(" ");
        l.field(s, max, true);
    }
    l.raw(" by=\"");
    l.field(e.initiator, 40, false);
    l.raw("\" why=\"");
    l.field(e.reason, LINE_MAX, false);
    if l.len < LINE_MAX { l.raw("\""); } else { l.cut = true; l.buf[LINE_MAX - 2] = b'~'; l.buf[LINE_MAX - 1] = b'"'; }
    l
}
