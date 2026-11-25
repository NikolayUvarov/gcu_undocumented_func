//! A2DP source (A2DP 1.3.2) over AVDTP (1.3): one audio source stream endpoint with SBC. As initiator it discovers
//! the sink's endpoints, reads their capabilities, configures SBC, opens the media channel and starts; as acceptor it
//! answers a sink that does those itself. Media packets carry an RTP header and SBC frames (A2DP 4.3.4), paced by the
//! clock, from a tone made here until a program feeds it (the audio gateway's sink, later).
use crate::buf::be16;
use crate::ctl::Ctl;
use crate::l2cap::{self, Channels};
use crate::sbc::{self, Config, Encoder, Mode, Tone};

pub const DISCOVER: u8 = 0x01;
pub const GET_CAPABILITIES: u8 = 0x02;
pub const SET_CONFIGURATION: u8 = 0x03;
pub const GET_CONFIGURATION: u8 = 0x04;
pub const RECONFIGURE: u8 = 0x05;
pub const OPEN: u8 = 0x06;
pub const START: u8 = 0x07;
pub const CLOSE: u8 = 0x08;
pub const SUSPEND: u8 = 0x09;
pub const ABORT: u8 = 0x0A;
pub const SECURITY_CONTROL: u8 = 0x0B;
pub const GET_ALL_CAPABILITIES: u8 = 0x0C;
pub const DELAY_REPORT: u8 = 0x0D;

pub const COMMAND: u8 = 0;
pub const GENERAL_REJECT: u8 = 1;
pub const ACCEPT: u8 = 2;
pub const REJECT: u8 = 3;

/// Service categories: media transport, media codec.
pub const MEDIA_TRANSPORT: u8 = 0x01;
pub const MEDIA_CODEC: u8 = 0x07;
/// Error codes: bad ACP SEID, SEP in use, bad state, unsupported configuration.
pub const BAD_ACP_SEID: u8 = 0x12;
pub const SEP_IN_USE: u8 = 0x13;
pub const BAD_STATE: u8 = 0x31;
pub const UNSUPPORTED_CONFIGURATION: u8 = 0x29;
pub const NOT_SUPPORTED_COMMAND: u8 = 0x19;

/// Our one stream endpoint: an audio source.
pub const SEID: u8 = 1;
/// SBC as we take it (A2DP 4.3.2): 44.1 and 48 kHz, every channel mode, every block length and subband count, both
/// allocations, bitpools 2 to 53.
pub const CAPABILITIES: [u8; 4] = [0x30 | 0x0F, 0xFF, 2, 53];
/// The bitpool used at most: A2DP's high quality for joint stereo at 44.1 and 48 kHz.
pub const BITPOOL: u8 = 53;
/// How long a command may wait for its answer, how long a sink that opened the signalling channel is left to
/// configure us, and how far ahead of the clock media goes.
pub const ANSWER_MS: u64 = 5_000;
pub const IDLE_MS: u64 = 2_000;
pub const LEAD_MS: u64 = 40;
/// The longest media payload we make.
pub const PACKET: usize = 1024;
/// Packets queued for the controller beyond which a media packet waits.
pub const BACKLOG: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State { Idle, Discovering, Capabilities, Configuring, Configured, Opening, Open, Starting, Streaming, Closing }

/// The sink's codec capability from its Get Capabilities answer, or None if it has no SBC.
pub fn sbc_capability(caps: &[u8]) -> Option<[u8; 4]> {
    let mut at = 0;
    while at + 2 <= caps.len() {
        let (category, len) = (caps[at], caps[at + 1] as usize);
        let info = caps.get(at + 2..at + 2 + len)?;
        if category == MEDIA_CODEC && len >= 6 && info[0] >> 4 == 0 && info[1] == 0 { return Some([info[2], info[3], info[4], info[5]]); }
        at += 2 + len;
    }
    None
}

/// The configuration we choose within a sink's SBC capability: 48 kHz before 44.1, joint stereo before stereo, dual
/// and mono, 16 blocks, 8 subbands, loudness, the highest bitpool up to ours. None when nothing is common.
pub fn choose(cap: [u8; 4]) -> Option<Config> {
    let pick = |bits: u8, order: &[(u8, u8)]| order.iter().find(|(b, _)| bits & b != 0).map(|&(_, v)| v);
    let rate = pick(cap[0] >> 4, &[(0x1, 3), (0x2, 2)])?;
    let mode = pick(cap[0] & 0x0F, &[(0x1, 3), (0x2, 2), (0x4, 1), (0x8, 0)])?;
    let blocks = pick(cap[1] >> 4, &[(0x1, 16), (0x2, 12), (0x4, 8), (0x8, 4)])?;
    let subbands = pick(cap[1] >> 2 & 3, &[(0x1, 8), (0x2, 4)])?;
    let snr = pick(cap[1] & 3, &[(0x1, 0), (0x2, 1)])? == 1;
    let mode = [Mode::Mono, Mode::Dual, Mode::Stereo, Mode::Joint][mode as usize];
    let mut cfg = Config { rate, blocks, mode, snr, subbands, bitpool: 0 };
    let (min, max) = (cap[2].max(2), cap[3].min(cfg.max_bitpool()));
    if min > max { return None; }
    cfg.bitpool = BITPOOL.clamp(min, max);
    Some(cfg)
}

/// A configuration's codec information (one bit each, A2DP 4.3.2), with the bitpool range used.
pub fn codec_info(cfg: &Config, min: u8) -> [u8; 4] {
    let rate = [0x80u8, 0x40, 0x20, 0x10][cfg.rate as usize & 3];
    let mode = [0x08u8, 0x04, 0x02, 0x01][cfg.mode as usize];
    let blocks = match cfg.blocks { 4 => 0x80u8, 8 => 0x40, 12 => 0x20, _ => 0x10 };
    let subbands = if cfg.subbands == 4 { 0x08u8 } else { 0x04 };
    [rate | mode, blocks | subbands | if cfg.snr { 0x02 } else { 0x01 }, min.min(cfg.bitpool), cfg.bitpool]
}

/// The configuration a sink sets (its codec information, one bit each), when we can encode it.
pub fn configured(info: [u8; 4]) -> Option<Config> {
    let one = |bits: u8| bits.count_ones() == 1;
    let (r, m, b, s, a) = (info[0] >> 4, info[0] & 0x0F, info[1] >> 4, info[1] >> 2 & 3, info[1] & 3);
    if !(one(r) && one(m) && one(b) && one(s) && one(a)) { return None; }
    let rate = match r { 0x8 => 0, 0x4 => 1, 0x2 => 2, _ => 3 };
    if rate < 2 { return None; }
    let mode = match m { 0x8 => Mode::Mono, 0x4 => Mode::Dual, 0x2 => Mode::Stereo, _ => Mode::Joint };
    let blocks = match b { 0x8 => 4, 0x4 => 8, 0x2 => 12, _ => 16 };
    let subbands = if s == 2 { 4 } else { 8 };
    let mut cfg = Config { rate, blocks, mode, snr: a == 2, subbands, bitpool: 0 };
    let (min, max) = (info[2].max(2), info[3].min(cfg.max_bitpool()).min(BITPOOL.max(info[2])));
    if min > max { return None; }
    cfg.bitpool = max;
    Some(cfg)
}

pub struct Source {
    pub handle: Option<u16>,
    pub sig: Option<u16>,
    pub media: Option<u16>,
    pub state: State,
    /// Whether we configured the stream (else the sink did); the sink's endpoint; the endpoints left to try.
    pub ours: bool,
    pub remote: u8,
    candidates: [u8; 8], tried: usize, found: usize,
    label: u8,
    /// Our command waiting for its answer: its label, signal and when it went.
    pending: Option<(u8, u8, u64)>,
    since: u64,
    pub cfg: Option<Config>,
    min_bitpool: u8,
    enc: Encoder,
    pub tone: Tone,
    seq: u16, timestamp: u32, started: u64, pub sent: u64, played: u64, pub packets: u32, pub late: u32,
    rx: [u8; 512], rx_len: usize,
}

impl Default for Source { fn default() -> Self { Self::new() } }

fn header(label: u8, message: u8) -> u8 { label << 4 | message }

impl Source {
    pub fn new() -> Self {
        let cfg = Config { rate: 3, blocks: 16, mode: Mode::Joint, snr: false, subbands: 8, bitpool: BITPOOL };
        Self { handle: None, sig: None, media: None, state: State::Idle, ours: false, remote: 0, candidates: [0; 8], tried: 0, found: 0, label: 0, pending: None,
               since: 0, cfg: None, min_bitpool: 2, enc: Encoder::new(cfg), tone: Tone::new(440, 660, 48_000, 8192), seq: 0, timestamp: 0, started: 0, sent: 0, played: 0, packets: 0,
               late: 0, rx: [0; 512], rx_len: 0 }
    }

    fn reset(&mut self) {
        let tone = core::mem::replace(&mut self.tone, Tone::new(440, 660, 48_000, 8192));
        *self = Self::new();
        self.tone = tone;
    }

    /// A secure link to an audio device: our signalling channel, unless the sink opened one.
    pub fn start(&mut self, ctl: &mut Ctl, chans: &mut Channels, handle: u16) {
        if self.handle.is_some_and(|h| h != handle) { ctl.say(format_args!("A2DP: A STREAM RUNS ON {:03X} ALREADY", self.handle.unwrap())); return; }
        if self.sig.is_some() || chans.find(handle, l2cap::PSM_AVDTP).is_some() { return; }
        self.handle = Some(handle);
        if chans.connect(ctl, handle, l2cap::PSM_AVDTP).is_none() { ctl.say(format_args!("A2DP: NO CHANNEL FREE")); }
    }

    fn send(&mut self, ctl: &mut Ctl, chans: &Channels, parts: &[&[u8]]) -> bool {
        let (Some(h), Some(sig)) = (self.handle, self.sig) else { return false };
        chans.send(ctl, h, sig, parts)
    }

    fn command(&mut self, ctl: &mut Ctl, chans: &Channels, signal: u8, params: &[u8]) {
        self.label = (self.label + 1) & 0x0F;
        let head = [header(self.label, COMMAND), signal];
        if self.send(ctl, chans, &[&head, params]) { self.pending = Some((self.label, signal, ctl.now)); }
    }

    fn answer(&mut self, ctl: &mut Ctl, chans: &Channels, label: u8, signal: u8, message: u8, params: &[u8]) {
        self.send(ctl, chans, &[&[header(label, message), signal], params]);
    }

    /// An AVDTP channel opened: the first is signalling, the next the media transport.
    pub fn opened(&mut self, ctl: &mut Ctl, chans: &mut Channels, handle: u16, local: u16, initiator: bool) {
        if self.handle.is_some_and(|h| h != handle) { chans.disconnect(ctl, handle, local); return; }
        self.handle = Some(handle);
        if self.sig.is_none() {
            self.sig = Some(local);
            self.since = ctl.now;
            if initiator { self.discover(ctl, chans); }
            return;
        }
        if self.media.is_none() && matches!(self.state, State::Opening | State::Open | State::Configured) {
            self.media = Some(local);
            let mtu = chans.get(local).map_or(0, |c| c.mtu);
            ctl.say(format_args!("A2DP: MEDIA CHANNEL OPEN, MTU {}", mtu));
            self.state = State::Open;
            if self.ours || self.pending.is_none() { self.command(ctl, chans, START, &[self.remote << 2]); self.state = State::Starting; }
            return;
        }
        chans.disconnect(ctl, handle, local);
    }

    fn discover(&mut self, ctl: &mut Ctl, chans: &Channels) {
        self.ours = true;
        self.state = State::Discovering;
        self.command(ctl, chans, DISCOVER, &[]);
    }

    pub fn refused(&mut self, handle: u16) { if self.handle == Some(handle) && self.sig.is_none() { self.reset(); } }

    pub fn closed(&mut self, ctl: &mut Ctl, chans: &mut Channels, handle: u16, local: u16) {
        if self.handle != Some(handle) { return; }
        if self.media == Some(local) {
            self.media = None;
            ctl.say(format_args!("A2DP: MEDIA CHANNEL CLOSED AFTER {} PACKETS ({} LATE)", self.packets, self.late));
            self.state = if self.state == State::Closing { State::Idle } else { State::Configured };
            if self.state == State::Idle { self.cfg = None; }
        } else if self.sig == Some(local) {
            if let Some(m) = self.media { chans.disconnect(ctl, handle, m); }
            ctl.say(format_args!("A2DP: SIGNALLING CLOSED"));
            self.reset();
        }
    }

    pub fn link_down(&mut self, handle: u16) { if self.handle == Some(handle) { self.reset(); } }

    /// A PDU on one of our AVDTP channels.
    pub fn data(&mut self, ctl: &mut Ctl, chans: &mut Channels, handle: u16, local: u16, payload: &[u8]) {
        if self.handle != Some(handle) || self.sig != Some(local) || payload.is_empty() { return; }
        let (label, kind, message) = (payload[0] >> 4, payload[0] >> 2 & 3, payload[0] & 3);
        // Packet types: single, start (with the number of packets), continue, end.
        let mut whole = [0u8; 512];
        let n = match kind {
            0 => { let n = payload.len().min(512); whole[..n].copy_from_slice(&payload[..n]); n }
            1 if payload.len() >= 3 => { self.rx[0] = payload[0] & 0xF3; let n = (payload.len() - 2).min(511); self.rx[1..1 + n].copy_from_slice(&payload[2..2 + n]); self.rx_len = 1 + n; return; }
            2 | 3 if self.rx_len > 0 => {
                let n = (payload.len() - 1).min(512 - self.rx_len);
                self.rx[self.rx_len..self.rx_len + n].copy_from_slice(&payload[1..1 + n]);
                self.rx_len += n;
                if kind == 2 { return; }
                let n = self.rx_len;
                whole[..n].copy_from_slice(&self.rx[..n]);
                self.rx_len = 0;
                n
            }
            _ => return,
        };
        let msg = &whole[..n];
        if msg.len() < 2 { return; }
        let (signal, params) = (msg[1] & 0x3F, &msg[2..]);
        if message == COMMAND { self.on_command(ctl, chans, label, signal, params); } else { self.on_answer(ctl, chans, label, signal, message, params); }
    }

    fn on_command(&mut self, ctl: &mut Ctl, chans: &mut Channels, label: u8, signal: u8, p: &[u8]) {
        let seid = p.first().map_or(0, |b| b >> 2);
        let ours = seid == SEID;
        match signal {
            DISCOVER => {
                let in_use = self.cfg.is_some() && !self.ours;
                // Audio (0), source (TSEP 0).
                self.answer(ctl, chans, label, signal, ACCEPT, &[SEID << 2 | (in_use as u8) << 1, 0x00]);
            }
            GET_CAPABILITIES | GET_ALL_CAPABILITIES if ours => {
                let c = CAPABILITIES;
                self.answer(ctl, chans, label, signal, ACCEPT, &[MEDIA_TRANSPORT, 0, MEDIA_CODEC, 6, 0x00, 0x00, c[0], c[1], c[2], c[3]]);
            }
            GET_CONFIGURATION if ours => match self.cfg {
                Some(cfg) => { let i = codec_info(&cfg, self.min_bitpool); self.answer(ctl, chans, label, signal, ACCEPT, &[MEDIA_TRANSPORT, 0, MEDIA_CODEC, 6, 0, 0, i[0], i[1], i[2], i[3]]); }
                None => self.answer(ctl, chans, label, signal, REJECT, &[BAD_STATE]),
            },
            SET_CONFIGURATION if ours && p.len() >= 2 => {
                if self.cfg.is_some() && self.state != State::Configuring { self.answer(ctl, chans, label, signal, REJECT, &[MEDIA_CODEC, SEP_IN_USE]); return; }
                let info = sbc_capability(&p[2..]);
                match info.and_then(configured) {
                    Some(cfg) => {
                        // The sink configures: ours, if we were at it, is given up.
                        self.set(cfg, info.unwrap()[2], p[1] >> 2, false);
                        self.pending = None;
                        ctl.say(format_args!("A2DP: THE SINK CONFIGURED SBC: {} HZ, {:?}, {} BLOCKS, {} SUBBANDS, {}, BITPOOL {} ({} KBIT/S)",
                            cfg.hz(), cfg.mode, cfg.blocks, cfg.subbands, if cfg.snr { "SNR" } else { "LOUDNESS" }, cfg.bitpool, cfg.bitrate() / 1000));
                        self.answer(ctl, chans, label, signal, ACCEPT, &[]);
                    }
                    None => self.answer(ctl, chans, label, signal, REJECT, &[MEDIA_CODEC, UNSUPPORTED_CONFIGURATION]),
                }
            }
            OPEN if ours => {
                if self.state != State::Configured { self.answer(ctl, chans, label, signal, REJECT, &[BAD_STATE]); return; }
                self.state = State::Opening;
                self.answer(ctl, chans, label, signal, ACCEPT, &[]);
            }
            START if ours => {
                if self.media.is_none() { self.answer(ctl, chans, label, signal, REJECT, &[SEID << 2, BAD_STATE]); return; }
                self.answer(ctl, chans, label, signal, ACCEPT, &[]);
                self.streaming(ctl);
            }
            SUSPEND if ours => {
                self.answer(ctl, chans, label, signal, ACCEPT, &[]);
                if self.state == State::Streaming { ctl.say(format_args!("A2DP: THE SINK SUSPENDED THE STREAM AFTER {} PACKETS", self.packets)); }
                self.state = State::Open;
            }
            CLOSE if ours => { self.answer(ctl, chans, label, signal, ACCEPT, &[]); self.state = State::Closing; }
            ABORT if ours => {
                self.answer(ctl, chans, label, signal, ACCEPT, &[]);
                if let (Some(h), Some(m)) = (self.handle, self.media) { chans.disconnect(ctl, h, m); }
                self.cfg = None;
                self.state = State::Idle;
            }
            DELAY_REPORT if ours => {
                ctl.say(format_args!("A2DP: THE SINK'S DELAY {} × 0.1 MS", be16(p, 1)));
                self.answer(ctl, chans, label, signal, ACCEPT, &[]);
            }
            RECONFIGURE | SECURITY_CONTROL if ours => self.answer(ctl, chans, label, signal, REJECT, &[NOT_SUPPORTED_COMMAND]),
            GET_CAPABILITIES | GET_ALL_CAPABILITIES | GET_CONFIGURATION | OPEN | CLOSE | ABORT | DELAY_REPORT => self.answer(ctl, chans, label, signal, REJECT, &[BAD_ACP_SEID]),
            SET_CONFIGURATION | START | SUSPEND | RECONFIGURE | SECURITY_CONTROL => self.answer(ctl, chans, label, signal, REJECT, &[p.first().copied().unwrap_or(0), BAD_ACP_SEID]),
            _ => self.answer(ctl, chans, label, signal, GENERAL_REJECT, &[]),
        }
    }

    fn set(&mut self, cfg: Config, min: u8, remote: u8, ours: bool) {
        (self.cfg, self.min_bitpool, self.remote, self.ours, self.state) = (Some(cfg), min.max(2), remote, ours, State::Configured);
        self.enc = Encoder::new(cfg);
        let level = self.tone.level;
        self.tone = Tone::new(440, 660, cfg.hz(), level);
    }

    fn on_answer(&mut self, ctl: &mut Ctl, chans: &mut Channels, label: u8, signal: u8, message: u8, p: &[u8]) {
        let Some((l, s, _)) = self.pending else { return };
        if l != label || s != signal { return; }
        self.pending = None;
        if message != ACCEPT {
            ctl.say(format_args!("A2DP: SIGNAL {:02X} REJECTED ({:02X?})", signal, &p[..p.len().min(4)]));
            match signal {
                GET_CAPABILITIES | SET_CONFIGURATION if self.state != State::Configured => self.next_candidate(ctl, chans),
                START => self.state = State::Open,
                _ => {}
            }
            return;
        }
        match signal {
            DISCOVER => {
                self.found = 0;
                for sep in p.chunks_exact(2) {
                    let (seid, in_use, media, sink) = (sep[0] >> 2, sep[0] >> 1 & 1, sep[1] >> 4, sep[1] >> 3 & 1);
                    ctl.say(format_args!("A2DP: ENDPOINT {} {} {}{}", seid, if media == 0 { "AUDIO" } else { "OTHER" }, if sink == 1 { "SINK" } else { "SOURCE" }, if in_use == 1 { " IN USE" } else { "" }));
                    if media == 0 && sink == 1 && in_use == 0 && self.found < 8 { self.candidates[self.found] = seid; self.found += 1; }
                }
                self.tried = 0;
                self.next_candidate(ctl, chans);
            }
            GET_CAPABILITIES => {
                let seid = self.candidates[self.tried - 1];
                match sbc_capability(p).and_then(|cap| choose(cap).map(|c| (cap, c))) {
                    Some((cap, cfg)) => {
                        let info = codec_info(&cfg, cap[2]);
                        self.remote = seid;
                        self.state = State::Configuring;
                        self.cfg = Some(cfg);
                        ctl.say(format_args!("A2DP: SBC {:02X?} AT ENDPOINT {}: {} HZ, {:?}, {} BLOCKS, {} SUBBANDS, BITPOOL {} ({} KBIT/S)",
                            cap, seid, cfg.hz(), cfg.mode, cfg.blocks, cfg.subbands, cfg.bitpool, cfg.bitrate() / 1000));
                        self.command(ctl, chans, SET_CONFIGURATION, &[seid << 2, SEID << 2, MEDIA_TRANSPORT, 0, MEDIA_CODEC, 6, 0, 0, info[0], info[1], info[2], info[3]]);
                    }
                    None => self.next_candidate(ctl, chans),
                }
            }
            SET_CONFIGURATION => {
                let cfg = self.cfg.unwrap_or(self.enc.cfg);
                self.set(cfg, codec_info(&cfg, 2)[2], self.remote, true);
                self.state = State::Opening;
                self.command(ctl, chans, OPEN, &[self.remote << 2]);
            }
            OPEN => {
                // The initiator opens the media channel.
                if let Some(h) = self.handle { if chans.connect(ctl, h, l2cap::PSM_AVDTP).is_none() { ctl.say(format_args!("A2DP: NO CHANNEL FREE FOR MEDIA")); } }
            }
            START => self.streaming(ctl),
            SUSPEND => self.state = State::Open,
            CLOSE => {
                self.state = State::Closing;
                if let (Some(h), Some(m)) = (self.handle, self.media) { chans.disconnect(ctl, h, m); }
            }
            _ => {}
        }
    }

    fn next_candidate(&mut self, ctl: &mut Ctl, chans: &Channels) {
        if self.tried >= self.found {
            ctl.say(format_args!("A2DP: NO AUDIO SINK WITH SBC WE CAN USE"));
            self.state = State::Idle;
            self.cfg = None;
            return;
        }
        let seid = self.candidates[self.tried];
        self.tried += 1;
        self.state = State::Capabilities;
        self.command(ctl, chans, GET_CAPABILITIES, &[seid << 2]);
    }

    fn streaming(&mut self, ctl: &mut Ctl) {
        self.state = State::Streaming;
        (self.started, self.sent) = (ctl.now, 0);
        ctl.say(format_args!("A2DP: STREAMING: A TONE OF 440 HZ LEFT AND 660 HZ RIGHT, 0.4 S EVERY SECOND"));
    }

    /// Frames in one media packet: as many as the channel's MTU takes after the RTP header and the payload's byte, at
    /// most 15.
    pub fn frames_per_packet(cfg: &Config, mtu: u16) -> usize { ((mtu as usize).saturating_sub(13) / cfg.frame_len()).clamp(1, 15) }

    /// Media as the clock asks; commands that went unanswered.
    pub fn tick(&mut self, ctl: &mut Ctl, chans: &mut Channels) {
        if let Some((_, signal, at)) = self.pending {
            if ctl.now > at + ANSWER_MS {
                ctl.say(format_args!("A2DP: NO ANSWER TO SIGNAL {:02X} IN {} S: THE SIGNALLING CLOSES", signal, ANSWER_MS / 1000));
                self.pending = None;
                if let (Some(h), Some(s)) = (self.handle, self.sig) { chans.disconnect(ctl, h, s); }
            }
        }
        // A sink that opened signalling and configured nothing: we do it.
        if self.state == State::Idle && self.sig.is_some() && self.pending.is_none() && self.cfg.is_none() && ctl.now > self.since + IDLE_MS { self.discover(ctl, chans); }
        if self.state != State::Streaming { return; }
        let (Some(handle), Some(media), Some(cfg)) = (self.handle, self.media, self.cfg) else { return };
        let mtu = chans.get(media).map_or(l2cap::DEFAULT_MTU, |c| c.mtu);
        let frames = Self::frames_per_packet(&cfg, mtu).min(PACKET / cfg.frame_len());
        let per_packet = (frames * cfg.samples()) as u64;
        let due = (ctl.now - self.started + LEAD_MS) * cfg.hz() as u64 / 1000;
        // Fallen a second behind (the process stalled): that audio is skipped, not sent late.
        if due > self.sent + cfg.hz() as u64 { self.late += 1; self.sent = due - per_packet; }
        let mut packet = [0u8; PACKET];
        let mut pcm = [0i16; 256];
        while self.sent + per_packet <= due && ctl.backlog(handle) < BACKLOG {
            let rtp = rtp_header(self.seq, self.timestamp);
            let mut at = 0;
            for _ in 0..frames {
                let n = cfg.samples() * cfg.channels();
                self.fill(&mut pcm[..n], cfg.channels(), cfg.hz());
                at += self.enc.frame(&pcm[..n], &mut packet[at..]);
            }
            if !chans.send(ctl, handle, media, &[&rtp, &[frames as u8], &packet[..at]]) { break; }
            self.seq = self.seq.wrapping_add(1);
            self.timestamp = self.timestamp.wrapping_add(per_packet as u32);
            self.sent += per_packet;
            self.packets += 1;
        }
    }

    /// The tone, 0.4 s of every second, else silence.
    fn fill(&mut self, out: &mut [i16], channels: usize, hz: u32) {
        self.tone.fill(out, channels);
        if self.played % hz as u64 >= hz as u64 * 4 / 10 { out.fill(0); }
        self.played += (out.len() / channels) as u64;
    }
}

/// RTP's fixed header (RFC 3550) as A2DP's media packets carry it: version 2, payload type 96, the sequence number,
/// the timestamp in samples, source 1.
pub fn rtp_header(seq: u16, timestamp: u32) -> [u8; 12] {
    let (s, t) = (seq.to_be_bytes(), timestamp.to_be_bytes());
    [0x80, 0x60, s[0], s[1], t[0], t[1], t[2], t[3], 0, 0, 0, 1]
}

pub use sbc::Mode as SbcMode;
