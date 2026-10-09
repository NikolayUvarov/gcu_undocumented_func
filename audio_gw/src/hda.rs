// Intel High Definition Audio (551-DRV-0010): the controller (CORB/RIRB for codec verbs, one output and one input
// stream over cyclic buffer lists) and the first codec's paths: an output pin (speaker, headphone or line out) fed by a
// DAC, and an input pin (microphone or line in) feeding an ADC. Streams run cyclically, so played buffers are cleared
// and the play position is read from LPIB, by the interrupt or by the gateway's polling.
use mind::dev::Mmio;
use mind::mem::Mapping;

pub const BUFFERS: usize = 32;
pub const BUFFER_BYTES: usize = 4096;
pub const CAPTURE_BUFFERS: usize = 16;
// DMA layout: playback ring, capture ring, then a page each for the two buffer lists and CORB with RIRB.
const CAPTURE_AT: usize = BUFFERS * BUFFER_BYTES;
const OUT_LIST: usize = CAPTURE_AT + CAPTURE_BUFFERS * BUFFER_BYTES;
const IN_LIST: usize = OUT_LIST + 4096;
const CORB_AT: usize = IN_LIST + 4096;
const RIRB_AT: usize = CORB_AT + 1024;
pub const DMA_BYTES: usize = CORB_AT + 4096;

// Controller registers.
const GCAP: usize = 0x00; const GCTL: usize = 0x08; const STATESTS: usize = 0x0E; const INTCTL: usize = 0x20; const INTSTS: usize = 0x24;
const CORBLBASE: usize = 0x40; const CORBUBASE: usize = 0x44; const CORBWP: usize = 0x48; const CORBRP: usize = 0x4A; const CORBCTL: usize = 0x4C; const CORBSIZE: usize = 0x4E;
const RIRBLBASE: usize = 0x50; const RIRBUBASE: usize = 0x54; const RIRBWP: usize = 0x58; const RINTCNT: usize = 0x5A; const RIRBCTL: usize = 0x5C; const RIRBSTS: usize = 0x5D; const RIRBSIZE: usize = 0x5E;
// Stream descriptor registers, from the descriptor's base.
const SD_CTL: usize = 0x00; const SD_STS: usize = 0x03; const SD_LPIB: usize = 0x04; const SD_CBL: usize = 0x08; const SD_LVI: usize = 0x0C; const SD_FMT: usize = 0x12; const SD_BDPL: usize = 0x18; const SD_BDPU: usize = 0x1C;
const FORMAT: u16 = 0x0011; // 48 kHz, 16 bits, 2 channels
const OUT_TAG: u32 = 1; const IN_TAG: u32 = 2;

// Widget types (audio widget capabilities, bits 23:20).
const OUTPUT: u8 = 0; const INPUT: u8 = 1; const MIXER: u8 = 2; const PIN: u8 = 4;
const WIDGETS: usize = 96; const CONNECTIONS: usize = 16;
const SUMMARY: usize = 160; // the codec's line in the log

#[derive(Clone, Copy, Default)]
struct Widget { nid: u8, kind: u8, caps: u32, pin_caps: u32, config: u32, connections: [u8; CONNECTIONS], count: u8 }

pub struct Hda {
    regs: Mmio, ring: Mapping, physical: u64, codec: u32, afg: u8, rirb_read: usize, corb_entries: usize, rirb_entries: usize,
    out_sd: usize, in_sd: usize,
    // Playback: the next buffer to fill, the buffers queued and not yet played, the last position seen.
    head: usize, queued: usize, played: usize, started: bool,
    // Capture: the next buffer to read, buffers ready, the last position seen; whether it overflowed.
    capture: bool, tail: usize, ready: usize, captured: usize, lost: bool,
    pub interrupts: usize, pub has_input: bool, pub summary: [u8; SUMMARY], pub summary_len: usize,
}

fn put(summary: &mut [u8; SUMMARY], len: &mut usize, args: core::fmt::Arguments) {
    let mut text = mind::util::FixedBuf::<SUMMARY>::new();
    let _ = core::fmt::Write::write_fmt(&mut text, args);
    for &b in text.as_bytes() { if *len < summary.len() { summary[*len] = b; *len += 1; } }
}

impl Hda {
    /// The controller in the MMIO capability `slot` with the DMA region `dma`; None if it does not answer.
    pub fn init(slot: usize, dma: usize) -> Option<Self> {
        let regs = Mmio::map(slot).ok()?;
        let ring = Mapping::new(dma).ok()?;
        let physical = mind::mem::dma_physical(dma).ok()? as u64;
        if ring.len() < DMA_BYTES { mind::println!("[AUDIO] HDA: DMA REGION TOO SMALL"); return None; }
        let gcap = regs.read16(GCAP);
        let (inputs, outputs) = (((gcap >> 8) & 0xF) as usize, ((gcap >> 12) & 0xF) as usize);
        if outputs == 0 { mind::println!("[AUDIO] HDA: NO OUTPUT STREAM"); return None; }
        // Controller reset: CRST low, then high, then time for the codecs to report themselves.
        regs.write32(GCTL, regs.read32(GCTL) & !1);
        for _ in 0..1000 { if regs.read32(GCTL) & 1 == 0 { break; } mind::time::sleep(1); }
        regs.write32(GCTL, regs.read32(GCTL) | 1);
        for _ in 0..1000 { if regs.read32(GCTL) & 1 != 0 { break; } mind::time::sleep(1); }
        mind::time::sleep(10);
        let codecs = regs.read16(STATESTS);
        if codecs == 0 { mind::println!("[AUDIO] HDA: NO CODEC"); return None; }
        mind::println!("[AUDIO] HDA: VERSION {}.{}, {} INPUT AND {} OUTPUT STREAMS, CODECS {:#06b}, 64-BIT {}", regs.read8(0x03), regs.read8(0x02), inputs, outputs, codecs, gcap & 1);
        let mut hda = Self { regs, ring, physical, codec: codecs.trailing_zeros(), afg: 1, rirb_read: 0, corb_entries: 256, rirb_entries: 256,
            out_sd: 0x80 + inputs * 0x20, in_sd: 0x80, head: 0, queued: 0, played: 0, started: false,
            capture: false, tail: 0, ready: 0, captured: 0, lost: false, interrupts: 0, has_input: inputs > 0, summary: [0; SUMMARY], summary_len: 0 };
        hda.ring.as_mut_slice()[CAPTURE_AT..DMA_BYTES].fill(0);
        hda.ring.as_mut_slice()[..CAPTURE_AT].fill(0);
        hda.start_corb();
        let Some(vendor) = hda.parameter(0, 0) else { mind::println!("[AUDIO] HDA: THE CODEC DOES NOT ANSWER (CORBRP {} RIRBWP {})", hda.regs.read16(CORBRP), hda.regs.read16(RIRBWP)); return None };
        let revision = hda.parameter(0, 2).unwrap_or(0);
        let (mut summary, mut len) = ([0u8; SUMMARY], 0);
        put(&mut summary, &mut len, format_args!("CODEC {} {:04X}:{:04X} REVISION {:08X}", hda.codec, vendor >> 16, vendor & 0xFFFF, revision));
        let found = hda.configure(&mut summary, &mut len);
        hda.summary = summary; hda.summary_len = len;
        if !found { mind::println!("[AUDIO] HDA: {}; NO OUTPUT PATH", core::str::from_utf8(&hda.summary[..hda.summary_len]).unwrap_or("?")); return None; }
        hda.program(hda.out_sd, OUT_LIST, BUFFERS, OUT_TAG, 0);
        if hda.has_input { hda.program(hda.in_sd, IN_LIST, CAPTURE_BUFFERS, IN_TAG, CAPTURE_AT); }
        // Interrupts on buffer completion for both streams, when the line is granted (else the gateway polls).
        let stream_bits = (1u32 << (inputs as u32)) | if hda.has_input { 1 } else { 0 };
        hda.regs.write32(INTCTL, (1 << 31) | stream_bits);
        Some(hda)
    }

    pub fn summary(&self) -> &str { core::str::from_utf8(&self.summary[..self.summary_len]).unwrap_or("?") }

    fn dma(&mut self) -> &mut [u8] { self.ring.as_mut_slice() }

    fn start_corb(&mut self) {
        let r = &self.regs;
        r.write8(CORBCTL, 0); r.write8(RIRBCTL, 0);
        for _ in 0..100 { if r.read8(CORBCTL) & 2 == 0 && r.read8(RIRBCTL) & 2 == 0 { break; } mind::time::sleep(1); }
        // The largest ring each supports (size capability in bits 7:4: 256, 16 or 2 entries).
        let pick = |cap: u8| if cap & 0x40 != 0 { (2u8, 256) } else if cap & 0x20 != 0 { (1, 16) } else { (0, 2) };
        let (corb_code, corb_entries) = pick(r.read8(CORBSIZE));
        let (rirb_code, rirb_entries) = pick(r.read8(RIRBSIZE));
        r.write8(CORBSIZE, (r.read8(CORBSIZE) & !3) | corb_code); r.write8(RIRBSIZE, (r.read8(RIRBSIZE) & !3) | rirb_code);
        let (corb, rirb) = (self.physical + CORB_AT as u64, self.physical + RIRB_AT as u64);
        r.write32(CORBLBASE, corb as u32); r.write32(CORBUBASE, (corb >> 32) as u32);
        r.write32(RIRBLBASE, rirb as u32); r.write32(RIRBUBASE, (rirb >> 32) as u32);
        // Read pointer reset: set, seen set, cleared, seen clear (some controllers need both halves).
        r.write16(CORBRP, 0x8000);
        for _ in 0..100 { if r.read16(CORBRP) & 0x8000 != 0 { break; } mind::time::sleep(1); }
        r.write16(CORBRP, 0);
        for _ in 0..100 { if r.read16(CORBRP) & 0x8000 == 0 { break; } mind::time::sleep(1); }
        r.write16(CORBWP, 0);
        r.write16(RIRBWP, 0x8000);
        r.write16(RINTCNT, 1);
        // RINTCTL on: a controller stops taking verbs after RINTCNT responses until RIRBSTS's flag is cleared, and the
        // flag is only set with it (CIE stays off, so no interrupt reaches the line).
        r.write8(CORBCTL, 2); r.write8(RIRBCTL, 3);
        self.corb_entries = corb_entries; self.rirb_entries = rirb_entries; self.rirb_read = 0;
    }

    // One verb to the codec and its response, by polling the RIRB's write pointer.
    fn verb(&mut self, nid: u8, verb: u32, payload: u32) -> Option<u32> {
        let command = self.codec << 28 | (nid as u32) << 20 | if verb >= 0x100 { verb << 8 | (payload & 0xFF) } else { verb << 16 | (payload & 0xFFFF) };
        let write = ((self.regs.read16(CORBWP) as usize & 0xFF) + 1) % self.corb_entries;
        self.dma()[CORB_AT + write * 4..CORB_AT + write * 4 + 4].copy_from_slice(&command.to_le_bytes());
        self.regs.write16(CORBWP, write as u16);
        for spin in 0..300 {
            let wp = self.regs.read16(RIRBWP) as usize & 0xFF;
            if wp != self.rirb_read {
                self.rirb_read = (self.rirb_read + 1) % self.rirb_entries;
                let at = RIRB_AT + self.rirb_read * 8;
                let response = u32::from_le_bytes(self.ring.as_slice()[at..at + 4].try_into().unwrap());
                let extended = u32::from_le_bytes(self.ring.as_slice()[at + 4..at + 8].try_into().unwrap());
                self.regs.write8(RIRBSTS, 0x5);
                if extended & 0x10 != 0 { continue; } // an unsolicited response: not ours
                return Some(response);
            }
            if spin > 100 { mind::time::sleep(1); }
        }
        None
    }
    fn parameter(&mut self, nid: u8, id: u32) -> Option<u32> { self.verb(nid, 0xF00, id) }

    // Every widget of the audio function group, with its connections, pin capabilities and default configuration.
    fn widgets(&mut self, list: &mut [Widget; WIDGETS]) -> usize {
        let Some(groups) = self.parameter(0, 4) else { return 0 };
        let (first, count) = ((groups >> 16) & 0xFF, groups & 0xFF);
        let Some(afg) = (first..first + count).map(|n| n as u8).find(|&n| self.parameter(n, 5).is_some_and(|t| t & 0xFF == 1)) else { return 0 };
        let _ = self.verb(afg, 0x705, 0); // the function group at D0
        self.afg = afg;
        let nodes = self.parameter(afg, 4).unwrap_or(0);
        let (first, count) = ((nodes >> 16) & 0xFF, (nodes & 0xFF).min(WIDGETS as u32));
        let mut n = 0;
        for nid in first..first + count {
            let nid = nid as u8;
            let caps = self.parameter(nid, 9).unwrap_or(0);
            let mut w = Widget { nid, kind: ((caps >> 20) & 0xF) as u8, caps, ..Widget::default() };
            if w.kind == PIN { w.pin_caps = self.parameter(nid, 0xC).unwrap_or(0); w.config = self.verb(nid, 0xF1C, 0).unwrap_or(0); }
            if caps & (1 << 8) != 0 {
                let length = self.parameter(nid, 0xE).unwrap_or(0);
                let (long, total) = (length & 0x80 != 0, (length & 0x7F) as usize);
                let per = if long { 2 } else { 4 };
                let mut index = 0;
                while index < total && (w.count as usize) < CONNECTIONS {
                    let entries = self.verb(nid, 0xF02, index as u32).unwrap_or(0);
                    for k in 0..per.min(total - index) {
                        let entry = if long { (entries >> (16 * k)) & 0xFFFF } else { (entries >> (8 * k)) & 0xFF };
                        let range = entry & if long { 0x8000 } else { 0x80 } != 0;
                        let id = (entry & if long { 0x7FFF } else { 0x7F }) as u8;
                        // A range entry lists every node from the previous one to this one.
                        if range && w.count > 0 { let mut from = w.connections[w.count as usize - 1] + 1; while from <= id && (w.count as usize) < CONNECTIONS { w.connections[w.count as usize] = from; w.count += 1; from += 1; } }
                        else if (w.count as usize) < CONNECTIONS { w.connections[w.count as usize] = id; w.count += 1; }
                    }
                    index += per;
                }
            }
            list[n] = w; n += 1;
        }
        n
    }

    // A chain from `from` through connections to a widget of `kind`, at most 5 deep: the node IDs and, for each node,
    // the index of the connection taken.
    fn find(list: &[Widget], from: u8, kind: u8, depth: usize, path: &mut [(u8, u8); 6]) -> Option<usize> {
        let w = list.iter().find(|w| w.nid == from)?;
        path[depth].0 = from;
        if w.kind == kind && depth > 0 { return Some(depth + 1); }
        if depth >= 5 { return None; }
        for (index, &next) in w.connections[..w.count as usize].iter().enumerate() {
            path[depth].1 = index as u8;
            if let Some(n) = Self::find(list, next, kind, depth + 1, path) { return Some(n); }
        }
        None
    }

    // Unmutes a node's output amplifier at 0 dB, and its input amplifier on connection `index`.
    fn unmute(&mut self, w: &Widget, index: u8) {
        let _ = self.verb(w.nid, 0x705, 0);
        if w.caps & (1 << 2) != 0 {
            let caps = self.parameter(w.nid, 0x12).unwrap_or(0);
            let _ = self.verb(w.nid, 0x3, 0xB000 | (caps & 0x7F));
        }
        if w.caps & (1 << 1) != 0 {
            let caps = self.parameter(w.nid, 0xD).unwrap_or(0);
            let _ = self.verb(w.nid, 0x3, 0x7000 | (index as u32) << 8 | (caps & 0x7F));
        }
        if w.count > 1 && w.kind != MIXER { let _ = self.verb(w.nid, 0x701, index as u32); }
    }

    // Apple's Cirrus codecs (CS4206, CS4207) power the speaker and headphone amplifiers by GPIOs of the function group:
    // GPIO3 the speakers, GPIO1 (MacBook Pro 10,1) or GPIO2 the headphones (facts of Linux's patch_cirrus).
    fn apple_amplifiers(&mut self, headphone: bool) {
        let vendor = self.parameter(0, 0).unwrap_or(0);
        let ssid = self.verb(self.afg, 0xF20, 0).unwrap_or(0); // the subsystem ID: the machine
        if vendor >> 16 != 0x1013 || ssid >> 16 != 0x106B { return; }
        let gpios = self.parameter(self.afg, 0x11).unwrap_or(0) & 0xFF;
        let (speakers, headphones) = (0x08, if ssid == 0x106B_2800 { 0x02 } else { 0x04 });
        let data = if headphone { headphones } else { speakers };
        let _ = self.verb(self.afg, 0x716, speakers | headphones); // mask
        let _ = self.verb(self.afg, 0x717, speakers | headphones); // direction: outputs
        let _ = self.verb(self.afg, 0x715, data);
        let read = self.verb(self.afg, 0xF15, 0).unwrap_or(0xFF);
        mind::println!("[AUDIO] HDA: APPLE {:08X}, CIRRUS AMPLIFIERS: GPIO {:#04X} OF {} SET, READ {:#04X}", ssid, data, gpios, read);
    }

    // Chooses and sets up the output and input paths; false without an output.
    fn configure(&mut self, summary: &mut [u8; SUMMARY], len: &mut usize) -> bool {
        let mut list = [Widget::default(); WIDGETS];
        let n = self.widgets(&mut list);
        let list = &list[..n];
        let device = |w: &Widget| (w.config >> 20) & 0xF;
        let connected = |w: &Widget| (w.config >> 30) != 1;
        let internal = |w: &Widget| (w.config >> 30) == 2;
        // Output: an internal speaker, else headphones, else line out; each fed by a DAC through mixers or selectors.
        let outputs = [(1, true), (1, false), (2, false), (0, true), (0, false)];
        let mut output = None;
        'out: for (kind, inside) in outputs {
            for pin in list.iter().filter(|w| w.kind == PIN && connected(w) && device(w) == kind && internal(w) == inside && w.pin_caps & (1 << 4) != 0) {
                let mut path = [(0u8, 0u8); 6];
                if let Some(depth) = Self::find(list, pin.nid, OUTPUT, 0, &mut path) { output = Some((*pin, path, depth)); break 'out; }
            }
        }
        let Some((pin, path, depth)) = output else { return false };
        for &(nid, index) in &path[..depth] { let w = *list.iter().find(|w| w.nid == nid).unwrap(); self.unmute(&w, index); }
        let dac = path[depth - 1].0;
        let _ = self.verb(dac, 0x706, OUT_TAG << 4);
        let _ = self.verb(dac, 0x2, FORMAT as u32);
        let headphone = device(&pin) == 2;
        let _ = self.verb(pin.nid, 0x707, 0x40 | if headphone { 0x80 } else { 0 });
        if pin.pin_caps & (1 << 16) != 0 { let _ = self.verb(pin.nid, 0x70C, 2); } // EAPD: the external amplifier on
        put(summary, len, format_args!("; OUT PIN {:#X} ({}) <- DAC {:#X}", pin.nid, ["LINE", "SPEAKER", "HEADPHONE"].get(device(&pin) as usize).unwrap_or(&"?"), dac));
        self.apple_amplifiers(headphone);
        // Input: an internal microphone, else a microphone jack, else line in; an ADC reaching it.
        let inputs = [(0xA, true), (0xA, false), (8, false)];
        let mut input = None;
        'inp: for (kind, inside) in inputs {
            for pin in list.iter().filter(|w| w.kind == PIN && connected(w) && device(w) == kind && internal(w) == inside && w.pin_caps & (1 << 5) != 0) {
                for adc in list.iter().filter(|w| w.kind == INPUT) {
                    let mut path = [(0u8, 0u8); 6];
                    if let Some(depth) = Self::find_pin(list, adc.nid, pin.nid, 0, &mut path) { input = Some((*pin, path, depth)); break 'inp; }
                }
            }
        }
        match input {
            Some((pin, path, depth)) if self.has_input => {
                for &(nid, index) in &path[..depth] { let w = *list.iter().find(|w| w.nid == nid).unwrap(); self.unmute(&w, index); }
                let adc = path[0].0;
                let _ = self.verb(adc, 0x706, IN_TAG << 4);
                let _ = self.verb(adc, 0x2, FORMAT as u32);
                // A microphone gets its bias voltage (80 % where the pin offers it).
                let vref = if device(&pin) == 0xA && pin.pin_caps & (1 << 12) != 0 { 4 } else { 0 };
                let _ = self.verb(pin.nid, 0x707, 0x20 | vref);
                put(summary, len, format_args!("; IN PIN {:#X} ({}) -> ADC {:#X}", pin.nid, if device(&pin) == 0xA { "MICROPHONE" } else { "LINE" }, adc));
            }
            _ => { self.has_input = false; put(summary, len, format_args!("; NO INPUT PATH")); }
        }
        true
    }

    // A chain from the ADC `from` to the pin `target`.
    fn find_pin(list: &[Widget], from: u8, target: u8, depth: usize, path: &mut [(u8, u8); 6]) -> Option<usize> {
        let w = list.iter().find(|w| w.nid == from)?;
        path[depth].0 = from;
        if from == target { return Some(depth + 1); }
        if depth >= 5 || (w.kind == PIN && depth > 0) { return None; }
        for (index, &next) in w.connections[..w.count as usize].iter().enumerate() {
            path[depth].1 = index as u8;
            if let Some(n) = Self::find_pin(list, next, target, depth + 1, path) { return Some(n); }
        }
        None
    }

    // A stream descriptor reset and set to a cyclic list of `count` buffers from DMA offset `at`.
    fn program(&mut self, sd: usize, list: usize, count: usize, tag: u32, at: usize) {
        let r = &self.regs;
        r.write8(sd + SD_CTL, 0);
        r.write8(sd + SD_CTL, 1);
        for _ in 0..100 { if r.read8(sd + SD_CTL) & 1 != 0 { break; } mind::time::sleep(1); }
        r.write8(sd + SD_CTL, 0);
        for _ in 0..100 { if r.read8(sd + SD_CTL) & 1 == 0 { break; } mind::time::sleep(1); }
        for i in 0..count {
            let address = self.physical + (at + i * BUFFER_BYTES) as u64;
            let entry = &mut self.ring.as_mut_slice()[list + i * 16..list + i * 16 + 16];
            entry[..8].copy_from_slice(&address.to_le_bytes());
            entry[8..12].copy_from_slice(&(BUFFER_BYTES as u32).to_le_bytes());
            entry[12..16].copy_from_slice(&1u32.to_le_bytes()); // interrupt on completion
        }
        let base = self.physical + list as u64;
        let r = &self.regs;
        r.write32(sd + SD_BDPL, base as u32); r.write32(sd + SD_BDPU, (base >> 32) as u32);
        r.write32(sd + SD_CBL, (count * BUFFER_BYTES) as u32);
        r.write16(sd + SD_LVI, (count - 1) as u16);
        r.write16(sd + SD_FMT, FORMAT);
        r.write8(sd + SD_CTL + 2, (tag << 4) as u8);
    }
    fn run(&self, sd: usize, on: bool) { self.regs.write8(sd + SD_CTL, if on { 0x1E } else { 0 }); } // RUN with IOCE, FEIE, DEIE

    // Playback position: buffers played since the last look are cleared, so a ring the writer left plays silence.
    fn update(&mut self) {
        if self.started {
            let now = self.regs.read32(self.out_sd + SD_LPIB) as usize / BUFFER_BYTES % BUFFERS;
            while self.played != now {
                let at = self.played * BUFFER_BYTES;
                self.dma()[at..at + BUFFER_BYTES].fill(0);
                self.queued = self.queued.saturating_sub(1);
                self.played = (self.played + 1) % BUFFERS;
            }
            if self.queued == 0 { self.head = (now + 1) % BUFFERS; }
        }
        if self.capture {
            let now = self.regs.read32(self.in_sd + SD_LPIB) as usize / BUFFER_BYTES % CAPTURE_BUFFERS;
            let advanced = (now + CAPTURE_BUFFERS - self.captured) % CAPTURE_BUFFERS;
            self.captured = now;
            self.ready += advanced;
            if self.ready >= CAPTURE_BUFFERS - 1 { self.ready = CAPTURE_BUFFERS - 2; self.tail = (now + 2) % CAPTURE_BUFFERS; self.lost = true; }
        }
    }

    pub fn free(&mut self) -> usize { self.update(); BUFFERS - 2 - self.queued }
    pub fn active(&self) -> bool { self.started || self.capture }

    fn submit(&mut self, fill: impl FnOnce(&mut [u8]) -> usize) {
        let at = self.head * BUFFER_BYTES;
        let bytes = fill(&mut self.ring.as_mut_slice()[at..at + BUFFER_BYTES]);
        self.dma()[at + bytes..at + BUFFER_BYTES].fill(0);
        self.head = (self.head + 1) % BUFFERS; self.queued += 1;
        if !self.started { self.played = 0; self.run(self.out_sd, true); self.started = true; }
    }
    pub fn play(&mut self, pcm: &[u8]) -> usize {
        let mut done = 0;
        while done + 4 <= pcm.len() && self.free() > 0 {
            let take = (pcm.len() - done).min(BUFFER_BYTES) & !3;
            self.submit(|buffer| { buffer[..take].copy_from_slice(&pcm[done..done + take]); take });
            done += take;
        }
        done
    }
    pub fn tone(&mut self, hz: usize, ms: usize, sine: fn(u32) -> i16) -> usize {
        let frames = mind::abi::AUDIO_RATE * ms.min(5000) / 1000; let mut phase: u32 = 0; let step = (hz.clamp(20, 20_000) as u32) * 64 * 256 / mind::abi::AUDIO_RATE as u32;
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
    pub fn reset(&mut self) {
        self.run(self.out_sd, false);
        self.dma()[..CAPTURE_AT].fill(0);
        self.program(self.out_sd, OUT_LIST, BUFFERS, OUT_TAG, 0);
        self.head = 0; self.queued = 0; self.played = 0; self.started = false;
    }
    pub fn record_start(&mut self) -> bool {
        if !self.has_input { return false; }
        if self.capture { return true; }
        self.program(self.in_sd, IN_LIST, CAPTURE_BUFFERS, IN_TAG, CAPTURE_AT);
        self.tail = 0; self.ready = 0; self.captured = 0; self.lost = false;
        self.run(self.in_sd, true);
        self.capture = true;
        true
    }
    pub fn record_stop(&mut self) { self.run(self.in_sd, false); self.capture = false; }
    pub fn record_read(&mut self, out: &mut [u8]) -> (usize, bool) {
        if !self.capture { return (0, false); }
        self.update();
        let take = self.ready.min(out.len() / BUFFER_BYTES);
        for i in 0..take {
            let at = CAPTURE_AT + ((self.tail + i) % CAPTURE_BUFFERS) * BUFFER_BYTES;
            out[i * BUFFER_BYTES..(i + 1) * BUFFER_BYTES].copy_from_slice(&self.ring.as_slice()[at..at + BUFFER_BYTES]);
        }
        self.tail = (self.tail + take) % CAPTURE_BUFFERS; self.ready -= take;
        (take * BUFFER_BYTES, core::mem::take(&mut self.lost))
    }
    // Clears the streams' status flags and the controller's.
    pub fn clear_status(&mut self) {
        for sd in [self.out_sd, self.in_sd] { let status = self.regs.read8(sd + SD_STS); if status != 0 { self.regs.write8(sd + SD_STS, status); } }
        let _ = self.regs.read32(INTSTS);
        self.update();
    }
    pub fn interrupt(&mut self) { self.clear_status(); self.interrupts += 1; }
}
