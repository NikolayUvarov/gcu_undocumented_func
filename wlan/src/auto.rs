// The automatic join (550-NET-0041): when the station tries a remembered network by itself, and which one. No I/O.
/// The first two minutes of a search: a try every 10 s; then one a minute.
pub const FIRST_MS: u64 = 10_000;
pub const FIRST_SPAN_MS: u64 = 120_000;
pub const LATER_MS: u64 = 60_000;

/// Whether the station joins by itself (on at start; `wifi leave` turns it off until the station next joins), and the
/// search under way: its tries since the network was last joined, when it began, and when the next try is due.
pub struct Auto { pub on: bool, pub tries: u32, pub since: u64, pub next: u64 }

impl Auto {
    pub const fn new() -> Self { Self { on: true, tries: 0, since: 0, next: 0 } }

    /// Whether a try is due at `now`.
    pub fn due(&self, now: u64) -> bool { self.on && now >= self.next }

    /// A try begins: its number in this search.
    pub fn begin(&mut self, now: u64) -> u32 {
        if self.tries == 0 { self.since = now; }
        self.tries += 1;
        self.tries
    }

    /// A try ended without a join: how long until the next one.
    pub fn ended(&mut self, now: u64) -> u64 {
        let wait = if now.saturating_sub(self.since) < FIRST_SPAN_MS { FIRST_MS } else { LATER_MS };
        self.next = now + wait;
        wait
    }

    /// Nothing to try (no network remembered, or `keystore` not answering): look again a first wait later, no try counted.
    pub fn idle(&mut self, now: u64) { self.next = now + FIRST_MS; }

    /// The station joined a network: on again, and a loss starts a new search at once.
    pub fn joined(&mut self) { *self = Self::new(); }

    /// `wifi leave`: no try until the station next joins.
    pub fn left(&mut self) { self.on = false; }
}

/// Which network to try of those `heard` (name, weakness as minus dBm, 0 unknown) that `remembered` holds: `previous`
/// (the one last joined) when it is heard, else the strongest.
pub fn choose<'a>(heard: impl Iterator<Item = (&'a [u8], u8)>, remembered: impl Fn(&[u8]) -> bool, previous: &[u8]) -> Option<&'a [u8]> {
    let mut best: Option<(&'a [u8], u8)> = None;
    for (ssid, weakness) in heard.filter(|(ssid, _)| !ssid.is_empty() && remembered(ssid)) {
        if !previous.is_empty() && ssid == previous { return Some(ssid); }
        let rank = if weakness == 0 { u8::MAX } else { weakness };
        if best.is_none_or(|(_, r)| rank < r) { best = Some((ssid, rank)); }
    }
    best.map(|(ssid, _)| ssid)
}
