#![no_std]
#![no_main]
// audio_gw: ring 3 audio gateway. Intel High Definition Audio (551-DRV-0010, hda.rs) or, LEGACY, AC97 (QEMU's old card,
// docs/legacy.md): playback DMA ring of 32 buffers, capture ring of 16 (microphone); interrupts arrive as IPC messages
// on the service endpoint, client PCM comes through their shared buffers.
// Extension point for TTS: a speech synthesizer is an ordinary client feeding PCM to `play` (idl/audio.wit).
use mind::abi::*;
use mind::dev::{Irq, Ports};
use mind::idl::wire::{self, Call};
use mind::idl::audio;
use mind::ipc::Endpoint;
use mind::sys::Error;
use mind::mem::{self, Mapping};

mod hda;

const RECEIVED_CAP: usize = 9;
const BUFFERS: usize = 32;
const BUFFER_BYTES: usize = 4096;
// Capture ring: 16 buffers of 4 KiB and a descriptor list, after the playback ring in the DMA region.
const CAPTURE_BUFFERS: usize = 16;
const CAPTURE_BASE: usize = (BUFFERS + 1) * BUFFER_BYTES;
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

struct Ac97 { bus: Regs, ring: Mapping, physical: u32, head: usize, started: bool, interrupts: usize, capture: bool, tail: usize }

impl Ac97 {
    fn init() -> Option<Self> {
        let ring = Mapping::new(SLOT_MEM).ok()?;
        let physical = mem::dma_physical(SLOT_MEM).ok()? as u32;
        let (mixer, bus) = (Regs::open(SLOT_DEV0)?, Regs::open(SLOT_DEV1)?);
        bus.out32(GLOB_CNT, 0x2); // release the codec cold reset
        mind::time::sleep(20);
        mixer.out16(RESET, 0);
        mixer.out16(MASTER, 0x0000);
        mixer.out16(PCM_OUT, 0x0808);
        if mixer.in16(EXT_ID) & 1 != 0 { mixer.out16(EXT_CTRL, mixer.in16(EXT_CTRL) | 1); mixer.out16(FRONT_RATE, AUDIO_RATE as u16); mixer.out16(ADC_RATE, AUDIO_RATE as u16); }
        // Record from the microphone input, unmuted at 0 dB.
        mixer.out16(MIC, 0x0008); mixer.out16(RECORD_SELECT, 0x0000); mixer.out16(RECORD_GAIN, 0x0000);
        if ring.len() < CAPTURE_BASE + (CAPTURE_BUFFERS + 1) * BUFFER_BYTES { mind::println!("[AUDIO] DMA REGION TOO SMALL FOR CAPTURE"); }
        let mut device = Self { bus, ring, physical, head: 0, started: false, interrupts: 0, capture: false, tail: 0 };
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
    // Starts PCM-in DMA over the whole capture ring; the hardware stops at LVI, kept just behind the reader.
    fn record_start(&mut self) -> bool {
        if !self.can_capture() { return false; }
        if self.capture { return true; }
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
        true
    }
    fn record_stop(&mut self) { self.bus.out8(PI_CR, 0); self.bus.out8(PI_CR, CR_RESET); self.capture = false; }
    // Copies completed capture buffers into `out`; returns (bytes, overflow). On overflow the ring restarts.
    fn record_read(&mut self, out: &mut [u8]) -> (usize, bool) {
        if !self.capture { return (0, false); }
        let halted = self.bus.in16(PI_SR) & SR_DCH != 0;
        let civ = self.bus.in8(PI_CIV) as usize % CAPTURE_BUFFERS;
        let ready = if halted { CAPTURE_BUFFERS } else { (civ + CAPTURE_BUFFERS - self.tail) % CAPTURE_BUFFERS };
        let take = ready.min(out.len() / BUFFER_BYTES);
        for i in 0..take {
            let at = CAPTURE_BASE + ((self.tail + i) % CAPTURE_BUFFERS) * BUFFER_BYTES;
            out[i * BUFFER_BYTES..(i + 1) * BUFFER_BYTES].copy_from_slice(&self.ring.as_slice()[at..at + BUFFER_BYTES]);
        }
        self.tail = (self.tail + take) % CAPTURE_BUFFERS;
        if halted {
            // The reader fell a whole ring behind: deliver what was kept and start over.
            self.bus.out16(PI_SR, SR_CLEAR | SR_DCH); self.capture = false; self.record_start();
        } else {
            self.bus.out8(PI_LVI, ((self.tail + CAPTURE_BUFFERS - 1) % CAPTURE_BUFFERS) as u8);
        }
        (take * BUFFER_BYTES, halted)
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
}

// The device behind the gateway: HDA when init granted memory-mapped registers, AC97 when it granted ports.
enum Device { Ac97(Ac97), Hda(hda::Hda) }
impl Device {
    fn open() -> Option<Self> {
        if mind::dev::cap_info(SLOT_DEV0).0 == CAP_KIND_MMIO { return hda::Hda::init(SLOT_DEV0, SLOT_MEM).map(Device::Hda); }
        Ac97::init().map(Device::Ac97)
    }
    fn free(&mut self) -> usize { match self { Device::Ac97(d) => d.free(), Device::Hda(d) => d.free() } }
    fn active(&self) -> bool { matches!(self, Device::Hda(d) if d.active()) }
    fn play(&mut self, pcm: &[u8]) -> usize { match self { Device::Ac97(d) => d.play(pcm), Device::Hda(d) => d.play(pcm) } }
    fn tone(&mut self, hz: usize, ms: usize) -> usize { match self { Device::Ac97(d) => d.tone(hz, ms), Device::Hda(d) => d.tone(hz, ms, sine) } }
    fn reset(&mut self) { match self { Device::Ac97(d) => d.reset(), Device::Hda(d) => d.reset() } }
    fn record_start(&mut self) -> bool { match self { Device::Ac97(d) => d.record_start(), Device::Hda(d) => d.record_start() } }
    fn record_stop(&mut self) { match self { Device::Ac97(d) => d.record_stop(), Device::Hda(d) => d.record_stop() } }
    fn record_read(&mut self, out: &mut [u8]) -> (usize, bool) { match self { Device::Ac97(d) => d.record_read(out), Device::Hda(d) => d.record_read(out) } }
    fn clear_status(&mut self) { match self { Device::Ac97(d) => d.clear_status(), Device::Hda(d) => d.clear_status() } }
    fn interrupt(&mut self) -> usize { match self { Device::Ac97(d) => { d.interrupt(); d.interrupts } Device::Hda(d) => { d.interrupt(); d.interrupts } } }
}

// Clients waiting for ring space: saved reply capability and the number of free buffers needed.
const WAITERS: usize = 8;
// While a client waits, the ring is also looked at this often: an interrupt lost to a burst of completions (a host
// backend such as dsound plays 100 ms at a time) would otherwise leave it waiting forever, and its program with it.
const POLL_MS: u32 = 20;

// Answers deferred `wait` calls whose space is now free (or all of them on stop).
fn release(device: &mut Option<Device>, waiters: &mut [Option<(Call, u8)>; WAITERS], all: bool) {
    let free = device.as_mut().map_or(BUFFERS - 1, |d| d.free());
    for waiter in waiters.iter_mut() {
        if waiter.as_ref().is_some_and(|(_, want)| all || free >= *want as usize) { let (call, _) = waiter.take().unwrap(); let _ = audio::reply_wait(call, Ok(free as u32)); }
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut device = Device::open();
    let irq = Irq(SLOT_IRQ);
    let mut waiters: [Option<(Call, u8)>; WAITERS] = [const { None }; WAITERS];
    let mut overflows = 0u32;
    let mut owner: Option<u64> = None; // the task that owns the microphone capture
    match &device {
        Some(Device::Ac97(_)) => { let _ = irq.bind(Endpoint::SERVICE); mind::println!("[AUDIO] AC97 READY: {} HZ STEREO S16, {} DMA BUFFERS", AUDIO_RATE, BUFFERS); }
        Some(Device::Hda(d)) => {
            let line = irq.bind(Endpoint::SERVICE).is_ok();
            mind::println!("[AUDIO] HDA READY: {}; {} HZ STEREO S16, {} DMA BUFFERS, {}", d.summary(), AUDIO_RATE, BUFFERS, if line { "INTERRUPTS" } else { "POLLED" });
        }
        None => mind::println!("[AUDIO] NO AUDIO DEVICE (HDA OR AC97); GATEWAY ANSWERS WITHOUT OUTPUT"),
    }
    loop {
        // A running HDA stream is looked at while it runs, so the buffers it played are cleared before it loops.
        let waiting = waiters.iter().any(Option::is_some) || device.as_ref().is_some_and(Device::active);
        let request = match Endpoint::SERVICE.recv_timeout(RECEIVED_CAP, if waiting { POLL_MS } else { 0 }) {
            Ok(request) => request,
            Err(Error::Other(ERR_TIMEOUT)) => {
                // No interrupt for a while: do what it would have done, and unmask the line again.
                if let Some(device) = device.as_mut() { device.clear_status(); }
                let _ = irq.ack();
                release(&mut device, &mut waiters, false);
                continue;
            }
            Err(_) => continue,
        };
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
        let sender = request.sender;
        // The capture has one owner (1.1); a task that ended owns nothing.
        if owner.is_some_and(|pid| pid != sender && !mind::process::alive(pid)) { owner = None; }
        let foreign = owner.is_some_and(|pid| pid != sender);
        let (request, call) = match audio::decode(&request, RECEIVED_CAP) { Ok(decoded) => decoded, Err(reason) => { if request.is_call { let _ = wire::reject(reason); } continue; } };
        let lent = |slot: usize| Mapping::new(slot).map_err(|_| Error::Invalid);
        let _ = match (request, device.as_mut()) {
            (audio::Request::Device, d) => audio::reply_device(call, d.is_some().then_some(AUDIO_RATE as u32)),
            (audio::Request::Overflows, _) => audio::reply_overflows(call, overflows),
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
                if started { owner = Some(sender); }
                audio::reply_record_start(call, if started { Ok(()) } else { Err(Error::NotFound) })
            }
            (audio::Request::RecordStop, Some(d)) => { d.record_stop(); owner = None; audio::reply_record_stop(call, Ok(())) }
            (audio::Request::RecordRead { capacity, buffer }, Some(d)) => {
                let result = lent(buffer).map(|mut out| { let len = (capacity as usize).min(out.len()); let (bytes, lost) = d.record_read(&mut out.as_mut_slice()[..len]); overflows += lost as u32; bytes as u32 });
                audio::reply_record_read(call, result)
            }
            (audio::Request::Play { bytes, pcm }, Some(d)) => {
                let result = lent(pcm).map(|data| { let len = (bytes as usize).min(data.len()); d.play(&data.as_slice()[..len]) as u32 });
                audio::reply_play(call, result)
            }
        };
    }
}
