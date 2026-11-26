//! The Bluetooth sink (700-DRV-0110, audio.wit 1.3): while bt_hci's A2DP stream runs and it has taken the output, the
//! gateway's output goes into this ring instead of to the machine's own device, and bt_hci reads it. 16-bit stereo at
//! the gateway's rate, as much as the gateway's own DMA ring holds (32 buffers of 4 KiB, 0.68 s at 48 kHz), so its
//! waiters see the same room; the level scales the samples. Self-contained: a host test includes it by its path.

pub const FRAME: usize = 4;
pub const RING: usize = 32 * 4096;
/// A sink not read this long is taken as gone, and the machine's own output comes back.
pub const SILENT_MS: u64 = 2_000;

/// The ring, apart from the sink, as usbaudio keeps its queue: there is one sink at a time.
static mut RING_BYTES: [u8; RING] = [0; RING];
fn ring() -> &'static mut [u8; RING] { unsafe { &mut *core::ptr::addr_of_mut!(RING_BYTES) } }

pub struct Sink { head: usize, len: usize, pub reader: u64, pub read_at: u64, pub out_factor: u32, pub buffer: usize }

impl Sink {
    /// A sink for `reader` (its task), taking the output at `now`; `buffer` is the gateway's DMA buffer size, the unit
    /// of `free`.
    pub fn new(reader: u64, now: u64, buffer: usize) -> Self { Self { head: 0, len: 0, reader, read_at: now, out_factor: 1 << 15, buffer } }

    /// Free buffers, as the gateway's waiters count them, one kept back as a device's ring keeps one.
    pub fn free(&self, buffers: usize) -> usize { ((RING - self.len) / self.buffer).min(buffers - 1) }

    fn push_frame(&mut self, frame: [u8; FRAME]) {
        let at = (self.head + self.len) % RING;
        for (i, b) in frame.iter().enumerate() { ring()[(at + i) % RING] = *b; }
        self.len += FRAME;
    }

    fn scale(&self, sample: i16) -> i16 { ((sample as i32 * self.out_factor as i32) >> 15).clamp(i16::MIN as i32, i16::MAX as i32) as i16 }

    /// Whole frames of `pcm` taken while there is room, scaled by the volume; the bytes taken.
    pub fn play(&mut self, pcm: &[u8]) -> usize {
        let take = pcm.len().min(RING - self.len) & !(FRAME - 1);
        for f in pcm[..take].chunks_exact(FRAME) {
            let (l, r) = (self.scale(i16::from_le_bytes([f[0], f[1]])), self.scale(i16::from_le_bytes([f[2], f[3]])));
            let (l, r) = (l.to_le_bytes(), r.to_le_bytes());
            self.push_frame([l[0], l[1], r[0], r[1]]);
        }
        take
    }

    /// A tone of `hz` for `ms` (at most 5 s, and the room there is) at `rate`; the frames queued.
    pub fn tone(&mut self, hz: usize, ms: usize, rate: usize, sine: fn(u32) -> i16) -> usize {
        let frames = (rate * ms.min(5000) / 1000).min((RING - self.len) / FRAME);
        let step = (hz.clamp(20, 20_000) as u32) * 64 * 256 / rate as u32;
        let mut phase = 0u32;
        for _ in 0..frames {
            let s = self.scale(sine(phase / 256)).to_le_bytes();
            phase = phase.wrapping_add(step);
            self.push_frame([s[0], s[1], s[0], s[1]]);
        }
        frames
    }

    pub fn reset(&mut self) { (self.head, self.len) = (0, 0); }

    /// Queued PCM into `out`, whole frames, at `now`; the bytes copied.
    pub fn read(&mut self, out: &mut [u8], now: u64) -> usize {
        self.read_at = now;
        let n = self.len.min(out.len()) & !(FRAME - 1);
        for (i, b) in out[..n].iter_mut().enumerate() { *b = ring()[(self.head + i) % RING]; }
        self.head = (self.head + n) % RING;
        self.len -= n;
        n
    }

    /// Whether its reader stopped reading: then the output goes back to the machine.
    pub fn silent(&self, now: u64) -> bool { now > self.read_at + SILENT_MS }
}
