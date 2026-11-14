// Host tests of audio_gw's codec module (551-DRV-0056): the dump's decoding and lines; the paths, the speakers and the
// headphone jack found from the codec's own answers, on the MacBook Pro 10,1's CS4206 as its dump of 2026-11-13 gave
// them, and on another Mac's from a published dump (Launchpad question 274923, subsystem 106B0300).
#![allow(dead_code)] // the gateway uses parts these tests do not
#[path = "../audio_gw/src/codec.rs"]
mod codec;

use codec::*;

fn widget(nid: u8, caps: u32, pin_caps: u32, config: u32, links: &[u8]) -> Widget {
    let mut w = Widget { nid, kind: ((caps >> 20) & 0xF) as u8, caps, pin_caps, config, ..Widget::default() };
    w.connections[..links.len()].copy_from_slice(links);
    w.count = links.len() as u8;
    w
}

// That Mac's CS4206: DACs 0x2-0x4, ADCs 0x5 and 0x6, the headphone jack 0x9, the speaker 0xB, the line in 0xC.
fn other_mac() -> Vec<Widget> {
    vec![
        widget(0x02, 0x000D_041D, 0, 0, &[]),
        widget(0x03, 0x000D_041D, 0, 0, &[]),
        widget(0x04, 0x000D_041D, 0, 0, &[]),
        widget(0x05, 0x0018_051B, 0, 0, &[0x0C, 0x12]),
        widget(0x06, 0x0018_051B, 0, 0, &[0x0D, 0x0E]),
        widget(0x09, 0x0041_0581, 0x0000_001C, 0x012B_4040, &[0x02]),
        widget(0x0A, 0x0041_0581, 0x0000_0054, 0x4000_00F0, &[0x03]),
        widget(0x0B, 0x0041_0101, 0x0000_0050, 0x9010_0130, &[0x04]),
        widget(0x0C, 0x0041_048B, 0x0000_0024, 0x018B_3010, &[]),
        widget(0x0E, 0x0041_000B, 0x0000_0020, 0x4000_00F0, &[]),
        widget(0x11, 0x00F0_0040, 0, 0, &[]),
    ]
}

// The rest of that dump's widgets: the digital converters and pins, the microphone jack, DMIC2, the beep.
fn other_mac_whole() -> Vec<Widget> {
    let mut all = other_mac();
    all.extend([
        widget(0x07, 0x0018_0791, 0, 0, &[0x0F]),
        widget(0x08, 0x0004_0611, 0, 0, &[]),
        widget(0x0D, 0x0041_048B, 0x0000_1764, 0x4000_00F0, &[]),
        widget(0x0F, 0x0041_0681, 0x0000_0024, 0x01CB_E020, &[]),
        widget(0x10, 0x0041_0301, 0x0000_0010, 0x014B_E050, &[0x08]),
        widget(0x12, 0x0041_000B, 0x0000_0020, 0x4000_00F0, &[]),
        widget(0x13, 0x0070_0000, 0, 0, &[]),
        widget(0x14, 0x0004_0611, 0, 0, &[]),
        widget(0x15, 0x0041_0301, 0x0000_0010, 0x4000_00F0, &[0x14]),
    ]);
    all.sort_by_key(|w| w.nid);
    all
}

fn lines(f: impl FnOnce(&mut dyn FnMut(&str))) -> Vec<String> {
    let mut out = Vec::new();
    f(&mut |line: &str| out.push(line.to_string()));
    out
}

// A widget's lines as one: the prefix of each line after the first left out, wherever the fields wrapped.
fn joined(out: &[String]) -> String {
    out.iter().enumerate().map(|(i, l)| if i == 0 { l.as_str() } else { l.split_once(": ").map_or(l.as_str(), |(_, rest)| rest) }).collect::<Vec<_>>().join("; ")
}

#[test]
fn a_pins_configuration_and_capabilities_decoded() {
    let mac = other_mac();
    let jack = lines(|sink| dump_widget(0, &mac[5], &State { control: 0xC0, sense: 0x8000_0000, ..State::default() }, sink));
    let text = joined(&jack);
    for part in ["[AUDIO] CODEC 0 NID 0x9: PIN 00410581 STEREO UNSOLICITED POWER-CONTROL", "CONFIG 012B4040 HEADPHONE, JACK AT EXTERNAL REAR, COMBINATION GREEN, ASSOCIATION 4.0",
                 "PIN CAPS 0000001C PRESENCE HP-AMP OUT", "CONTROL 0xC0", "SENSE 80000000 (PRESENT)", "FROM 0x2"] {
        assert!(text.contains(part), "{part} in {text}");
    }
    let speaker = joined(&lines(|sink| dump_widget(0, &mac[7], &State::default(), sink)));
    assert!(speaker.contains("CONFIG 90100130 SPEAKER, INTERNAL AT INTERNAL, UNKNOWN TYPE UNKNOWN COLOR, NO DETECT, ASSOCIATION 3.0"), "{speaker}");
    assert!(!speaker.contains("SENSE"), "no presence detect, no sense: {speaker}");
    let unconnected = joined(&lines(|sink| dump_widget(0, &mac[6], &State::default(), sink)));
    assert!(unconnected.contains("LINE OUT, UNCONNECTED AT EXTERNAL, UNKNOWN TYPE UNKNOWN COLOR, ASSOCIATION 15"), "{unconnected}");
    // A microphone pin with every bias, and a mixer's input amplifiers.
    let mic = widget(0x0D, 0x0041_048B, 0x0000_1724, 0x90A7_0110, &[]);
    let text = joined(&lines(|sink| dump_widget(1, &mic, &State { amp_in: 0x0027_0300, gains_in: [3, 0, 0, 0], ..State::default() }, sink)));
    assert!(text.contains("[AUDIO] CODEC 1 NID 0xD: PIN 0041048B STEREO IN-AMP AMP-OVERRIDE UNSOLICITED POWER-CONTROL; CONFIG 90A70110 MICROPHONE, INTERNAL AT INTERNAL, OTHER ANALOG UNKNOWN COLOR, NO DETECT, ASSOCIATION 1.0"), "{text}");
    assert!(text.contains("PIN CAPS 00001724 PRESENCE IN VREF HIZ 50 GND 80"), "{text}");
    assert!(text.contains("AMP IN 00270300 (4 X 10.00 DB, 0 DB AT 0x0) SET 0x03"), "{text}");
    let dac = joined(&lines(|sink| dump_widget(0, &mac[0], &State { amp_out: 0x8001_7F73, gain_out: 0x5B, ..State::default() }, sink)));
    assert!(dac.contains("OUT 000D041D STEREO OUT-AMP AMP-OVERRIDE FORMAT-OVERRIDE POWER-CONTROL; AMP OUT 80017F73 (128 X 0.50 DB, 0 DB AT 0x73, MUTE) SET 0x5B"), "{dac}");
    let adc = joined(&lines(|sink| dump_widget(0, &mac[4], &State { selected: 1, ..State::default() }, sink)));
    assert!(adc.contains("FROM 0xD 0xE (CHOSEN 0xE)"), "{adc}");
}

#[test]
fn lines_fit_the_log_and_keep_their_fields_whole() {
    let mut w = widget(0x20, 0x0020_0103, 0, 0, &[]);
    w.count = CONNECTIONS as u8;
    for (i, c) in w.connections.iter_mut().enumerate() { *c = 0x60 + i as u8; }
    let s = State { amp_in: 0x8005_1F1F, amp_out: 0x8005_1F1F, ..State::default() };
    let out = lines(|sink| dump_widget(0, &w, &s, sink));
    for line in &out { assert!(line.len() <= WIDTH, "{} bytes: {line}", line.len()); assert!(line.starts_with("[AUDIO] CODEC 0 NID 0x20: "), "{line}"); }
    let coefficients: Vec<u16> = (0..32).collect();
    let out = lines(|sink| dump_coefficients(0, 0x11, &coefficients, sink));
    assert_eq!(out.len(), 2, "{out:?}");
    assert!(out.iter().all(|l| l.len() <= WIDTH && l.starts_with("[AUDIO] CODEC 0 NID 0x11 COEFFICIENTS: ")), "{out:?}");
    let joined = out.join(" ");
    assert!(joined.contains("04=0004") && joined.contains("1F=001F"), "{joined}");
    // A pin with every flag stays within the line, whatever wraps.
    let busy = widget(0x0A, 0x0041_07FF, 0x0901_37FF, 0xF1FF_FFFF, &[1, 2, 3, 4]);
    for line in lines(|sink| dump_widget(15, &busy, &State { sense: 0x7FFF_FFFF, ..State::default() }, sink)) { assert!(line.len() <= WIDTH, "{line}"); }
}

#[test]
fn the_function_group_line() {
    let g = Group { address: 0, nid: 1, vendor: 0x1013_4206, revision: 0x0010_0302, subsystem: 0x106B_2800, nodes: 0x0002_0014, gpio: 4, gpio_data: 8, gpio_mask: 0xA, gpio_direction: 0xA, ..Group::default() };
    let text = joined(&lines(|sink| dump_group(&g, sink)));
    for part in ["[AUDIO] CODEC 0 AFG 0x1: 1013:4206 REVISION 00100302, SUBSYSTEM 106B2800", "NODES 0x2-0x15", "GPIO 00000004 (4 IO, 0 OUT, 0 IN): DATA 0x08 MASK 0x0A DIRECTION 0x0A"] {
        assert!(text.contains(part), "{part} in {text}");
    }
    assert!(!text.contains("AMP IN"), "no default amplifier, none shown: {text}");
}

#[test]
fn the_headphone_jack_from_the_codecs_answers() {
    let mac = other_mac();
    assert_eq!(headphone_jack(&mac, 0x0B).map(|w| w.nid), Some(0x09));
    assert_eq!(headphone_jack(&mac, 0x09), None, "the output itself is no jack beside it");
    // Without presence detect, or with the configuration's "no detect", it cannot be watched.
    let mut deaf = mac.clone();
    deaf[5].pin_caps &= !PRESENCE;
    assert_eq!(headphone_jack(&deaf, 0x0B), None);
    let mut overridden = mac.clone();
    overridden[5].config |= 1 << 8;
    assert_eq!(headphone_jack(&overridden, 0x0B), None);
    // An internal headphone is no jack.
    let mut internal = mac.clone();
    internal[5].config = 0x9021_0110;
    assert_eq!(headphone_jack(&internal, 0x0B), None);
    assert!(present(0x8000_0000) && !present(0x7FFF_FFFF));
    // The paths: the jack to its DAC, the ADC to the digital microphone's pin.
    let mut path = [(0u8, 0u8); 6];
    assert_eq!(find(&mac, 0x09, OUTPUT, 0, &mut path), Some(2));
    assert_eq!(path[..2], [(0x09, 0), (0x02, 0)]);
    assert_eq!(find_pin(&mac, 0x06, 0x0E, 0, &mut path), Some(2));
    assert_eq!(path[..2], [(0x06, 1), (0x0E, 0)]);
}

#[test]
fn a_cs4206_dump_stays_within_the_log() {
    // logd keeps 2048 records and takes 64 a second from a sender; the gateway sends the dump 40 lines a second.
    let g = Group { vendor: 0x1013_4206, subsystem: 0x106B_2800, nodes: 0x0002_0014, gpio: 4, ..Group::default() };
    let s = State { amp_in: 0x8003_3F33, amp_out: 0x8001_7F73, sense: 0x8000_0000, ..State::default() };
    let mut count = lines(|sink| dump_group(&g, sink)).len();
    for w in macbook() {
        let out = lines(|sink| dump_widget(0, &w, &s, sink));
        assert!(out.len() <= 3 && out.iter().all(|l| l.len() <= WIDTH), "{out:?}");
        count += out.len();
        if w.kind == VENDOR { count += lines(|sink| dump_coefficients(0, w.nid, &[0; 32], sink)).len(); }
    }
    // The Mac's own dump of 2026-11-13 took 33 lines, the jack's line apart.
    assert!((30..=40).contains(&count), "{count} lines for the CS4206's 20 widgets");
}

// The MacBook Pro 10,1's CS4206 as audio_gw dumped it on 2026-11-13 (subsystem 106B2800): the headphone jack 0x9 from
// DAC 0x2, two speaker pins of association 1 (0xA from DAC 0x3, 0xB from DAC 0x4), the digital microphone 0xE to ADC 0x6.
fn macbook() -> Vec<Widget> {
    vec![
        widget(0x02, 0x000D_041D, 0, 0, &[]), widget(0x03, 0x000D_041D, 0, 0, &[]), widget(0x04, 0x000D_041D, 0, 0, &[]),
        widget(0x05, 0x0018_051B, 0, 0, &[0x0C, 0x12]), widget(0x06, 0x0018_051B, 0, 0, &[0x0D, 0x0E]), widget(0x07, 0x0018_0791, 0, 0, &[0x0F]),
        widget(0x08, 0x0004_0611, 0, 0, &[]),
        widget(0x09, 0x0041_0581, 0x0000_001C, 0x002B_4020, &[0x02]),
        widget(0x0A, 0x0041_0581, 0x0000_0054, 0x9010_0112, &[0x03]),
        widget(0x0B, 0x0041_0101, 0x0000_0050, 0x9010_0110, &[0x04]),
        widget(0x0C, 0x0041_048B, 0x0000_0024, 0x4000_00F0, &[]),
        widget(0x0D, 0x0041_048B, 0x0000_1764, 0x4000_00F0, &[]),
        widget(0x0E, 0x0041_000B, 0x0000_0020, 0x90A6_0100, &[]),
        widget(0x0F, 0x0041_0681, 0x0000_0024, 0x4000_00F0, &[]),
        widget(0x10, 0x0041_0301, 0x0000_0010, 0x004B_E030, &[0x08]),
        widget(0x11, 0x00F0_0040, 0, 0, &[]),
        widget(0x12, 0x0041_000B, 0x0000_0020, 0x4000_00F0, &[]),
        widget(0x13, 0x0070_0000, 0, 0, &[]), widget(0x14, 0x0004_0611, 0, 0, &[]),
        widget(0x15, 0x0041_0301, 0x0000_0010, 0x4000_00F0, &[0x14]),
    ]
}

// The codec's answers as a model: each pin's sense, and every verb that sets something, in order.
#[derive(Default)]
struct Model { senses: Vec<(u8, u32)>, set: Vec<(u8, u32, u32)>, paused: usize }
impl Verbs for Model {
    fn verb(&mut self, nid: u8, verb: u32, payload: u32) -> Option<u32> {
        if verb == 0xF09 { return Some(self.senses.iter().find(|(n, _)| *n == nid).map_or(0, |&(_, s)| s)); }
        self.set.push((nid, verb, payload));
        Some(0)
    }
    fn pause(&mut self, ms: usize) { self.paused += ms; }
}
impl Model { fn plug(&mut self, nid: u8, sense: u32) { self.senses.retain(|(n, _)| *n != nid); self.senses.push((nid, sense)); } }

fn nid(list: &[Widget], nid: u8) -> Widget { *list.iter().find(|w| w.nid == nid).unwrap() }

#[test]
fn the_macbook_pros_paths_from_its_own_answers() {
    let mac = macbook();
    let (pin, path, depth) = output(&mac).unwrap();
    assert_eq!((pin.nid, &path[..depth]), (0x0A, &[(0x0A, 0), (0x03, 0)][..]), "the first speaker pin, as its log says");
    let (others, n) = companions(&mac, &pin);
    assert_eq!(n, 1, "one more speaker pin of association 1");
    assert_eq!((others[0].0.nid, &others[0].1[..others[0].2]), (0x0B, &[(0x0B, 0), (0x04, 0)][..]));
    let (jack, path, depth) = jack_path(&mac, 0x0A).unwrap();
    assert_eq!((jack.nid, &path[..depth]), (0x09, &[(0x09, 0), (0x02, 0)][..]), "the jack from its own DAC");
    let (mic, path, depth) = input(&mac).unwrap();
    assert_eq!((mic.nid, &path[..depth]), (0x0E, &[(0x06, 1), (0x0E, 0)][..]), "DMIC1 through ADC 0x6's second connection");
    assert_eq!(cirrus_digital_microphone(0x1013_4206, mic.nid), Some(1 << 3));
    assert_eq!(cirrus_digital_microphone(0x1013_4207, 0x12), Some(1 << 4));
    assert_eq!(cirrus_digital_microphone(0x1013_4206, 0x0D), None, "an analog pin");
    assert_eq!(cirrus_digital_microphone(0x1013_4210, 0x0E), None, "another Cirrus codec");
    // The other Mac: one speaker pin (0xB, association 3), the jack 0x9, the line in 0xC as input (no microphone pin).
    let other = other_mac_whole();
    let (pin, ..) = output(&other).unwrap();
    assert_eq!((pin.nid, companions(&other, &pin).1, jack_path(&other, pin.nid).map(|j| j.0.nid)), (0x0B, 0, Some(0x09)));
    assert_eq!(input(&other).map(|i| i.0.nid), Some(0x0C));
}

#[test]
fn nothing_to_switch_without_a_speaker_and_a_jack() {
    let mac = macbook();
    assert!(jack_path(&mac, 0x09).is_none(), "headphones as the only output");
    let mut desktop = mac.clone();
    for w in desktop.iter_mut() { if w.nid == 0x0A { w.config = 0x0101_4010; } }
    assert!(jack_path(&desktop, 0x0A).is_none(), "a line out is not silenced by the headphones");
    assert_eq!(companions(&desktop, &nid(&desktop, 0x0A)).1, 0, "nor joined by speakers");
    let mut fed = mac.clone();
    fed.retain(|w| w.nid != 0x02);
    assert!(jack_path(&fed, 0x0A).is_none(), "a jack no DAC reaches is not watched");
    let mut alone = mac.clone();
    for w in alone.iter_mut() { if w.nid == 0x0B { w.config = 0x9010_01F0; } }
    assert_eq!(companions(&alone, &nid(&alone, 0x0A)).1, 0, "association 15 groups nothing");
}

#[test]
fn apples_amplifiers_by_machine() {
    assert_eq!(apple_amplifiers(0x1013_4206, 0x106B_2800), Some(Amplifiers { speakers: 0x08, headphones: 0x02 }), "MacBook Pro 10,1: GPIO1");
    assert_eq!(apple_amplifiers(0x1013_4206, 0x106B_2000), Some(Amplifiers { speakers: 0x08, headphones: 0x04 }), "iMac 12,2: GPIO2");
    assert_eq!(apple_amplifiers(0x1013_4206, 0x106B_0300), Some(Amplifiers { speakers: 0x08, headphones: 0x02 }), "any other Apple machine: GPIO1");
    assert_eq!(apple_amplifiers(0x1013_4206, 0x1028_0100), None, "not Apple's");
    assert_eq!(apple_amplifiers(0x10EC_0269, 0x106B_2800), None, "not Cirrus's");
}

#[test]
fn headphones_in_silence_both_speakers_and_out_bring_them_back() {
    let mac = macbook();
    let mut speakers = [Widget::default(); SPEAKERS];
    speakers[0] = nid(&mac, 0x0A); speakers[1] = nid(&mac, 0x0B);
    let mut jack = Jack { pin: nid(&mac, 0x09), dac: 0x02, speakers, count: 2, afg: 1, amplifiers: apple_amplifiers(0x1013_4206, 0x106B_2800), headphones: None };
    let mut codec = Model::default();
    // At start, empty (as the Mac's sense 00000000 said): the speakers play, the jack's pin off, the speakers' amplifier.
    let (route, sense) = jack.watch(&mut codec).unwrap();
    assert_eq!((route, sense), (Route { speaker: 0x40, speaker_eapd: 2, jack: 0, gpio: Some(0x08) }, 0));
    assert_eq!(codec.set, [(0x0A, 0x707, 0x40), (0x0B, 0x707, 0x40), (0x09, 0x707, 0), (0x01, 0x715, 0x08)]);
    codec.set.clear();
    assert!(jack.watch(&mut codec).is_none(), "no change, nothing set");
    assert!(codec.set.is_empty());
    // Plugged: the jack's pin on with its headphone amplifier, both speaker pins off, the headphones' amplifier alone.
    codec.plug(0x09, 0x8000_0000);
    let (route, _) = jack.watch(&mut codec).unwrap();
    assert_eq!(route, Route { speaker: 0, speaker_eapd: 0, jack: 0xC0, gpio: Some(0x02) });
    assert_eq!(codec.set, [(0x0A, 0x707, 0), (0x0B, 0x707, 0), (0x09, 0x707, 0xC0), (0x01, 0x715, 0x02)]);
    assert_eq!(jack.headphones, Some(true));
    codec.set.clear();
    assert!(jack.watch(&mut codec).is_none());
    // Pulled out: back to the speakers.
    codec.plug(0x09, 0x7FFF_FFFF);
    assert_eq!(jack.watch(&mut codec).unwrap().0, Route { speaker: 0x40, speaker_eapd: 2, jack: 0, gpio: Some(0x08) });
    assert_eq!(codec.set, [(0x0A, 0x707, 0x40), (0x0B, 0x707, 0x40), (0x09, 0x707, 0), (0x01, 0x715, 0x08)]);
}

#[test]
fn eapd_a_trigger_and_no_apple_amplifiers() {
    // A speaker with EAPD, a jack that needs its sense triggered and has no headphone amplifier, not an Apple machine.
    let mut speakers = [Widget::default(); SPEAKERS];
    speakers[0] = widget(0x14, 0x0040_0181, EAPD | OUT, 0x9017_0110, &[0x02]);
    let jack = widget(0x15, 0x0040_0181, TRIGGER | PRESENCE | OUT | EAPD, 0x0221_4020, &[0x03]);
    let mut j = Jack { pin: jack, dac: 0x03, speakers, count: 1, afg: 1, amplifiers: None, headphones: Some(false) };
    let mut codec = Model::default();
    codec.plug(0x15, 0x8000_0000);
    let (route, _) = j.watch(&mut codec).unwrap();
    assert_eq!(route, Route { speaker: 0, speaker_eapd: 0, jack: 0x40, gpio: None });
    assert_eq!(codec.set, [(0x15, 0x709, 0), (0x14, 0x707, 0), (0x14, 0x70C, 0), (0x15, 0x707, 0x40), (0x15, 0x70C, 2)], "the trigger before the sense; no GPIO");
    assert_eq!(codec.paused, 1);
}
