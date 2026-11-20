//! USB Audio Class 1 (000-DRV-0073): what `audio_gw` needs to play to and record from a USB speaker, microphone or
//! speakerphone through `usb_host` — the streaming settings a configuration descriptor offers (16-bit PCM at 48 kHz),
//! the rate an asynchronous device's feedback asks for, and the frames each packet carries. No system calls:
//! tests/uac_host.rs.

/// The interface class and subclasses of an audio function.
pub const CLASS_AUDIO: u8 = 1;
pub const SUBCLASS_CONTROL: u8 = 1;
pub const SUBCLASS_STREAMING: u8 = 2;

const CS_INTERFACE: u8 = 0x24;
const AS_FORMAT_TYPE: u8 = 2;
const FORMAT_TYPE_I: u8 = 1;

/// The rate the gateway runs at, and the bytes of one of its frames (16-bit stereo).
pub const RATE: u32 = 48_000;
pub const FRAME: usize = 4;

/// A streaming setting the gateway can use: 16-bit PCM at 48 kHz on an isochronous endpoint.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Setting {
    /// The streaming interface and its alternate setting.
    pub interface: u8, pub alternate: u8,
    /// Channels (1 or 2) and the bytes of one sample frame on the wire.
    pub channels: u8,
    /// The data endpoint (bit 7: IN), its wMaxPacketSize and bInterval.
    pub endpoint: u8, pub packet: u16, pub interval: u8,
    /// bmAttributes bits 2-3: 1 asynchronous, 2 adaptive, 3 synchronous.
    pub sync: u8,
    /// An asynchronous OUT endpoint's feedback endpoint (bSynchAddress) and its packet, if the setting has it.
    pub feedback: Option<(u8, u16)>,
}

impl Setting {
    pub fn input(&self) -> bool { self.endpoint & 0x80 != 0 }
    /// The bytes of one frame on the wire.
    pub fn frame(&self) -> usize { self.channels as usize * 2 }
}

/// An audio function as its configuration descriptor describes it: the control interface and the settings to play and
/// record with (the first usable of each).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Function { pub control: u8, pub playback: Option<Setting>, pub capture: Option<Setting> }

/// The audio function a configuration descriptor describes, or None without a control interface and a usable setting.
/// A usable setting carries format type I, 16-bit samples in 2 bytes, one or two channels, and 48 kHz among its rates.
pub fn parse(config: &[u8]) -> Option<Function> {
    let mut function = Function::default();
    let mut control = false;
    // The interface the descriptors at hand belong to, and what its setting has said so far.
    let (mut number, mut alternate, mut subclass) = (0u8, 0u8, 0u8);
    let mut format: Option<u8> = None; // the channels of a usable format type I
    let mut data: Option<Setting> = None;
    let mut at = 0;
    while at + 2 <= config.len() {
        let length = config[at] as usize;
        if length < 2 || at + length > config.len() { break; }
        let d = &config[at..at + length];
        match d[1] {
            4 if length >= 9 => {
                finish(&mut data, &mut function);
                (number, alternate) = (d[2], d[3]);
                subclass = if d[5] == CLASS_AUDIO { d[6] } else { 0 };
                format = None;
                if subclass == SUBCLASS_CONTROL && !control { control = true; function.control = number; }
            }
            CS_INTERFACE if subclass == SUBCLASS_STREAMING && length >= 8 && d[2] == AS_FORMAT_TYPE && d[3] == FORMAT_TYPE_I => {
                let (channels, subframe, bits, rates) = (d[4], d[5], d[6], d[7] as usize);
                // bSamFreqType 0: a continuous range (lowest, highest); else that many discrete rates.
                let rate = |i: usize| d.get(8 + 3 * i..11 + 3 * i).map(|r| u32::from_le_bytes([r[0], r[1], r[2], 0]));
                let has = if rates == 0 { matches!((rate(0), rate(1)), (Some(lo), Some(hi)) if lo <= RATE && RATE <= hi) } else { (0..rates).any(|i| rate(i) == Some(RATE)) };
                format = ((1..=2).contains(&channels) && subframe == 2 && bits == 16 && has).then_some(channels);
            }
            5 if length >= 7 && subclass == SUBCLASS_STREAMING && alternate != 0 => {
                let (address, attributes, packet, interval) = (d[2], d[3], u16::from_le_bytes([d[4], d[5]]), d[6]);
                let synch = if length >= 9 { d[8] } else { 0 };
                if attributes & 3 != 1 { at += length; continue; }
                // The feedback endpoint the data endpoint before it names (bSynchAddress); UAC1 marks it no other way.
                if let Some(s) = data.as_mut().filter(|s| s.feedback.is_some_and(|f| f.0 == address)) { s.feedback = Some((address, packet)); }
                else if attributes >> 4 & 3 == 1 { /* a feedback endpoint no data endpoint here names */ }
                else if let Some(channels) = format {
                    if data.is_none() {
                        data = Some(Setting { interface: number, alternate, channels, endpoint: address, packet, interval, sync: attributes >> 2 & 3,
                                              feedback: (synch != 0 && address & 0x80 == 0).then_some((synch, 0)) });
                    }
                }
            }
            _ => {}
        }
        at += length;
    }
    finish(&mut data, &mut function);
    (control && (function.playback.is_some() || function.capture.is_some())).then_some(function)
}

/// The frames a millisecond an asynchronous device's feedback asks for, in 16.16, or None for a value no reading makes
/// near the nominal 48. Full speed sends 10.14 in three bytes, high speed 16.16 a microframe in four; devices that send
/// one where the other belongs are read as Linux reads them, by the value's size.
pub fn feedback(bytes: &[u8], high_speed: bool) -> Option<u32> {
    let value = match bytes.len() { 3 => u32::from_le_bytes([bytes[0], bytes[1], bytes[2], 0]), 4.. => u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]), _ => return None };
    let near = |q16: u32| (44 << 16..=52 << 16).contains(&q16);
    // Candidates in 16.16 frames a millisecond: 10.14 a frame, 16.16 a frame, 16.16 a microframe.
    let candidates = if high_speed { [value.checked_mul(8), Some(value), value.checked_mul(4)] } else { [value.checked_mul(4), Some(value), value.checked_mul(8)] };
    candidates.into_iter().flatten().find(|&q| near(q))
}

// The setting just described goes to its direction's place, if that is still free.
fn finish(data: &mut Option<Setting>, function: &mut Function) {
    if let Some(s) = data.take() {
        let slot = if s.input() { &mut function.capture } else { &mut function.playback };
        if slot.is_none() { *slot = Some(s); }
    }
}

/// Frames a packet from a rate in 16.16 frames a millisecond: whole frames, the fraction carried to later packets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pacer { rate: u32, carry: u32, most: usize }

impl Pacer {
    /// At 48 frames a millisecond, and at most `most` frames a packet (the endpoint's packet over the frame's bytes).
    pub fn new(most: usize) -> Self { Self { rate: 48 << 16, carry: 0, most: most.max(1) } }
    /// The device asked for another rate (`feedback`'s value); kept within one frame of the nominal 48.
    pub fn set(&mut self, rate: u32) { self.rate = rate.clamp(47 << 16, 49 << 16); }
    pub fn rate(&self) -> u32 { self.rate }
    /// The next packet's frames.
    pub fn next(&mut self) -> usize {
        let total = self.carry + self.rate;
        let frames = ((total >> 16) as usize).min(self.most);
        self.carry = total - ((frames as u32) << 16);
        if self.carry >= 2 << 16 { self.carry = 1 << 16; } // a cap that held frames back: no growing debt
        frames
    }
}

/// Samples on the wire from the gateway's 16-bit stereo frames: as they are for two channels, their mean for one.
pub fn to_wire(stereo: &[u8], channels: u8, out: &mut [u8]) -> usize {
    if channels == 2 { let n = stereo.len().min(out.len()); out[..n].copy_from_slice(&stereo[..n]); return n; }
    let mut n = 0;
    for (frame, sample) in stereo.chunks_exact(FRAME).zip(out.chunks_exact_mut(2)) {
        let (l, r) = (i16::from_le_bytes([frame[0], frame[1]]) as i32, i16::from_le_bytes([frame[2], frame[3]]) as i32);
        sample.copy_from_slice(&(((l + r) / 2) as i16).to_le_bytes());
        n += 2;
    }
    n
}

/// The gateway's 16-bit stereo frames from samples on the wire: as they are for two channels, each twice for one.
pub fn from_wire(wire: &[u8], channels: u8, out: &mut [u8]) -> usize {
    if channels == 2 { let n = wire.len().min(out.len()) & !3; out[..n].copy_from_slice(&wire[..n]); return n; }
    let mut n = 0;
    for (sample, frame) in wire.chunks_exact(2).zip(out.chunks_exact_mut(FRAME)) {
        frame[..2].copy_from_slice(sample); frame[2..].copy_from_slice(sample);
        n += FRAME;
    }
    n
}
