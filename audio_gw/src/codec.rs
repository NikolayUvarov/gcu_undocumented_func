// An HD Audio codec's answers without the controller (551-DRV-0056): its widgets and the paths between them, decoded as
// the HD Audio specification 1.0a defines them (7.3.4), and the dump the gateway logs once at start. Builds on the host
// (tests/hda_codec_host.rs).
use core::fmt::Write;

// Widget types (audio widget capabilities, bits 23:20).
pub const OUTPUT: u8 = 0; pub const INPUT: u8 = 1; pub const MIXER: u8 = 2; pub const PIN: u8 = 4; pub const VENDOR: u8 = 0xF;
pub const CONNECTIONS: usize = 16;
// Pin capabilities (parameter 0xC).
pub const TRIGGER: u32 = 1 << 1; pub const PRESENCE: u32 = 1 << 2; pub const HP_AMP: u32 = 1 << 3; pub const OUT: u32 = 1 << 4;
pub const IN: u32 = 1 << 5; pub const EAPD: u32 = 1 << 16;
// Default devices (configuration bits 23:20).
pub const LINE_OUT: u32 = 0; pub const SPEAKER: u32 = 1; pub const HEADPHONE: u32 = 2; pub const LINE_IN: u32 = 8; pub const MICROPHONE: u32 = 0xA;

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Widget { pub nid: u8, pub kind: u8, pub caps: u32, pub pin_caps: u32, pub config: u32, pub connections: [u8; CONNECTIONS], pub count: u8 }

impl Widget {
    pub fn device(&self) -> u32 { (self.config >> 20) & 0xF }
    /// Connected to something: a jack, an internal device or both.
    pub fn connected(&self) -> bool { self.config >> 30 != 1 }
    pub fn internal(&self) -> bool { self.config >> 30 == 2 }
    /// A jack (alone or beside an internal device) that says whether something is plugged in.
    pub fn senses(&self) -> bool { self.config >> 30 & 1 == 0 && self.config & (1 << 8) == 0 && self.pin_caps & PRESENCE != 0 }
    pub fn links(&self) -> &[u8] { &self.connections[..self.count as usize] }
}

/// A chain from `from` through connections to a widget of `kind`, at most 5 deep: the node IDs and, for each node, the
/// index of the connection taken.
pub fn find(list: &[Widget], from: u8, kind: u8, depth: usize, path: &mut [(u8, u8); 6]) -> Option<usize> {
    let w = list.iter().find(|w| w.nid == from)?;
    path[depth].0 = from;
    if w.kind == kind && depth > 0 { return Some(depth + 1); }
    if depth >= 5 { return None; }
    for (index, &next) in w.links().iter().enumerate() {
        path[depth].1 = index as u8;
        if let Some(n) = find(list, next, kind, depth + 1, path) { return Some(n); }
    }
    None
}

/// A chain from the ADC `from` to the pin `target`.
pub fn find_pin(list: &[Widget], from: u8, target: u8, depth: usize, path: &mut [(u8, u8); 6]) -> Option<usize> {
    let w = list.iter().find(|w| w.nid == from)?;
    path[depth].0 = from;
    if from == target { return Some(depth + 1); }
    if depth >= 5 || (w.kind == PIN && depth > 0) { return None; }
    for (index, &next) in w.links().iter().enumerate() {
        path[depth].1 = index as u8;
        if let Some(n) = find_pin(list, next, target, depth + 1, path) { return Some(n); }
    }
    None
}

/// The headphone jack beside the output `speaker`: a pin that drives headphones from a DAC and senses them plugged in.
pub fn headphone_jack(list: &[Widget], speaker: u8) -> Option<Widget> {
    list.iter().copied().find(|w| w.kind == PIN && w.nid != speaker && w.device() == HEADPHONE && w.senses() && w.pin_caps & OUT != 0)
}

/// Whether a pin's sense (verb F09) says something is plugged in.
pub fn present(sense: u32) -> bool { sense >> 31 != 0 }

/// Text formatted into a fixed buffer; what does not fit is left out.
pub struct Text<const N: usize> { bytes: [u8; N], len: usize }
impl<const N: usize> Text<N> {
    pub const fn new() -> Self { Self { bytes: [0; N], len: 0 } }
    pub fn as_str(&self) -> &str { core::str::from_utf8(&self.bytes[..self.len]).unwrap_or("") }
}
impl<const N: usize> Write for Text<N> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let take = s.len().min(N - self.len);
        self.bytes[self.len..self.len + take].copy_from_slice(&s.as_bytes()[..take]);
        self.len += take;
        Ok(())
    }
}

/// A dump line's bytes, under logd's 200 (logd/src/ring.rs).
pub const WIDTH: usize = 190;

/// A dump's fields gathered into lines of at most WIDTH bytes, each starting with the same prefix; a field is never
/// split, and fields are parted by "; ".
pub struct Lines<'a> { line: Text<WIDTH>, prefix: usize, fields: usize, sink: &'a mut dyn FnMut(&str) }
impl<'a> Lines<'a> {
    pub fn new(sink: &'a mut dyn FnMut(&str), prefix: core::fmt::Arguments) -> Self {
        let mut line = Text::new();
        let _ = line.write_fmt(prefix);
        let prefix = line.len;
        Self { line, prefix, fields: 0, sink }
    }
    pub fn field(&mut self, args: core::fmt::Arguments) {
        let mut field = Text::<WIDTH>::new();
        let _ = field.write_fmt(args);
        if field.len == 0 { return; }
        if self.fields > 0 && self.line.len + 2 + field.len > WIDTH { self.flush(); }
        let _ = self.line.write_str(if self.fields == 0 { " " } else { "; " });
        let _ = self.line.write_str(field.as_str());
        self.fields += 1;
    }
    fn flush(&mut self) {
        if self.fields > 0 { (self.sink)(self.line.as_str()); }
        self.line.len = self.prefix; self.fields = 0;
    }
}
impl Drop for Lines<'_> { fn drop(&mut self) { self.flush(); } }

const KINDS: [&str; 16] = ["OUT", "IN", "MIXER", "SELECTOR", "PIN", "POWER", "KNOB", "BEEP", "TYPE 8", "TYPE 9", "TYPE A", "TYPE B", "TYPE C", "TYPE D", "TYPE E", "VENDOR"];
const WIDGET_FLAGS: [(u32, &str); 9] = [(1 << 0, "STEREO"), (1 << 1, "IN-AMP"), (1 << 2, "OUT-AMP"), (1 << 3, "AMP-OVERRIDE"), (1 << 4, "FORMAT-OVERRIDE"),
                                        (1 << 6, "PROCESSING"), (1 << 7, "UNSOLICITED"), (1 << 9, "DIGITAL"), (1 << 10, "POWER-CONTROL")];
const CONNECTIVITY: [&str; 4] = ["JACK", "UNCONNECTED", "INTERNAL", "JACK+INTERNAL"];
const DEVICES: [&str; 16] = ["LINE OUT", "SPEAKER", "HEADPHONE", "CD", "SPDIF OUT", "DIGITAL OUT", "MODEM LINE", "MODEM HANDSET", "LINE IN", "AUX",
                             "MICROPHONE", "TELEPHONY", "SPDIF IN", "DIGITAL IN", "RESERVED", "OTHER"];
const TYPES: [&str; 16] = ["UNKNOWN TYPE", "1/8 INCH", "1/4 INCH", "ATAPI", "RCA", "OPTICAL", "OTHER DIGITAL", "OTHER ANALOG", "DIN", "XLR", "RJ-11",
                           "COMBINATION", "TYPE C", "TYPE D", "TYPE E", "OTHER TYPE"];
const COLORS: [&str; 16] = ["UNKNOWN COLOR", "BLACK", "GREY", "BLUE", "GREEN", "RED", "ORANGE", "YELLOW", "PURPLE", "PINK", "COLOR A", "COLOR B",
                            "COLOR C", "COLOR D", "WHITE", "OTHER COLOR"];
const GROSS: [&str; 4] = ["EXTERNAL", "INTERNAL", "SEPARATE", "OTHER"];
const GEOMETRIC: [&str; 7] = ["", " REAR", " FRONT", " LEFT", " RIGHT", " TOP", " BOTTOM"];
const PIN_FLAGS: [(u32, &str); 11] = [(1 << 0, "IMPEDANCE"), (TRIGGER, "TRIGGER"), (PRESENCE, "PRESENCE"), (HP_AMP, "HP-AMP"), (OUT, "OUT"), (IN, "IN"),
                                      (1 << 6, "BALANCED"), (1 << 7, "HDMI"), (EAPD, "EAPD"), (1 << 24, "DP"), (1 << 27, "HBR")];
const VREFS: [(u32, &str); 5] = [(1 << 8, "HIZ"), (1 << 9, "50"), (1 << 10, "GND"), (1 << 12, "80"), (1 << 13, "100")];

struct Flags<'a, const N: usize>(u32, &'a [(u32, &'a str); N]);
impl<const N: usize> core::fmt::Display for Flags<'_, N> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        for &(bit, name) in self.1 { if self.0 & bit != 0 { write!(f, " {}", name)?; } }
        Ok(())
    }
}

// A pin's location (configuration bits 29:24): the special places by name, the others as gross and side.
struct Location(u32);
impl core::fmt::Display for Location {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self.0 {
            0x07 => f.write_str("REAR PANEL"), 0x08 => f.write_str("DRIVE BAY"), 0x17 => f.write_str("RISER"), 0x18 => f.write_str("DIGITAL DISPLAY"),
            0x19 => f.write_str("ATAPI"), 0x37 => f.write_str("INSIDE THE LID"), 0x38 => f.write_str("OUTSIDE THE LID"),
            l => write!(f, "{}{}", GROSS[(l >> 4) as usize & 3], GEOMETRIC.get(l as usize & 0xF).copied().unwrap_or(" ?")),
        }
    }
}

// An amplifier's capabilities (parameter 0xD or 0x12): raw, then settings and step, the 0 dB setting, mute.
struct AmpCaps(u32);
impl core::fmt::Display for AmpCaps {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let c = self.0;
        let step = ((c >> 16) & 0x7F) + 1;
        write!(f, "{:08X} ({} X {}.{:02} DB, 0 DB AT {:#X}{})", c, ((c >> 8) & 0x7F) + 1, step / 4, step % 4 * 25, c & 0x7F, if c >> 31 != 0 { ", MUTE" } else { "" })
    }
}

/// A codec's function group as it answered.
#[derive(Clone, Copy, Default)]
pub struct Group { pub address: u32, pub nid: u8, pub vendor: u32, pub revision: u32, pub subsystem: u32, pub nodes: u32, pub caps: u32, pub pcm: u32,
                   pub formats: u32, pub amp_in: u32, pub amp_out: u32, pub power: u32, pub gpio: u32, pub gpio_data: u32, pub gpio_mask: u32, pub gpio_direction: u32 }

/// What a widget is set to: its connection chosen, the effective amplifier capabilities and their left gains (bit 7
/// mute), and a pin's control, EAPD and sense.
#[derive(Clone, Copy, Default)]
pub struct State { pub selected: u8, pub amp_in: u32, pub amp_out: u32, pub gains_in: [u8; 4], pub gain_out: u8, pub control: u8, pub eapd: u8, pub sense: u32 }

pub fn dump_group(g: &Group, sink: &mut dyn FnMut(&str)) {
    let mut l = Lines::new(sink, format_args!("[AUDIO] CODEC {} AFG {:#X}:", g.address, g.nid));
    let first = (g.nodes >> 16) & 0xFF;
    l.field(format_args!("{:04X}:{:04X} REVISION {:08X}, SUBSYSTEM {:08X}", g.vendor >> 16, g.vendor & 0xFFFF, g.revision, g.subsystem));
    l.field(format_args!("NODES {:#X}-{:#X}", first, first + (g.nodes & 0xFF).max(1) - 1));
    l.field(format_args!("CAPS {:08X}, PCM {:08X}, FORMATS {:08X}, POWER {:08X}", g.caps, g.pcm, g.formats, g.power));
    if g.amp_in != 0 { l.field(format_args!("AMP IN {}", AmpCaps(g.amp_in))); }
    if g.amp_out != 0 { l.field(format_args!("AMP OUT {}", AmpCaps(g.amp_out))); }
    l.field(format_args!("GPIO {:08X} ({} IO, {} OUT, {} IN): DATA {:#04X} MASK {:#04X} DIRECTION {:#04X}", g.gpio, g.gpio & 0xFF, (g.gpio >> 8) & 0xFF, (g.gpio >> 16) & 0xFF,
                         g.gpio_data, g.gpio_mask, g.gpio_direction));
}

pub fn dump_widget(address: u32, w: &Widget, s: &State, sink: &mut dyn FnMut(&str)) {
    let mut l = Lines::new(sink, format_args!("[AUDIO] CODEC {} NID {:#X}:", address, w.nid));
    l.field(format_args!("{} {:08X}{}", KINDS[w.kind as usize & 0xF], w.caps, Flags(w.caps, &WIDGET_FLAGS)));
    if w.kind == PIN {
        let c = w.config;
        l.field(format_args!("CONFIG {:08X} {}, {} AT {}, {} {}{}, ASSOCIATION {}.{}", c, DEVICES[w.device() as usize], CONNECTIVITY[(c >> 30) as usize],
                             Location((c >> 24) & 0x3F), TYPES[(c >> 16) as usize & 0xF], COLORS[(c >> 12) as usize & 0xF], if c & (1 << 8) != 0 { ", NO DETECT" } else { "" },
                             (c >> 4) & 0xF, c & 0xF));
        let vref = Flags(w.pin_caps, &VREFS);
        l.field(format_args!("PIN CAPS {:08X}{}{}{}", w.pin_caps, Flags(w.pin_caps, &PIN_FLAGS), if w.pin_caps & 0x3700 != 0 { " VREF" } else { "" }, vref));
        l.field(format_args!("CONTROL {:#04X}", s.control));
        if w.pin_caps & EAPD != 0 { l.field(format_args!("EAPD {:#X}", s.eapd)); }
        if w.pin_caps & PRESENCE != 0 { l.field(format_args!("SENSE {:08X} ({})", s.sense, if present(s.sense) { "PRESENT" } else { "EMPTY" })); }
    }
    if w.count > 0 {
        let mut list = Text::<{ CONNECTIONS * 5 }>::new();
        for (i, n) in w.links().iter().enumerate() { let _ = write!(list, "{}{:#X}", if i > 0 { " " } else { "" }, n); }
        let chosen = w.count > 1 && w.kind != MIXER;
        if chosen { l.field(format_args!("FROM {} (CHOSEN {:#X})", list.as_str(), w.connections[s.selected as usize % w.count as usize])); }
        else { l.field(format_args!("FROM {}", list.as_str())); }
    }
    if w.caps & (1 << 1) != 0 {
        let inputs = if w.kind == MIXER { (w.count as usize).clamp(1, 4) } else { 1 };
        let mut gains = Text::<24>::new();
        for (i, g) in s.gains_in[..inputs].iter().enumerate() { let _ = write!(gains, "{}{:#04X}", if i > 0 { " " } else { "" }, g); }
        l.field(format_args!("AMP IN {} SET {}", AmpCaps(s.amp_in), gains.as_str()));
    }
    if w.caps & (1 << 2) != 0 { l.field(format_args!("AMP OUT {} SET {:#04X}", AmpCaps(s.amp_out), s.gain_out)); }
}

/// A vendor widget's processing coefficients, from index 0.
pub fn dump_coefficients(address: u32, nid: u8, coefficients: &[u16], sink: &mut dyn FnMut(&str)) {
    let mut l = Lines::new(sink, format_args!("[AUDIO] CODEC {} NID {:#X} COEFFICIENTS:", address, nid));
    for (i, c) in coefficients.iter().enumerate() { l.field(format_args!("{:02X}={:04X}", i, c)); }
}
