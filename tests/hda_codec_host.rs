// Host tests of audio_gw's codec module (551-DRV-0056): the dump's decoding and lines, and the headphone jack found
// from the codec's own answers. The CS4206 here is a model built from a published codec dump of another Mac
// (Launchpad question 274923, subsystem 106B0300), not the MacBook Pro's.
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
    for w in other_mac_whole() {
        let out = lines(|sink| dump_widget(0, &w, &s, sink));
        assert!(out.len() <= 3 && out.iter().all(|l| l.len() <= WIDTH), "{out:?}");
        count += out.len();
        if w.kind == VENDOR { count += lines(|sink| dump_coefficients(0, w.nid, &[0; 32], sink)).len(); }
    }
    assert!((25..=45).contains(&count), "{count} lines for the CS4206's 20 widgets");
}
