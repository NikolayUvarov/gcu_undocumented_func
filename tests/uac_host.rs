//! Host tests of libmind/src/uac.rs (000-DRV-0073): an audio function's settings from its configuration descriptor
//! (the STM-Speaker's own bytes), an asynchronous device's feedback read in each format, the frames each packet
//! carries, and samples to and from one channel.
#[path = "../libmind/src/uac.rs"]
mod uac;
use uac::*;

fn le16(v: u16) -> [u8; 2] { v.to_le_bytes() }

// The STM-Speaker's configuration (firmware/usb/sp_usb_descriptors.c, the mono capture build): control interface 0,
// playback interface 1 (async OUT 0x01 with feedback 0x82), capture interface 2 (async IN 0x81, mono), HID interface 3.
fn speakerphone() -> Vec<u8> {
    let mut c = vec![9, 2, 0, 0, 4, 1, 0, 0x80, 0xFA];
    c.extend([9, 4, 0, 0, 0, 1, 1, 0, 0]); // audio control
    c.extend([10, 0x24, 1, 0x00, 0x01, 0x40, 0x00, 2, 1, 2]); // AC header, two streaming interfaces
    c.extend([12, 0x24, 2, 0x11, 0x01, 0x01, 0, 2, 3, 0, 0, 0]); // input terminal: USB streaming
    c.extend([9, 0x24, 3, 0x13, 0x01, 0x01, 0, 0x12, 0]); // output terminal
    c.extend([9, 4, 1, 0, 0, 1, 2, 0, 5]);
    c.extend([9, 4, 1, 1, 2, 1, 2, 0, 5]);
    c.extend([7, 0x24, 1, 1, 1, 1, 0]);
    c.extend([11, 0x24, 2, 1, 2, 2, 16, 1, 0x80, 0xBB, 0]);
    c.extend([9, 5, 0x01, 0x05]); c.extend(le16(196)); c.extend([1, 0, 0x82]);
    c.extend([7, 0x25, 1, 0, 0, 0, 0]);
    c.extend([9, 5, 0x82, 0x01]); c.extend(le16(3)); c.extend([1, 0, 0]);
    c.extend([9, 4, 2, 0, 0, 1, 2, 0, 6]);
    c.extend([9, 4, 2, 1, 1, 1, 2, 0, 6]);
    c.extend([7, 0x24, 1, 0x13, 1, 1, 0]);
    c.extend([11, 0x24, 2, 1, 1, 2, 16, 1, 0x80, 0xBB, 0]);
    c.extend([9, 5, 0x81, 0x05]); c.extend(le16(98)); c.extend([1, 0, 0]);
    c.extend([7, 0x25, 1, 0, 0, 0, 0]);
    c.extend([9, 4, 3, 0, 1, 3, 0, 0, 7]);
    c.extend([7, 5, 0x83, 0x03, 64, 0, 1]);
    let len = c.len() as u16; c[2..4].copy_from_slice(&le16(len));
    c
}

#[test]
fn the_speakerphones_settings() {
    let f = parse(&speakerphone()).expect("an audio function");
    assert_eq!(f.control, 0);
    assert_eq!(f.playback, Some(Setting { interface: 1, alternate: 1, channels: 2, endpoint: 0x01, packet: 196, interval: 1, sync: 1, feedback: Some((0x82, 3)), rated: false }));
    assert_eq!(f.capture, Some(Setting { interface: 2, alternate: 1, channels: 1, endpoint: 0x81, packet: 98, interval: 1, sync: 1, feedback: None, rated: false }));
    assert_eq!(f.playback.unwrap().frame(), 4);
    assert!(f.capture.unwrap().input());
}

// An endpoint that takes its sampling frequency by request says so in its class descriptor (bmAttributes bit 0): the
// gateway then tells it 48 kHz. The DEXP's own USB audio rasped without that (000-DRV-0073).
#[test]
fn an_endpoint_that_takes_its_rate_by_request_is_marked() {
    // The playback endpoint's class descriptor with the bit: playback is marked, capture is not.
    let mut c = speakerphone();
    let at = c.windows(3).position(|w| w == [7, 0x25, 1]).unwrap();
    c[at + 3] = 0x01;
    let f = parse(&c).unwrap();
    assert!(f.playback.unwrap().rated);
    assert!(!f.capture.unwrap().rated);
    // The bit in a class descriptor that follows the feedback endpoint is not the data endpoint's.
    let mut c = speakerphone();
    let feedback = c.windows(4).position(|w| w == [9, 5, 0x82, 0x01]).unwrap();
    c.splice(feedback + 9..feedback + 9, [7, 0x25, 1, 0x01, 0, 0, 0]);
    let len = c.len() as u16; c[2..4].copy_from_slice(&le16(len));
    assert!(!parse(&c).unwrap().playback.unwrap().rated);
    // Other bits (pitch, packets of the largest size only) are not this one.
    let mut c = speakerphone();
    c[at + 3] = 0x82;
    assert!(!parse(&c).unwrap().playback.unwrap().rated);
}

#[test]
fn settings_it_cannot_use_are_left_out() {
    // 24-bit samples, then 44.1 kHz alone: no playback; a continuous range that holds 48 kHz is used.
    let mut c = speakerphone();
    let at = c.windows(4).position(|w| w == [11, 0x24, 2, 1]).unwrap();
    c[at + 5] = 3; c[at + 6] = 24;
    assert_eq!(parse(&c).unwrap().playback, None);
    let mut c = speakerphone();
    c[at + 8..at + 11].copy_from_slice(&[0x44, 0xAC, 0]);
    assert_eq!(parse(&c).unwrap().playback, None);
    let mut c = speakerphone();
    c.splice(at..at + 11, [14, 0x24, 2, 1, 2, 2, 16, 0, 0x40, 0x1F, 0, 0x00, 0x77, 1]);
    let len = c.len() as u16; c[2..4].copy_from_slice(&le16(len));
    assert_eq!(parse(&c).unwrap().playback.map(|s| s.endpoint), Some(0x01));
    // No audio control interface, or nothing usable: not a function.
    assert_eq!(parse(&[9, 2, 9, 0, 0, 1, 0, 0x80, 0]), None);
}

#[test]
fn feedback_in_every_format() {
    // 48.0 frames a millisecond at full speed in 10.14 (3 bytes), and the same sent as 16.16.
    assert_eq!(feedback(&(48u32 << 14).to_le_bytes()[..3], false), Some(48 << 16));
    assert_eq!(feedback(&(48u32 << 16).to_le_bytes()[..3], false), Some(48 << 16));
    // 47.5 at full speed in 10.14; 6.0 a microframe at high speed in 16.16.
    assert_eq!(feedback(&((47u32 << 14) + (1 << 13)).to_le_bytes()[..3], false), Some((47 << 16) + (1 << 15)));
    assert_eq!(feedback(&(6u32 << 16).to_le_bytes(), true), Some(48 << 16));
    // Nonsense and short values are not read.
    assert_eq!(feedback(&[0, 0, 0], false), None);
    assert_eq!(feedback(&[1, 2], false), None);
}

#[test]
fn the_pacer_follows_the_rate_and_carries_the_fraction() {
    let mut p = Pacer::new(49);
    assert_eq!((0..10).map(|_| p.next()).sum::<usize>(), 480);
    p.set((48 << 16) + (1 << 15)); // 48.5
    let frames: Vec<usize> = (0..4).map(|_| p.next()).collect();
    assert_eq!(frames.iter().sum::<usize>(), 194);
    assert!(frames.iter().all(|&f| f == 48 || f == 49));
    p.set(60 << 16); // beyond a frame from 48: held at 49
    assert_eq!(p.rate(), 49 << 16);
    let mut small = Pacer::new(10); // a packet too small for the rate never builds a debt
    assert!((0..100).all(|_| small.next() == 10));
}

#[test]
fn samples_to_and_from_one_channel() {
    let stereo = [0x10, 0x00, 0x30, 0x00, 0xFF, 0xFF, 0x01, 0x00]; // (16, 48), (-1, 1)
    let mut mono = [0u8; 4];
    assert_eq!(to_wire(&stereo, 1, &mut mono), 4);
    assert_eq!(mono, [0x20, 0x00, 0x00, 0x00]); // 32, 0
    let mut back = [0u8; 8];
    assert_eq!(from_wire(&mono, 1, &mut back), 8);
    assert_eq!(back, [0x20, 0, 0x20, 0, 0, 0, 0, 0]);
    let mut same = [0u8; 8];
    assert_eq!(to_wire(&stereo, 2, &mut same), 8);
    assert_eq!(same, stereo);
}
