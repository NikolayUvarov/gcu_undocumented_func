// A USB Audio Class 1 device through usb_host (000-DRV-0073): a speaker, microphone or speakerphone such as the
// STM-Speaker. While one is plugged in it takes the gateway's place of the HD Audio codec; the codec comes back when it
// goes. Playback goes through the gateway's own queue as packets of a millisecond each, as many frames as the device's
// feedback asks for; capture comes as packets and is kept in whole buffers, as the DMA ring's.
use mind::abi::CAP_KIND_ENDPOINT;
use mind::ipc::Endpoint;
use mind::uac::{self, Pacer, Setting, FRAME};
use mind::usb::{Host, Interface};

use crate::levels;

/// Where init grants usb_host's client badged for audio: its SLOT_AUDIO_USB (213-KRN-0263).
pub const SLOT_USB: usize = 8;
/// A device not plugged in yet is looked for this often.
pub const LOOK_MS: u64 = 2000;
/// While a stream runs, the gateway looks at it this often: usb_host holds 32 packets, 32 ms.
pub const PUMP_MS: u32 = 4;
const BUFFER_BYTES: usize = super::BUFFER_BYTES;
// The playback queue: as many buffers as the DMA ring; the capture kept for its reader: 64 buffers, 1.3 s.
const QUEUE_BYTES: usize = super::BUFFERS * BUFFER_BYTES;
const KEPT_BYTES: usize = 64 * BUFFER_BYTES;
static mut QUEUE: [u8; QUEUE_BYTES] = [0; QUEUE_BYTES];
static mut KEPT: [u8; KEPT_BYTES] = [0; KEPT_BYTES];
// Packets built a call, and how long a stream that has nothing to play sends silence before it stops.
const PACKETS: usize = 8;
const IDLE_MS: u64 = 1000;
const CLAIMED: usize = 8;

// A device's streams: the handles of its streaming interfaces and the settings chosen.
struct Attached { name: [u16; 2], playback: Option<(u32, Setting)>, capture: Option<(u32, Setting)>, handles: [u32; CLAIMED], count: usize, high_speed: bool }

pub struct Usb {
    host: Host, attached: Option<Attached>, looked: u64,
    // Playback: the queue's oldest byte and length; the stream selected; when it last had something to play.
    head: usize, queued: usize, playing: bool, pacer: Pacer, fed_ms: u64, sent: u64, silent: u64,
    // Capture: the kept bytes (whole frames), an overflow not yet reported.
    capturing: bool, kept_head: usize, kept: usize, lost: bool,
    pub out_factor: u32, pub in_factor: u32,
}

impl Usb {
    /// usb_host's audio client, if init granted one; no device yet.
    pub fn open() -> Option<Self> {
        if mind::dev::cap_info(SLOT_USB).0 != CAP_KIND_ENDPOINT { return None; }
        let host = Host::new(Endpoint(SLOT_USB)).ok()?;
        Some(Self { host, attached: None, looked: 0, head: 0, queued: 0, playing: false, pacer: Pacer::new(1), fed_ms: 0, sent: 0, silent: 0,
                    capturing: false, kept_head: 0, kept: 0, lost: false, out_factor: 1 << 15, in_factor: 1 << 15 })
    }

    pub fn attached(&self) -> bool { self.attached.is_some() }

    /// An attached device with no stream running is asked for its device descriptor every LOOK_MS: one that went away
    /// answers no more, and the gateway goes back to the codec. A running stream finds that out by itself.
    pub fn check(&mut self) {
        let now = mind::time::uptime_ms() as u64;
        let Some(a) = self.attached.as_ref() else { return };
        if self.active() || now < self.looked + LOOK_MS { return; }
        self.looked = now;
        let first = a.handles[0];
        if let Err(e) = self.host.control(first, 0x80, 6, 0x0100, 0, 18) { self.gone(e); }
    }

    /// Claims an audio function's interfaces if usb_host has one, at most every LOOK_MS; true when one is attached now.
    pub fn look(&mut self) -> bool {
        let now = mind::time::uptime_ms() as u64;
        if self.attached.is_some() { return true; }
        if self.looked != 0 && now < self.looked + LOOK_MS { return false; }
        self.looked = now.max(1);
        let mut claimed = [(0u32, Interface::default()); CLAIMED];
        let mut count = 0;
        while count < CLAIMED { match self.host.claim() { Ok(c) => { claimed[count] = c; count += 1; } Err(_) => break } }
        if count == 0 { return false; }
        // The interfaces of the first device claimed (a handle's low byte names the interface); the others go back.
        let device = claimed[0].0 & !0xFF;
        for &(h, _) in claimed[..count].iter().filter(|(h, _)| h & !0xFF != device) { let _ = self.host.release(h); }
        let mine: [(u32, Interface); CLAIMED] = core::array::from_fn(|i| if i < count && claimed[i].0 & !0xFF == device { claimed[i] } else { (0, Interface::default()) });
        let first = mine[0];
        let read = self.host.control(first.0, 0x80, 6, 0x0200, 0, 9).ok().and_then(|_| {
            let total = u16::from_le_bytes([self.host.buffer()[2], self.host.buffer()[3]]).clamp(9, mind::usb::CONTROL_MAX as u16);
            self.host.control(first.0, 0x80, 6, 0x0200, 0, total).ok()
        });
        let function = read.and_then(|got| uac::parse(&self.host.buffer()[..got]));
        let Some(function) = function else {
            mind::println!("[AUDIO] USB {:04X}:{:04X}: NO 16-BIT 48 KHZ SETTING TO USE", first.1.vendor, first.1.product);
            return false; // its interfaces stay claimed, so it is not looked at again until it is plugged in again
        };
        let handle_of = |number: u8| mine.iter().find(|(h, i)| *h != 0 && i.number == number).map(|(h, _)| *h);
        let playback = function.playback.and_then(|s| handle_of(s.interface).map(|h| (h, s)));
        let capture = function.capture.and_then(|s| handle_of(s.interface).map(|h| (h, s)));
        let mut handles = [0u32; CLAIMED];
        let mut n = 0;
        for (h, _) in mine.iter().filter(|(h, _)| *h != 0) { handles[n] = *h; n += 1; }
        let speed = first.1.speed;
        let say = |s: Option<(u32, Setting)>| s.map_or((0, 0, 0, 0, false), |(_, s)| (s.interface, s.channels, s.endpoint, s.packet, s.feedback.is_some()));
        let (p, c) = (say(playback), say(capture));
        mind::println!("[AUDIO] USB {:04X}:{:04X} ATTACHED: PLAYBACK {} (INTERFACE {}, {} CHANNELS, ENDPOINT {:02X}, {} BYTES{}), CAPTURE {} (INTERFACE {}, {} CHANNELS, ENDPOINT {:02X}, {} BYTES)",
            first.1.vendor, first.1.product, if playback.is_some() { "YES" } else { "NO" }, p.0, p.1, p.2, p.3, if p.4 { ", FEEDBACK" } else { "" },
            if capture.is_some() { "YES" } else { "NO" }, c.0, c.1, c.2, c.3);
        self.attached = Some(Attached { name: [first.1.vendor, first.1.product], playback, capture, handles, count: n, high_speed: speed >= 3 });
        (self.head, self.queued, self.playing, self.capturing, self.kept, self.kept_head) = (0, 0, false, false, 0, 0);
        true
    }

    // The device went away (usb_host refused its handles): the gateway goes back to the codec.
    fn gone(&mut self, why: mind::Error) {
        if let Some(a) = self.attached.take() {
            mind::println!("[AUDIO] USB {:04X}:{:04X} GONE ({:?}): {} PACKETS PLAYED, {} OF THEM SILENCE", a.name[0], a.name[1], why, self.sent, self.silent);
            for &h in &a.handles[..a.count] { let _ = self.host.release(h); }
        }
        (self.playing, self.capturing, self.queued, self.kept, self.looked) = (false, false, 0, 0, 0);
    }


    /// Free buffers of the playback queue, as the DMA ring counts them (one kept as a gap).
    pub fn free(&self) -> usize { ((QUEUE_BYTES - self.queued) / BUFFER_BYTES).min(super::BUFFERS - 1) }
    pub fn active(&self) -> bool { self.playing || self.capturing }

    fn queue() -> &'static mut [u8; QUEUE_BYTES] { unsafe { &mut *core::ptr::addr_of_mut!(QUEUE) } }
    fn kept_area() -> &'static mut [u8; KEPT_BYTES] { unsafe { &mut *core::ptr::addr_of_mut!(KEPT) } }

    // Appends whole frames to the playback queue; the bytes taken.
    fn push(&mut self, pcm: &[u8]) -> usize {
        let take = pcm.len().min(QUEUE_BYTES - self.queued) & !(FRAME - 1);
        let queue = Self::queue();
        let mut at = (self.head + self.queued) % QUEUE_BYTES;
        for chunk in pcm[..take].chunks(QUEUE_BYTES - at) {
            queue[at..at + chunk.len()].copy_from_slice(chunk);
            at = (at + chunk.len()) % QUEUE_BYTES;
        }
        self.queued += take;
        take
    }

    pub fn play(&mut self, pcm: &[u8]) -> usize { let n = self.push(pcm); if n > 0 { self.start(); self.pump(); } n }

    pub fn tone(&mut self, hz: usize, ms: usize, sine: fn(u32) -> i16) -> usize {
        let frames = (super::AUDIO_RATE * ms.min(5000) / 1000).min((QUEUE_BYTES - self.queued) / FRAME);
        let step = (hz.clamp(20, 20_000) as u32) * 64 * 256 / super::AUDIO_RATE as u32;
        let mut phase = 0u32;
        let mut frame = [0u8; FRAME];
        for _ in 0..frames {
            let sample = sine(phase / 256).to_le_bytes(); phase = phase.wrapping_add(step);
            frame[..2].copy_from_slice(&sample); frame[2..].copy_from_slice(&sample);
            self.push(&frame);
        }
        if frames > 0 { self.start(); self.pump(); }
        frames
    }

    /// Flushes the playback queue; the stream stops.
    pub fn reset(&mut self) { self.queued = 0; self.stop_playback(); }

    // The playback setting selected, so its stream runs.
    fn start(&mut self) {
        if self.playing { return; }
        let Some((handle, s)) = self.attached.as_ref().and_then(|a| a.playback) else { return };
        match self.host.select(handle, s.alternate) {
            Ok(_) => {
                self.playing = true;
                self.pacer = Pacer::new(s.packet as usize / s.frame());
                self.fed_ms = mind::time::uptime_ms() as u64;
                mind::println!("[AUDIO] USB PLAYBACK STARTED: INTERFACE {} SETTING {}", s.interface, s.alternate);
            }
            Err(e) => { mind::println!("[AUDIO] USB PLAYBACK SETTING {} REFUSED ({:?})", s.alternate, e); if matches!(e, mind::Error::NotFound | mind::Error::Peer) { self.gone(e); } }
        }
    }

    fn stop_playback(&mut self) {
        if !self.playing { return; }
        self.playing = false;
        if let Some((handle, s)) = self.attached.as_ref().and_then(|a| a.playback) {
            let _ = self.host.select(handle, 0);
            mind::println!("[AUDIO] USB PLAYBACK STOPPED: INTERFACE {}; {} PACKETS SENT, {} OF THEM SILENCE, THE LAST RATE {}.{:03} FRAMES A MS",
                s.interface, self.sent, self.silent, self.pacer.rate() >> 16, (self.pacer.rate() & 0xFFFF) * 1000 >> 16);
        }
    }

    /// The streams' work: the feedback read, packets given while usb_host has room (silence once the queue is empty, and
    /// after IDLE_MS of it the stream stops), the capture's packets kept.
    pub fn pump(&mut self) {
        let Some(a) = self.attached.as_ref() else { return };
        let (playback, capture, high_speed) = (a.playback, a.capture, a.high_speed);
        if let (true, Some((handle, s))) = (self.playing, playback) {
            if let Some((address, _)) = s.feedback {
                let mut rate = None;
                match self.host.isochronous(handle, address, |packet, damaged| { if !damaged { if let Some(r) = uac::feedback(packet, high_speed) { rate = Some(r); } } }) {
                    Ok(_) => if let Some(r) = rate { self.pacer.set(r); },
                    Err(e) => { self.gone(e); return; }
                }
            }
            if let Err(e) = self.feed(handle, s) { self.gone(e); return; }
        }
        if let (true, Some((handle, s))) = (self.capturing, capture) {
            match self.host.isochronous(handle, s.endpoint, |_, _| {}) {
                // The packets stay in the client buffer after the call, each after its two length bytes.
                Ok(written) => {
                    let mut at = 0;
                    let mut wire = [0u8; 1024];
                    while at + 2 <= written {
                        let buffer = self.host.buffer();
                        let length = (u16::from_le_bytes([buffer[at], buffer[at + 1]]) & 0x7FFF) as usize;
                        if at + 2 + length > written { break; }
                        let n = length.min(wire.len());
                        wire[..n].copy_from_slice(&buffer[at + 2..at + 2 + n]);
                        at += 2 + length;
                        self.keep(&wire[..n], s.channels);
                    }
                }
                Err(e) => self.gone(e),
            }
        }
    }

    // Packets for the playback stream while usb_host takes them.
    fn feed(&mut self, handle: u32, s: Setting) -> mind::Result<()> {
        let now = mind::time::uptime_ms() as u64;
        loop {
            // Up to PACKETS packets at the buffer's start: two length bytes each, then the samples on the wire. A
            // packet's frames, or SILENT for one of silence.
            let mut at = 0;
            let mut built = [0usize; PACKETS];
            let mut count = 0;
            let mut taken_frames = 0; // the queue's frames in the packets built so far
            while count < PACKETS {
                let want = self.pacer.next();
                let frames = want.min(self.queued / FRAME - taken_frames);
                let silent = frames == 0;
                if silent && now > self.fed_ms + IDLE_MS { break; }
                let wire_frames = if silent { want } else { frames };
                let bytes = wire_frames * s.frame();
                let mut stereo = [0u8; 256 * FRAME];
                if !silent {
                    let start = (self.head + taken_frames * FRAME) % QUEUE_BYTES;
                    for f in 0..frames { let from = (start + f * FRAME) % QUEUE_BYTES; stereo[f * FRAME..(f + 1) * FRAME].copy_from_slice(&Self::queue()[from..from + FRAME]); }
                    levels::scale_bytes(&mut stereo[..frames * FRAME], self.out_factor);
                }
                let out = &mut self.host.buffer_mut()[at..at + 2 + bytes];
                out[..2].copy_from_slice(&(bytes as u16).to_le_bytes());
                uac::to_wire(&stereo[..wire_frames * FRAME], s.channels, &mut out[2..]);
                built[count] = if silent { SILENT } else { frames };
                if !silent { taken_frames += frames; }
                count += 1;
                at += 2 + bytes;
            }
            if count == 0 { self.stop_playback(); return Ok(()); }
            let taken = self.host.isochronous_out(handle, s.endpoint, 0, at)?;
            // The frames of the packets taken leave the queue; those not taken are built again next time, and the
            // pacer's frames for them are given back by building them anew (a frame at most is lost to its fraction).
            let (mut walked, mut used) = (0, 0);
            for &f in &built[..count] {
                if walked >= taken { break; }
                let length = u16::from_le_bytes([self.host.buffer()[walked], self.host.buffer()[walked + 1]]) as usize;
                walked += 2 + length;
                self.sent += 1;
                if f == SILENT { self.silent += 1; } else { used += f; self.fed_ms = now; }
            }
            self.head = (self.head + used * FRAME) % QUEUE_BYTES;
            self.queued -= used * FRAME;
            if taken < at { return Ok(()); }
        }
    }

    // A capture packet into the kept ring as the gateway's stereo frames; a full ring drops the oldest buffer.
    fn keep(&mut self, wire: &[u8], channels: u8) {
        let mut stereo = [0u8; 512 * FRAME / 2];
        let n = uac::from_wire(wire, channels, &mut stereo);
        levels::scale_bytes(&mut stereo[..n], self.in_factor);
        let kept = Self::kept_area();
        for &b in &stereo[..n] {
            if self.kept == KEPT_BYTES { self.kept_head = (self.kept_head + BUFFER_BYTES) % KEPT_BYTES; self.kept -= BUFFER_BYTES; self.lost = true; }
            kept[(self.kept_head + self.kept) % KEPT_BYTES] = b;
            self.kept += 1;
        }
    }

    pub fn record_start(&mut self) -> bool {
        let Some((handle, s)) = self.attached.as_ref().and_then(|a| a.capture) else { return false };
        if self.capturing { return true; }
        match self.host.select(handle, s.alternate) {
            Ok(_) => { (self.capturing, self.kept, self.kept_head, self.lost) = (true, 0, 0, false); true }
            Err(e) => { mind::println!("[AUDIO] USB CAPTURE SETTING {} REFUSED ({:?})", s.alternate, e); false }
        }
    }
    pub fn record_stop(&mut self) {
        if !self.capturing { return; }
        self.capturing = false;
        if let Some((handle, _)) = self.attached.as_ref().and_then(|a| a.capture) { let _ = self.host.select(handle, 0); }
    }
    /// Whole buffers of the kept capture into `out`; (bytes, an overflow since the last read).
    pub fn record_read(&mut self, out: &mut [u8]) -> (usize, bool) {
        self.pump();
        let take = (self.kept / BUFFER_BYTES).min(out.len() / BUFFER_BYTES) * BUFFER_BYTES;
        let kept = Self::kept_area();
        for (i, b) in out[..take].iter_mut().enumerate() { *b = kept[(self.kept_head + i) % KEPT_BYTES]; }
        self.kept_head = (self.kept_head + take) % KEPT_BYTES;
        self.kept -= take;
        (take, core::mem::take(&mut self.lost))
    }
}

// A packet of silence among those built.
const SILENT: usize = usize::MAX;
