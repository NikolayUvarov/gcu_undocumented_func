#![no_std]
#![no_main]
// audio_gw: ring 3 audio gateway. Intel High Definition Audio (551-DRV-0010, hda.rs) or, LEGACY, AC97 (QEMU's old card,
// docs/legacy.md): playback DMA ring of 32 buffers, capture ring of 16 (microphone), which the gateway empties into its own
// copy every POLL_MS while it records; interrupts arrive as IPC messages on the service endpoint, client PCM comes
// through their shared buffers.
// Extension point for TTS: a speech synthesizer is an ordinary client feeding PCM to `play` (idl/audio.wit).
use mind::abi::*;
use mind::dev::{Irq, Ports};
use mind::idl::wire::{self, Call};
use mind::idl::audio;
use mind::ipc::Endpoint;
use mind::sys::Error;
use mind::mem::{self, Mapping};

mod codec;
mod hda;
mod levels;

use levels::Amp;

const RECEIVED_CAP: usize = 9;
const BUFFERS: usize = 32;
const BUFFER_BYTES: usize = 4096;
// Capture ring: 16 buffers of 4 KiB and a descriptor list, after the playback ring in the DMA region.
const CAPTURE_BUFFERS: usize = 16;
const CAPTURE_BASE: usize = (BUFFERS + 1) * BUFFER_BYTES;
// AC'97's capture as the gateway keeps it for its reader (000-DRV-0066): 256 buffers, 5461 ms, beside the DMA ring's 341.
// 64 still overflowed once in 10 runs on a host three times oversubscribed, where the guest ran behind QEMU's sound.
const KEPT_BUFFERS: usize = 256;
static mut KEPT: [u8; KEPT_BUFFERS * BUFFER_BYTES] = [0; KEPT_BUFFERS * BUFFER_BYTES];
// Mixer registers (NAM) and the bus master PCM OUT channel (NABM).
const RESET: u16 = 0x00; const MASTER: u16 = 0x02; const PCM_OUT: u16 = 0x18; const EXT_ID: u16 = 0x28; const EXT_CTRL: u16 = 0x2A; const FRONT_RATE: u16 = 0x2C;
const MIC: u16 = 0x0E; const RECORD_SELECT: u16 = 0x1A; const RECORD_GAIN: u16 = 0x1C; const ADC_RATE: u16 = 0x32;
// PCM IN channel registers (NABM offsets 0x00..0x0B).
const PI_BDBAR: u16 = 0x00; const PI_CIV: u16 = 0x04; const PI_LVI: u16 = 0x05; const PI_SR: u16 = 0x06; const PI_CR: u16 = 0x0B;
const BDBAR: u16 = 0x10; const CIV: u16 = 0x14; const LVI: u16 = 0x15; const SR: u16 = 0x16; const CR: u16 = 0x1B; const GLOB_CNT: u16 = 0x2C;
const SR_DCH: u16 = 0x01; const SR_CLEAR: u16 = 0x1C; const CR_RUN: u8 = 0x01; const CR_RESET: u8 = 0x02; const CR_IOCE: u8 = 0x10;

// Quarter sine period, amplitude 12000.
const SINE: [i16; 17] = [0, 2341, 4592, 6667, 8485, 9978, 11087, 11769, 12000, 11769, 11087, 9978, 8485, 6667, 4592, 2341, 0];
fn sine(phase: u32) -> i16 { // phase: 0..64 per period
    let (quarter, i) = ((phase / 16) % 4, (phase % 16) as usize);
    match quarter { 0 => SINE[i], 1 => SINE[16 - i], 2 => -SINE[i], _ => -SINE[16 - i] }
}

// BAR registers, addressed by offset from the base of the granted range.
#[derive(Clone, Copy)]
struct Regs { ports: Ports, base: u16 }
impl Regs {
    fn open(slot: usize) -> Option<Self> { let ports = Ports(slot); ports.range().map(|(base, _)| Self { ports, base }) }
    fn in8(&self, at: u16) -> u8 { self.ports.in8(self.base + at) }
    fn in16(&self, at: u16) -> u16 { self.ports.in16(self.base + at) }
    fn out8(&self, at: u16, value: u8) { self.ports.out8(self.base + at, value) }
    fn out16(&self, at: u16, value: u16) { self.ports.out16(self.base + at, value) }
    fn out32(&self, at: u16, value: u32) { self.ports.out32(self.base + at, value) }
}

// `kept_head`, `kept`: the next buffer written in KEPT and how many it holds; `lost`: an overflow not yet reported;
// `looked_ms`, `asked_ms`: when the gateway last emptied the DMA ring and when its reader last asked, for the log.
struct Ac97 { mixer: Regs, bus: Regs, ring: Mapping, physical: u32, head: usize, started: bool, interrupts: usize, capture: bool, tail: usize, master: Amp,
              kept_head: usize, kept: usize, lost: bool, looked_ms: u64, asked_ms: u64 }

// AC'97's record gain: 16 settings of 1.5 dB from 0 dB (register 0x1C).
const AC97_RECORD: Amp = Amp { steps: 16, step_qdb: 6, zero: 0 };

impl Ac97 {
    fn init() -> Option<Self> {
        let ring = Mapping::new(SLOT_MEM).ok()?;
        let physical = mem::dma_physical(SLOT_MEM).ok()? as u32;
        let (mixer, bus) = (Regs::open(SLOT_DEV0)?, Regs::open(SLOT_DEV1)?);
        bus.out32(GLOB_CNT, 0x2); // release the codec cold reset
        mind::time::sleep(20);
        mixer.out16(RESET, 0);
        // The master's attenuation has 5 bits or 6: a codec with 5 reads 0x1F back for 0x20 (AC'97 2.3, 5.7.2).
        mixer.out16(MASTER, 0x2020);
        let master = Amp { steps: if mixer.in16(MASTER) & 0x3F == 0x20 { 64 } else { 32 }, step_qdb: 6, zero: 0 };
        let master = Amp { zero: master.steps - 1, ..master };
        mixer.out16(MASTER, 0x0000);
        mixer.out16(PCM_OUT, 0x0808);
        if mixer.in16(EXT_ID) & 1 != 0 { mixer.out16(EXT_CTRL, mixer.in16(EXT_CTRL) | 1); mixer.out16(FRONT_RATE, AUDIO_RATE as u16); mixer.out16(ADC_RATE, AUDIO_RATE as u16); }
        // Record from the microphone input, unmuted at 0 dB.
        mixer.out16(MIC, 0x0008); mixer.out16(RECORD_SELECT, 0x0000); mixer.out16(RECORD_GAIN, 0x0000);
        if ring.len() < CAPTURE_BASE + (CAPTURE_BUFFERS + 1) * BUFFER_BYTES { mind::println!("[AUDIO] DMA REGION TOO SMALL FOR CAPTURE"); }
        let mut device = Self { mixer, bus, ring, physical, head: 0, started: false, interrupts: 0, capture: false, tail: 0, master,
                                kept_head: 0, kept: 0, lost: false, looked_ms: 0, asked_ms: 0 };
        device.reset();
        Some(device)
    }
    fn reset(&mut self) {
        self.bus.out8(CR, CR_RESET);
        for _ in 0..1000 { if self.bus.in8(CR) & CR_RESET == 0 { break; } }
        // The buffer descriptor list lives in the last page of the DMA region.
        let list = BUFFERS * BUFFER_BYTES;
        for i in 0..BUFFERS {
            let entry = &mut self.ring.as_mut_slice()[list + i * 8..list + i * 8 + 8];
            entry[..4].copy_from_slice(&(self.physical + (i * BUFFER_BYTES) as u32).to_le_bytes());
            entry[4..6].copy_from_slice(&((BUFFER_BYTES / 2) as u16).to_le_bytes());
            entry[6..8].copy_from_slice(&0x8000u16.to_le_bytes()); // interrupt on buffer completion
        }
        self.bus.out32(BDBAR, self.physical + list as u32);
        self.head = 0; self.started = false;
    }
    // Free buffers: everything not queued between CIV and head (one is kept as a gap).
    fn free(&self) -> usize {
        if !self.started { return BUFFERS - 1; }
        if self.bus.in16(SR) & SR_DCH != 0 { return BUFFERS - 1; }
        let civ = self.bus.in8(CIV) as usize;
        BUFFERS - 1 - (self.head + BUFFERS - civ) % BUFFERS
    }
    // Queues a buffer and starts/resumes DMA.
    fn submit(&mut self, fill: impl FnOnce(&mut [u8]) -> usize) {
        let at = self.head * BUFFER_BYTES; let list = BUFFERS * BUFFER_BYTES + self.head * 8;
        let bytes = fill(&mut self.ring.as_mut_slice()[at..at + BUFFER_BYTES]) & !3;
        self.ring.as_mut_slice()[list + 4..list + 6].copy_from_slice(&((bytes / 2) as u16).to_le_bytes());
        self.bus.out8(LVI, self.head as u8);
        self.head = (self.head + 1) % BUFFERS;
        if !self.started { self.bus.out8(CR, CR_RUN | CR_IOCE); self.started = true; }
    }
    fn play(&mut self, pcm: &[u8]) -> usize {
        let mut done = 0;
        while done + 4 <= pcm.len() && self.free() > 0 {
            let take = (pcm.len() - done).min(BUFFER_BYTES) & !3;
            self.submit(|buffer| { buffer[..take].copy_from_slice(&pcm[done..done + take]); take });
            done += take;
        }
        done
    }
    fn tone(&mut self, hz: usize, ms: usize) -> usize {
        let frames = AUDIO_RATE * ms.min(5000) / 1000; let mut phase: u32 = 0; let step = (hz.clamp(20, 20_000) as u32) * 64 * 256 / AUDIO_RATE as u32;
        let mut done = 0;
        while done < frames && self.free() > 0 {
            let take = (frames - done).min(BUFFER_BYTES / 4);
            self.submit(|buffer| {
                for frame in buffer[..take * 4].chunks_exact_mut(4) {
                    let sample = sine(phase / 256).to_le_bytes(); phase = phase.wrapping_add(step);
                    frame[..2].copy_from_slice(&sample); frame[2..].copy_from_slice(&sample);
                }
                take * 4
            });
            done += take;
        }
        done
    }
    fn can_capture(&self) -> bool { self.ring.len() >= CAPTURE_BASE + (CAPTURE_BUFFERS + 1) * BUFFER_BYTES }
    // Starts a capture: the gateway's copy empty, the DMA over the whole ring.
    fn record_start(&mut self) -> bool {
        if !self.can_capture() { return false; }
        if self.capture { return true; }
        let now = mind::time::uptime_ms() as u64;
        (self.kept_head, self.kept, self.lost, self.looked_ms, self.asked_ms) = (0, 0, false, now, now);
        self.start_ring();
        true
    }
    // PCM-in DMA over the whole capture ring; the hardware stops at LVI, kept just behind what the gateway took.
    fn start_ring(&mut self) {
        self.bus.out8(PI_CR, CR_RESET);
        for _ in 0..1000 { if self.bus.in8(PI_CR) & CR_RESET == 0 { break; } }
        let list = CAPTURE_BASE + CAPTURE_BUFFERS * BUFFER_BYTES;
        for i in 0..CAPTURE_BUFFERS {
            let entry = &mut self.ring.as_mut_slice()[list + i * 8..list + i * 8 + 8];
            entry[..4].copy_from_slice(&(self.physical + (CAPTURE_BASE + i * BUFFER_BYTES) as u32).to_le_bytes());
            entry[4..6].copy_from_slice(&((BUFFER_BYTES / 2) as u16).to_le_bytes());
            entry[6..8].copy_from_slice(&0u16.to_le_bytes());
        }
        self.bus.out32(PI_BDBAR, self.physical + list as u32);
        self.tail = 0;
        self.bus.out8(PI_LVI, (CAPTURE_BUFFERS - 1) as u8);
        self.bus.out8(PI_CR, CR_RUN);
        self.capture = true;
    }
    fn record_stop(&mut self) { self.bus.out8(PI_CR, 0); self.bus.out8(PI_CR, CR_RESET); self.capture = false; }
    // Takes the completed capture buffers into the gateway's copy, as many as it has room for. A DMA ring that filled is
    // an overflow: what it held is taken, the log says whose wait it was, and the ring starts over.
    fn drain(&mut self) {
        if !self.capture { return; }
        let now = mind::time::uptime_ms() as u64;
        let halted = self.bus.in16(PI_SR) & SR_DCH != 0;
        let civ = self.bus.in8(PI_CIV) as usize % CAPTURE_BUFFERS;
        let ready = if halted { CAPTURE_BUFFERS } else { (civ + CAPTURE_BUFFERS - self.tail) % CAPTURE_BUFFERS };
        let take = ready.min(KEPT_BUFFERS - self.kept);
        let kept = unsafe { &mut *core::ptr::addr_of_mut!(KEPT) };
        for i in 0..take {
            let (from, to) = (CAPTURE_BASE + ((self.tail + i) % CAPTURE_BUFFERS) * BUFFER_BYTES, ((self.kept_head + i) % KEPT_BUFFERS) * BUFFER_BYTES);
            kept[to..to + BUFFER_BYTES].copy_from_slice(&self.ring.as_slice()[from..from + BUFFER_BYTES]);
        }
        (self.tail, self.kept_head, self.kept) = ((self.tail + take) % CAPTURE_BUFFERS, (self.kept_head + take) % KEPT_BUFFERS, self.kept + take);
        if halted {
            mind::println!("[AUDIO] CAPTURE OVERFLOW: THE DMA RING ({} MS) FILLED; THE GATEWAY LAST TOOK FROM IT {} MS AGO, ITS READER LAST ASKED {} MS AGO, {} OF {} BUFFERS KEPT",
                           CAPTURE_BUFFERS * BUFFER_BYTES * 1000 / (AUDIO_RATE * 4), now - self.looked_ms, now - self.asked_ms, self.kept, KEPT_BUFFERS);
            self.lost = true;
            self.bus.out16(PI_SR, SR_CLEAR | SR_DCH); self.capture = false; self.start_ring();
        } else {
            self.bus.out8(PI_LVI, ((self.tail + CAPTURE_BUFFERS - 1) % CAPTURE_BUFFERS) as u8);
        }
        self.looked_ms = now;
    }
    // The kept capture into `out`, whole buffers, oldest first; returns (bytes, an overflow since the last read).
    fn record_read(&mut self, out: &mut [u8]) -> (usize, bool) {
        if !self.capture && self.kept == 0 { return (0, core::mem::take(&mut self.lost)); }
        self.drain();
        self.asked_ms = mind::time::uptime_ms() as u64;
        let take = self.kept.min(out.len() / BUFFER_BYTES);
        let (first, kept) = ((self.kept_head + KEPT_BUFFERS - self.kept) % KEPT_BUFFERS, unsafe { &*core::ptr::addr_of!(KEPT) });
        for i in 0..take {
            let at = ((first + i) % KEPT_BUFFERS) * BUFFER_BYTES;
            out[i * BUFFER_BYTES..(i + 1) * BUFFER_BYTES].copy_from_slice(&kept[at..at + BUFFER_BYTES]);
        }
        self.kept -= take;
        (take * BUFFER_BYTES, core::mem::take(&mut self.lost))
    }
    // Clears the write-1-to-clear flags until none is left: a buffer that completes between the read and the write
    // would otherwise keep the line asserted, and on an edge-triggered line no interrupt would come again.
    fn clear_status(&mut self) {
        for _ in 0..8 {
            let status = self.bus.in16(SR) & SR_CLEAR;
            if status == 0 { break; }
            self.bus.out16(SR, status);
        }
    }
    fn interrupt(&mut self) { self.clear_status(); self.interrupts += 1; }
    fn set_volume(&self, setting: u8, muted: bool) { self.mixer.out16(MASTER, levels::ac97_master(setting, self.master, muted)); }
    fn set_gain(&self, setting: u8, boost: bool) { self.mixer.out16(RECORD_GAIN, levels::ac97_record(setting)); self.mixer.out16(MIC, levels::ac97_microphone(boost)); }
}

// The levels as set (audio.wit 1.2, 000-DRV-0057): each side's amplifier and setting, and the switch (mute for the
// output, the boost for the capture); `scaled`: no amplifier, the gateway scales the samples.
#[derive(Clone, Copy)]
struct Side { amp: Amp, setting: u8, switch: bool, scaled: bool }
struct Levels { output: Side, capture: Side, boost_qdb: u16 }

impl Levels {
    fn answer(&self) -> audio::Levels {
        let range = |s: &Side| audio::Range { steps: s.amp.steps, step_qdb: s.amp.step_qdb, zero: s.amp.zero, scaled: s.scaled };
        audio::Levels { volume: levels::percent(self.output.setting, self.output.amp), muted: self.output.switch, output: range(&self.output),
                        gain: levels::percent(self.capture.setting, self.capture.amp), boost: self.capture.switch, boost_qdb: self.boost_qdb, capture: range(&self.capture) }
    }
    fn say(&self, why: &str) {
        let side = |s: &Side| { let q = levels::qdb(s.setting, s.amp); (levels::percent(s.setting, s.amp), if q < 0 { "-" } else { "" }, q.abs() / 4, q.abs() % 4 * 25, s.amp.steps, if s.scaled { ", SCALED" } else { "" }) };
        let ((vp, vn, vd, vf, vs, vx), (gp, gn, gd, gf, gs, gx)) = (side(&self.output), side(&self.capture));
        mind::println!("[AUDIO] LEVELS {}: VOLUME {} % ({}{}.{:02} DB OF {} SETTINGS{}){}; GAIN {} % ({}{}.{:02} DB OF {} SETTINGS{}), BOOST {}", why,
                       vp, vn, vd, vf, vs, vx, if self.output.switch { ", MUTED" } else { "" },
                       gp, gn, gd, gf, gs, gx, if self.boost_qdb == 0 { "NONE" } else if self.capture.switch { "ON" } else { "OFF" });
    }
}

// The device behind the gateway: HDA when init granted memory-mapped registers, AC97 when it granted ports.
enum Device { Ac97(Ac97), Hda(hda::Hda) }
// A second HD Audio function's interrupt line, where init puts it (its SLOT_AUDIO_IRQ1, 213-KRN-0127).
const SECOND_IRQ: usize = 7;
impl Device {
    // The device and the slot of its interrupt line. A second HD Audio function (SLOT_DEV1, 213-KRN-0127: the Ryzen's
    // analog one beside its GPU's HDMI audio) is driven when the first has no output path; both share the DMA region.
    fn open() -> Option<(Self, usize)> {
        if mind::dev::cap_info(SLOT_DEV0).0 != CAP_KIND_MMIO { return Ac97::init().map(|d| (Device::Ac97(d), SLOT_IRQ)); }
        if let Some(d) = hda::Hda::init(SLOT_DEV0, SLOT_MEM) { return Some((Device::Hda(d), SLOT_IRQ)); }
        if mind::dev::cap_info(SLOT_DEV1).0 != CAP_KIND_MMIO { return None; }
        mind::println!("[AUDIO] HDA: THE FIRST FUNCTION IS NOT USED; TRYING THE SECOND");
        let d = hda::Hda::init(SLOT_DEV1, SLOT_MEM)?;
        mind::println!("[AUDIO] HDA: DRIVING THE SECOND FUNCTION");
        Some((Device::Hda(d), SECOND_IRQ))
    }
    fn free(&mut self) -> usize { match self { Device::Ac97(d) => d.free(), Device::Hda(d) => d.free() } }
    // A stream the loop looks at every POLL_MS: HDA's while it plays or records, AC'97's capture (000-DRV-0066).
    fn active(&self) -> bool { match self { Device::Hda(d) => d.active(), Device::Ac97(d) => d.capture } }
    fn drain(&mut self) { if let Device::Ac97(d) = self { d.drain(); } }
    fn play(&mut self, pcm: &[u8]) -> usize { match self { Device::Ac97(d) => d.play(pcm), Device::Hda(d) => d.play(pcm) } }
    fn tone(&mut self, hz: usize, ms: usize) -> usize { match self { Device::Ac97(d) => d.tone(hz, ms), Device::Hda(d) => d.tone(hz, ms, sine) } }
    fn reset(&mut self) { match self { Device::Ac97(d) => d.reset(), Device::Hda(d) => d.reset() } }
    fn record_start(&mut self) -> bool { match self { Device::Ac97(d) => d.record_start(), Device::Hda(d) => d.record_start() } }
    fn record_stop(&mut self) { match self { Device::Ac97(d) => d.record_stop(), Device::Hda(d) => d.record_stop() } }
    fn record_read(&mut self, out: &mut [u8]) -> (usize, bool) { match self { Device::Ac97(d) => d.record_read(out), Device::Hda(d) => d.record_read(out) } }
    fn clear_status(&mut self) { match self { Device::Ac97(d) => d.clear_status(), Device::Hda(d) => d.clear_status() } }
    fn interrupt(&mut self) -> usize { match self { Device::Ac97(d) => { d.interrupt(); d.interrupts } Device::Hda(d) => { d.interrupt(); d.interrupts } } }
    // A headphone jack to look at (551-DRV-0056), and the look.
    fn watches(&self) -> bool { matches!(self, Device::Hda(d) if d.watches()) }
    fn watch_jack(&mut self) { if let Device::Hda(d) = self { d.watch_jack(); } }
    // The levels the device starts with: 0 dB on each side, as `init` and `configure` leave the amplifiers.
    fn levels(&self) -> Levels {
        match self {
            Device::Ac97(d) => Levels { output: Side { amp: d.master, setting: d.master.zero, switch: false, scaled: false },
                                        capture: Side { amp: AC97_RECORD, setting: 0, switch: false, scaled: false }, boost_qdb: levels::AC97_BOOST_QDB },
            Device::Hda(d) => {
                let side = |c: Option<hda::Control>, soft: Amp| match c { Some(c) => Side { amp: c.amp, setting: c.amp.zero, switch: false, scaled: false }, None => Side { amp: soft, setting: soft.zero, switch: false, scaled: true } };
                let boost_qdb = d.boost_amp.map_or(0, |b| ((b.amp.steps - 1 - b.amp.zero) as u16) * b.amp.step_qdb as u16);
                Levels { output: side(d.out_amp, levels::SOFT_OUTPUT), capture: side(d.in_amp, levels::SOFT_CAPTURE), boost_qdb }
            }
        }
    }
    // Sets the output's level on the device, or the samples' factor where it has no amplifier.
    fn set_volume(&mut self, s: &Side) {
        match self {
            Device::Ac97(d) => d.set_volume(s.setting, s.switch),
            Device::Hda(d) => match d.out_amp {
                Some(c) => {
                    d.set_amp(c, s.setting, s.switch);
                    // The other speakers' and the headphones' amplifiers follow in dB, muted with it (551-DRV-0056).
                    for f in d.followers.into_iter().flatten() { d.set_amp(f, levels::matching(s.setting, c.amp, f.amp), s.switch); }
                    d.out_factor = if s.switch && (!c.mute || d.followers.iter().flatten().any(|f| !f.mute)) { 0 } else { 1 << 15 };
                }
                None => d.out_factor = levels::factor_q15(levels::qdb(s.setting, s.amp), s.switch),
            },
        }
    }
    fn set_gain(&mut self, s: &Side) {
        match self {
            Device::Ac97(d) => d.set_gain(s.setting, s.switch),
            Device::Hda(d) => {
                match d.in_amp { Some(c) => d.set_amp(c, s.setting, false), None => d.in_factor = levels::factor_q15(levels::qdb(s.setting, s.amp), false) }
                if let Some(b) = d.boost_amp { d.set_amp(b, if s.switch { b.amp.steps - 1 } else { b.amp.zero }, false); }
            }
        }
    }
}

// Clients waiting for ring space: saved reply capability and the number of free buffers needed.
const WAITERS: usize = 8;
// While a client waits, the ring is also looked at this often: an interrupt lost to a burst of completions (a host
// backend such as dsound plays 100 ms at a time) would otherwise leave it waiting forever, and its program with it.
const POLL_MS: u32 = 20;
// A headphone jack is looked at this often (551-DRV-0056).
const JACK_MS: u64 = 1000;

// Answers deferred `wait` calls whose space is now free (or all of them on stop).
fn release(device: &mut Option<Device>, waiters: &mut [Option<(Call, u8)>; WAITERS], all: bool) {
    let free = device.as_mut().map_or(BUFFERS - 1, |d| d.free());
    for waiter in waiters.iter_mut() {
        if waiter.as_ref().is_some_and(|(_, want)| all || free >= *want as usize) { let (call, _) = waiter.take().unwrap(); let _ = audio::reply_wait(call, Ok(free as u32)); }
    }
}

mind::entry!(main);
// A capture as the log tells it (551-DRV-0010): its first second's loudest sample, a stream that gives nothing, and
// what it gave in all, so a microphone that does not work shows in the boot log.
#[derive(Default)]
struct Session { start_ms: u64, bytes: usize, peak: i32, first: bool, idle: bool }
impl Session {
    fn read(&mut self, pcm: &[u8]) {
        for sample in pcm.chunks_exact(2) { self.peak = self.peak.max((i16::from_le_bytes([sample[0], sample[1]]) as i32).abs()); }
        self.bytes += pcm.len();
        let elapsed = (mind::time::uptime_ms() as u64).saturating_sub(self.start_ms);
        if !self.first && self.bytes >= AUDIO_RATE * 4 {
            self.first = true;
            mind::println!("[AUDIO] CAPTURE: THE FIRST SECOND'S PEAK {} OF 32767{}", self.peak, if self.peak < 64 { ", SILENT: THE MICROPHONE GIVES NOTHING" } else { "" });
        }
        if !self.idle && self.bytes == 0 && elapsed > 2000 { self.idle = true; mind::println!("[AUDIO] CAPTURE: NO DATA 2 S AFTER THE START: THE INPUT STREAM DOES NOT RUN"); }
    }
    fn stopped(&self, overflows: u32) {
        mind::println!("[AUDIO] CAPTURE STOPPED: {} MS OF SOUND, PEAK {}, {} OVERFLOWS", self.bytes / (AUDIO_RATE * 4 / 1000), self.peak, overflows);
    }
}

fn main(_info: &'static BootInfo) {
    let (mut device, irq) = match Device::open() { Some((d, line)) => (Some(d), Irq(line)), None => (None, Irq(SLOT_IRQ)) };
    let mut waiters: [Option<(Call, u8)>; WAITERS] = [const { None }; WAITERS];
    let mut overflows = 0u32;
    let mut owner: Option<u64> = None; // the task that owns the microphone capture
    let mut session = Session::default(); // what the current capture gave, for the log
    let mut set = device.as_ref().map(Device::levels); // the levels as set (1.2)
    if let Some(l) = &set { l.say("AT START"); }
    match &device {
        Some(Device::Ac97(_)) => { let _ = irq.bind(Endpoint::SERVICE); mind::println!("[AUDIO] AC97 READY: {} HZ STEREO S16, {} DMA BUFFERS", AUDIO_RATE, BUFFERS); }
        Some(Device::Hda(d)) => {
            let line = irq.bind(Endpoint::SERVICE).is_ok();
            mind::println!("[AUDIO] HDA READY: {}; {} HZ STEREO S16, {} DMA BUFFERS, {}", d.summary(), AUDIO_RATE, BUFFERS, if line { "INTERRUPTS" } else { "POLLED" });
        }
        None => mind::println!("[AUDIO] NO AUDIO DEVICE (HDA OR AC97); GATEWAY ANSWERS WITHOUT OUTPUT"),
    }
    let watching = device.as_ref().is_some_and(Device::watches);
    let mut jack_due = mind::time::uptime_ms() as u64 + JACK_MS;
    loop {
        let now = mind::time::uptime_ms() as u64;
        if watching && now >= jack_due { if let Some(d) = device.as_mut() { d.watch_jack(); } jack_due = now + JACK_MS; }
        // A running HDA stream is looked at while it runs, so the buffers it played are cleared before it loops.
        let waiting = waiters.iter().any(Option::is_some) || device.as_ref().is_some_and(Device::active);
        let timeout = if waiting { POLL_MS } else if watching { jack_due.saturating_sub(now).clamp(1, JACK_MS) as u32 } else { 0 };
        let request = match Endpoint::SERVICE.recv_timeout(RECEIVED_CAP, timeout) {
            Ok(request) => request,
            Err(Error::Other(ERR_TIMEOUT)) => {
                // No interrupt for a while: do what it would have done, and unmask the line again.
                if let Some(device) = device.as_mut() { device.clear_status(); device.drain(); }
                let _ = irq.ack();
                release(&mut device, &mut waiters, false);
                continue;
            }
            Err(_) => continue,
        };
        if let Some(device) = device.as_mut() { device.drain(); }
        if request.irq.is_some() {
            if let Some(device) = device.as_mut() {
                let count = device.interrupt();
                if count % 64 == 1 { mind::println!("[AUDIO] IRQ COUNT {}", count); }
            }
            let _ = irq.ack();
            release(&mut device, &mut waiters, false); // buffers finished playing: wake waiting clients
            continue;
        }
        // idl/audio.wit. PCM and capture travel in the client's lent buffer, mapped only for the call.
        let (sender, badge) = (request.sender, request.badge);
        // The capture has one owner (1.1); a task that ended owns nothing.
        if owner.is_some_and(|pid| pid != sender && !mind::process::alive(pid)) { owner = None; }
        let foreign = owner.is_some_and(|pid| pid != sender);
        let (request, call) = match audio::decode(&request, RECEIVED_CAP) { Ok(decoded) => decoded, Err(reason) => { if request.is_call { let _ = wire::reject(reason); } continue; } };
        let lent = |slot: usize| Mapping::new(slot).map_err(|_| Error::Invalid);
        let _ = match (request, device.as_mut()) {
            (audio::Request::Device, d) => audio::reply_device(call, d.is_some().then_some(AUDIO_RATE as u32)),
            (audio::Request::Overflows, _) => audio::reply_overflows(call, overflows),
            (audio::Request::Levels, _) => audio::reply_levels(call, set.as_ref().map(Levels::answer).as_ref().ok_or(Error::NotFound)),
            // Only the shell's client sets the levels (000-KRN-0118); every other gets `rights`, and the log says who.
            (audio::Request::SetVolume { .. }, _) | (audio::Request::SetGain { .. }, _) if badge != mind::audio::BADGE_LEVELS => {
                mind::println!("[AUDIO] LEVELS: PID {} HAS NO LEVELS BADGE: REFUSED", sender);
                match request { audio::Request::SetVolume { .. } => audio::reply_set_volume(call, Err(Error::Rights)), _ => audio::reply_set_gain(call, Err(Error::Rights)) }
            }
            (audio::Request::SetVolume { percent, .. }, _) | (audio::Request::SetGain { percent, .. }, _) if percent > 100 => {
                match request { audio::Request::SetVolume { .. } => audio::reply_set_volume(call, Err(Error::Invalid)), _ => audio::reply_set_gain(call, Err(Error::Invalid)) }
            }
            (audio::Request::SetVolume { .. }, None) => audio::reply_set_volume(call, Err(Error::NotFound)),
            (audio::Request::SetGain { .. }, None) => audio::reply_set_gain(call, Err(Error::NotFound)),
            (audio::Request::SetVolume { percent, muted }, Some(d)) => {
                let Some(l) = set.as_mut() else { continue };
                l.output.setting = levels::setting(percent, l.output.amp);
                l.output.switch = muted;
                d.set_volume(&l.output);
                l.say("SET");
                audio::reply_set_volume(call, Ok(&l.answer()))
            }
            (audio::Request::SetGain { percent, boost }, Some(d)) => {
                let Some(l) = set.as_mut() else { continue };
                l.capture.setting = levels::setting(percent, l.capture.amp);
                l.capture.switch = boost && l.boost_qdb > 0;
                d.set_gain(&l.capture);
                l.say("SET");
                audio::reply_set_gain(call, Ok(&l.answer()))
            }
            (audio::Request::Wait { buffers }, d) => {
                let want = (buffers as usize).clamp(1, BUFFERS - 1) as u8;
                let free = d.map_or(BUFFERS - 1, |d| d.free());
                if free >= want as usize { audio::reply_wait(call, Ok(free as u32)) } else {
                    // Parked until the playback interrupt frees enough buffers.
                    let mut call = call;
                    match (waiters.iter().position(Option::is_none), call.defer()) {
                        (Some(index), Ok(())) => { waiters[index] = Some((call, want)); Ok(()) }
                        _ => audio::reply_wait(call, Err(Error::NoSlot)),
                    }
                }
            }
            (audio::Request::Tone { .. } , None) => audio::reply_tone(call, Err(Error::NotFound)),
            (audio::Request::Stop, None) => audio::reply_stop(call, Err(Error::NotFound)),
            (audio::Request::RecordStart, None) => audio::reply_record_start(call, Err(Error::NotFound)),
            (audio::Request::RecordStop, None) => audio::reply_record_stop(call, Err(Error::NotFound)),
            (audio::Request::Play { .. }, None) => audio::reply_play(call, Err(Error::NotFound)),
            (audio::Request::RecordRead { .. }, None) => audio::reply_record_read(call, Err(Error::NotFound)),
            (audio::Request::Tone { hz, ms }, Some(d)) => audio::reply_tone(call, Ok(d.tone(hz as usize, ms as usize) as u32)),
            (audio::Request::Stop, Some(d)) => { d.reset(); let replied = audio::reply_stop(call, Ok(())); release(&mut device, &mut waiters, true); replied }
            (audio::Request::RecordStart, Some(_)) if foreign => audio::reply_record_start(call, Err(Error::Other(ERR_BUSY))),
            (audio::Request::RecordStop, Some(_)) if foreign => audio::reply_record_stop(call, Err(Error::Other(ERR_BUSY))),
            (audio::Request::RecordRead { .. }, Some(_)) if foreign => audio::reply_record_read(call, Err(Error::Other(ERR_BUSY))),
            (audio::Request::RecordStart, Some(d)) => {
                overflows = 0;
                if owner.is_none() { d.record_stop(); } // a capture left by a task that ended starts afresh
                let started = d.record_start();
                if started { owner = Some(sender); session = Session { start_ms: mind::time::uptime_ms() as u64, ..Session::default() }; }
                mind::println!("[AUDIO] CAPTURE {} FOR PID {}", if started { "STARTED" } else { "REFUSED: NO INPUT PATH" }, sender);
                audio::reply_record_start(call, if started { Ok(()) } else { Err(Error::NotFound) })
            }
            (audio::Request::RecordStop, Some(d)) => { d.record_stop(); owner = None; session.stopped(overflows); audio::reply_record_stop(call, Ok(())) }
            (audio::Request::RecordRead { capacity, buffer }, Some(d)) => {
                let result = lent(buffer).map(|mut out| {
                    let len = (capacity as usize).min(out.len());
                    let (bytes, lost) = d.record_read(&mut out.as_mut_slice()[..len]);
                    overflows += lost as u32;
                    session.read(&out.as_slice()[..bytes]);
                    bytes as u32
                });
                audio::reply_record_read(call, result)
            }
            (audio::Request::Play { bytes, pcm }, Some(d)) => {
                let result = lent(pcm).map(|data| { let len = (bytes as usize).min(data.len()); d.play(&data.as_slice()[..len]) as u32 });
                audio::reply_play(call, result)
            }
        };
    }
}
