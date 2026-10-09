//! Host tests of the USB HID decoding (libmind/src/hid.rs, issue 164): boot keyboard reports become PS/2 set 1 scan
//! codes with releases, modifiers, E0 keys and host-side repeat; report descriptors of a tablet and a mouse (QEMU's
//! usb-tablet and usb-mouse) give the fields their reports are read with.
#![allow(dead_code)]
#[path = "../common/abi.rs"]
mod abi;
#[path = "../libmind/src/hid.rs"]
mod hid;

use abi::{pointer_absolute_fields, pointer_across, pointer_fields, POINTER_LEFT, POINTER_RIGHT, POINTER_SCALE};
use hid::*;

fn keys(keyboard: &mut Keyboard, report: [u8; 8], now: u64) -> Vec<u8> {
    let mut out = Vec::new();
    keyboard.feed(&report, now, &mut |b| out.push(b));
    out
}

#[test]
fn keys_press_and_release() {
    let mut k = Keyboard::new();
    assert_eq!(keys(&mut k, [0, 0, 0x04, 0, 0, 0, 0, 0], 0), [0x1E]); // a
    assert_eq!(keys(&mut k, [0, 0, 0x04, 0x05, 0, 0, 0, 0], 10), [0x30]); // b while a is held
    assert_eq!(keys(&mut k, [0, 0, 0x05, 0, 0, 0, 0, 0], 20), [0x9E]); // a released, b still held
    assert_eq!(keys(&mut k, [0; 8], 30), [0xB0]);
}

#[test]
fn modifiers_and_extended_keys() {
    let mut k = Keyboard::new();
    assert_eq!(keys(&mut k, [0x02, 0, 0, 0, 0, 0, 0, 0], 0), [0x2A]); // left Shift
    assert_eq!(keys(&mut k, [0x12, 0, 0, 0, 0, 0, 0, 0], 0), [0xE0, 0x1D]); // right Ctrl too
    assert_eq!(keys(&mut k, [0x00, 0, 0x52, 0, 0, 0, 0, 0], 0), [0xAA, 0xE0, 0x9D, 0xE0, 0x48]); // both up, Up arrow down
    assert_eq!(keys(&mut k, [0; 8], 0), [0xE0, 0xC8]);
}

#[test]
fn rollover_errors_keep_the_state() {
    let mut k = Keyboard::new();
    keys(&mut k, [0, 0, 0x04, 0, 0, 0, 0, 0], 0);
    assert!(keys(&mut k, [0, 0, 1, 1, 1, 1, 1, 1], 0).is_empty());
    assert_eq!(keys(&mut k, [0; 8], 0), [0x9E]);
}

#[test]
fn held_keys_repeat() {
    let mut k = Keyboard::new();
    keys(&mut k, [0, 0, 0x2C, 0, 0, 0, 0, 0], 1000); // space
    let mut out = Vec::new();
    k.repeat(1000 + REPEAT_DELAY_MS - 1, &mut |b| out.push(b));
    assert!(out.is_empty());
    k.repeat(1000 + REPEAT_DELAY_MS, &mut |b| out.push(b));
    k.repeat(1000 + REPEAT_DELAY_MS + REPEAT_MS / 2, &mut |b| out.push(b));
    k.repeat(1000 + REPEAT_DELAY_MS + REPEAT_MS, &mut |b| out.push(b));
    assert_eq!(out, [0x39, 0x39]);
    keys(&mut k, [0; 8], 2000);
    out.clear(); k.repeat(5000, &mut |b| out.push(b));
    assert!(out.is_empty(), "a released key does not repeat");
}

#[test]
fn every_letter_and_digit_has_a_code() {
    for usage in 0x04..=0x45u8 { assert!(scancode(usage).is_some(), "usage {usage:#x}"); }
    assert_eq!(scancode(0x1E), Some((false, 0x02))); // 1
    assert_eq!(scancode(0x27), Some((false, 0x0B))); // 0
    assert_eq!(scancode(0x14), Some((false, 0x10))); // q
}

// QEMU's usb-tablet: three buttons, five bits of padding, X and Y 0..32767 in 16 bits, the wheel -127..127.
const TABLET: [u8; 74] = [0x05, 0x01, 0x09, 0x02, 0xa1, 0x01, 0x09, 0x01, 0xa1, 0x00, 0x05, 0x09, 0x19, 0x01, 0x29, 0x03, 0x15, 0x00, 0x25, 0x01,
    0x95, 0x03, 0x75, 0x01, 0x81, 0x02, 0x95, 0x01, 0x75, 0x05, 0x81, 0x01, 0x05, 0x01, 0x09, 0x30, 0x09, 0x31, 0x15, 0x00, 0x26, 0xff, 0x7f,
    0x35, 0x00, 0x46, 0xff, 0x7f, 0x75, 0x10, 0x95, 0x02, 0x81, 0x02, 0x05, 0x01, 0x09, 0x38, 0x15, 0x81, 0x25, 0x7f, 0x35, 0x00, 0x45, 0x00,
    0x75, 0x08, 0x95, 0x01, 0x81, 0x06, 0xc0, 0xc0];
// QEMU's usb-mouse: three buttons, padding, X, Y and the wheel as relative signed bytes.
const MOUSE: [u8; 52] = [0x05, 0x01, 0x09, 0x02, 0xa1, 0x01, 0x09, 0x01, 0xa1, 0x00, 0x05, 0x09, 0x19, 0x01, 0x29, 0x03, 0x15, 0x00, 0x25, 0x01,
    0x95, 0x03, 0x75, 0x01, 0x81, 0x02, 0x95, 0x01, 0x75, 0x05, 0x81, 0x01, 0x05, 0x01, 0x09, 0x30, 0x09, 0x31, 0x09, 0x38, 0x15, 0x81, 0x25,
    0x7f, 0x75, 0x08, 0x95, 0x03, 0x81, 0x06, 0xc0, 0xc0];

fn events(pointer: &Pointer, report: &[u8]) -> Vec<usize> { let mut out = Vec::new(); pointer.events(report, &mut |e| out.push(e)); out }

#[test]
fn a_tablet_descriptor_gives_absolute_positions() {
    let tablet = Pointer::parse(&TABLET).expect("layout");
    assert!(tablet.absolute);
    assert_eq!((tablet.x.offset, tablet.x.size, tablet.y.offset, tablet.y.size), (8, 16, 24, 16));
    assert_eq!(tablet.wheel.map(|w| (w.offset, w.size)), Some((40, 8)));
    let out = events(&tablet, &[1, 0xFF, 0x7F, 0, 0, 0]);
    assert_eq!(out.len(), 1);
    let (buttons, x, y, wheel) = pointer_absolute_fields(out[0]).expect("absolute");
    assert_eq!((buttons, x, y, wheel), (POINTER_LEFT, POINTER_SCALE - 1, 0, 0));
}

#[test]
fn a_mouse_descriptor_gives_movements() {
    let mouse = Pointer::parse(&MOUSE).expect("layout");
    assert!(!mouse.absolute);
    let out = events(&mouse, &[2, 5, 0xFD, 0x01]); // right button, x +5, y -3, wheel +1 (away from the user)
    assert_eq!(out.len(), 1);
    assert_eq!(pointer_fields(out[0]), (POINTER_RIGHT, 5, -3, -1));
    assert_eq!(Pointer::boot().x, mouse.x);
}

#[test]
fn a_keyboard_descriptor_has_no_pointer() {
    // The boot keyboard's: modifiers, a reserved byte, LEDs (output), six keys.
    let keyboard = [0x05, 0x01, 0x09, 0x06, 0xa1, 0x01, 0x75, 0x01, 0x95, 0x08, 0x05, 0x07, 0x19, 0xe0, 0x29, 0xe7, 0x15, 0x00, 0x25, 0x01,
        0x81, 0x02, 0x95, 0x01, 0x75, 0x08, 0x81, 0x01, 0x95, 0x05, 0x75, 0x01, 0x05, 0x08, 0x19, 0x01, 0x29, 0x05, 0x91, 0x02, 0x95, 0x01,
        0x75, 0x03, 0x91, 0x01, 0x95, 0x06, 0x75, 0x08, 0x15, 0x00, 0x25, 0xff, 0x05, 0x07, 0x19, 0x00, 0x29, 0xff, 0x81, 0x00, 0xc0];
    assert!(Pointer::parse(&keyboard).is_none());
    assert!(Pointer::parse(&[0x05, 0x01, 0x81]).is_none(), "a truncated descriptor");
}

#[test]
fn report_ids_select_the_report() {
    // Report 2 carries the pointer; report 1 something else.
    let mut descriptor = vec![0x85, 0x01, 0x75, 0x08, 0x95, 0x02, 0x81, 0x01];
    descriptor.extend_from_slice(&[0x85, 0x02]);
    descriptor.extend_from_slice(&MOUSE[10..50]);
    let mouse = Pointer::parse(&descriptor).expect("layout");
    assert_eq!(mouse.id, 2);
    assert!(events(&mouse, &[1, 9, 9, 9]).is_empty());
    assert_eq!(pointer_fields(events(&mouse, &[2, 0, 1, 2, 0])[0]), (0, 1, 2, 0));
}

// 211-DRV-0018: Wellspring packets as the MacBook Pro's trackpad sends them in its multitouch mode: a 30-byte header with
// the button at byte 15, then 28 bytes a finger (X at 2, Y at 4, upwards; the touch's major axis at 16).
fn packet(button: bool, fingers: &[(i16, i16)]) -> Vec<u8> {
    let mut p = vec![0u8; 30];
    p[15] = button as u8;
    for &(x, y) in fingers {
        let mut f = [0u8; 28];
        f[2..4].copy_from_slice(&x.to_le_bytes()); f[4..6].copy_from_slice(&y.to_le_bytes()); f[16..18].copy_from_slice(&300u16.to_le_bytes());
        p.extend_from_slice(&f);
    }
    p
}
fn feed(pad: &mut Trackpad, button: bool, fingers: &[(i16, i16)], now: u64) -> Vec<(u8, i32, i32, i32, i32)> {
    let mut out = Vec::new();
    pad.feed(&packet(button, fingers), now, &mut |e| { let (b, dx, dy, w) = pointer_fields(e); out.push((b, dx, dy, w, pointer_across(e))); });
    out
}

#[test]
fn wellspring_trackpads_are_known() {
    assert!(wellspring(0x05AC, 0x0263) && wellspring(0x05AC, 0x0262) && wellspring(0x05AC, 0x0259));
    assert!(!wellspring(0x05AC, 0x0290) && !wellspring(0x046D, 0x0263));
    assert_eq!(WELLSPRING_LONGEST, 478);
}

#[test]
fn one_finger_moves_the_pointer_down_the_screen_as_y_falls() {
    let mut pad = Trackpad::new();
    assert!(feed(&mut pad, false, &[(0, 1000)], 0).is_empty()); // the first packet of a touch only places it
    let moved = feed(&mut pad, false, &[(24, 976)], 8); // slow: 8 units a pixel
    assert_eq!(moved, vec![(0, 3, 3, 0, 0)]);
    let faster = feed(&mut pad, false, &[(24 + 200, 976)], 16); // 200 units in a packet: three times as far
    assert_eq!(faster, vec![(0, 75, 0, 0, 0)]);
    assert!(feed(&mut pad, false, &[], 24).is_empty());
}

#[test]
fn pressing_with_one_finger_is_left_and_with_two_right() {
    let mut pad = Trackpad::new();
    feed(&mut pad, false, &[(0, 0)], 0);
    assert_eq!(feed(&mut pad, true, &[(0, 0)], 8), vec![(POINTER_LEFT, 0, 0, 0, 0)]);
    assert_eq!(feed(&mut pad, false, &[(0, 0)], 16), vec![(0, 0, 0, 0, 0)]);
    feed(&mut pad, false, &[], 24);
    feed(&mut pad, false, &[(0, 0), (900, 0)], 400);
    assert_eq!(feed(&mut pad, true, &[(0, 0), (900, 0)], 408), vec![(POINTER_RIGHT, 0, 0, 0, 0)]);
    // A finger lifted while the pad is held keeps the right button until it is up.
    assert!(feed(&mut pad, true, &[(0, 0)], 416).iter().all(|e| e.0 == POINTER_RIGHT));
    assert_eq!(feed(&mut pad, false, &[(0, 0)], 424), vec![(0, 0, 0, 0, 0)]);
    assert!(feed(&mut pad, false, &[], 432).is_empty()); // pressed during the touch: no tap
}

#[test]
fn a_quick_two_finger_tap_is_a_right_click() {
    let mut pad = Trackpad::new();
    feed(&mut pad, false, &[(0, 0), (900, 0)], 0);
    feed(&mut pad, false, &[(10, 5), (910, 5)], 80);
    assert_eq!(feed(&mut pad, false, &[], 160), vec![(POINTER_RIGHT, 0, 0, 0, 0), (0, 0, 0, 0, 0)]);
    // Slow or moving two-finger touches are not taps; nor is a one-finger tap.
    feed(&mut pad, false, &[(0, 0), (900, 0)], 1000);
    assert!(feed(&mut pad, false, &[], 1400).is_empty());
    feed(&mut pad, false, &[(0, 0), (900, 0)], 2000);
    feed(&mut pad, false, &[(300, 0), (1200, 0)], 2050);
    assert!(feed(&mut pad, false, &[], 2100).is_empty());
    feed(&mut pad, false, &[(0, 0)], 3000);
    assert!(feed(&mut pad, false, &[], 3050).is_empty());
}

#[test]
fn three_fingers_scroll_one_way_at_a_time() {
    let mut pad = Trackpad::new();
    let three = |y: i16, x: i16| [(x, y), (x + 800, y), (x + 1600, y)];
    feed(&mut pad, false, &three(1000, 0), 0);
    // Upwards (Y grows): the content follows the fingers, as the wheel turned towards the user (positive).
    let mut steps = 0;
    for k in 1..=8 { for e in feed(&mut pad, false, &three(1000 + 40 * k, 0), 8 * k as u64) { assert_eq!((e.0, e.1, e.2, e.4), (0, 0, 0, 0)); steps += e.3; } }
    assert_eq!(steps, 2); // 320 units, 160 a step
    // Once vertical, sideways movement in the same gesture does not scroll across.
    for e in feed(&mut pad, false, &three(1320, 400), 80) { assert_eq!(e.4, 0); }
    feed(&mut pad, false, &[], 100);
    // Leftwards: across, positive (the view goes right as the content follows the fingers).
    feed(&mut pad, false, &three(0, 0), 200);
    let mut across = 0;
    for k in 1..=4 { for e in feed(&mut pad, false, &three(0, -80 * k), 200 + 8 * k as u64) { assert_eq!(e.3, 0); across += e.4; } }
    assert_eq!(across, 2);
    assert!(feed(&mut pad, false, &[], 300).is_empty());
}

#[test]
fn short_or_empty_packets_do_nothing() {
    let mut pad = Trackpad::new();
    let mut out = Vec::new();
    // The mode switch's 2-byte answer and the 8-byte reports the MacBook Pro sent when its mode had not changed are no fingers.
    assert!(!pad.feed(&[0u8; 12], 0, &mut |e| out.push(e)));
    assert!(!pad.feed(&[0x60, 0x02], 0, &mut |e| out.push(e)));
    assert!(!pad.feed(&[0x02, 0x00, 0xFB, 0x00, 0x00, 0x00, 0xFD, 0x00], 0, &mut |e| out.push(e)));
    let mut torn = packet(false, &[(0, 0)]); torn.pop();
    assert!(!pad.feed(&torn, 0, &mut |e| out.push(e)));
    assert!(pad.feed(&packet(false, &[]), 0, &mut |e| out.push(e)));
    assert!(out.is_empty());
    assert!(fingers(&packet(false, &[(1, 2), (3, 4)])) && !fingers(&[0u8; 29]));
}

// A one-finger tap, then a touch: the left button held across touches until a later tap (the drag lock).
fn tap(pad: &mut Trackpad, at: u64) -> Vec<(u8, i32, i32, i32, i32)> {
    let mut out = feed(pad, false, &[(500, 500)], at);
    out.extend(feed(pad, false, &[], at + 80));
    out
}

#[test]
fn a_tap_then_a_touch_holds_the_left_button_until_a_tap() {
    let mut pad = Trackpad::new();
    assert!(tap(&mut pad, 0).is_empty()); // one tap alone does nothing
    // The next touch, 200 ms later, presses the left button at once and drags with it.
    assert_eq!(feed(&mut pad, false, &[(0, 0)], 280), vec![(POINTER_LEFT, 0, 0, 0, 0)]);
    assert!(feed(&mut pad, false, &[(80, 0)], 288).iter().all(|e| e.0 == POINTER_LEFT && e.1 > 0));
    // Lifted, the button stays down; a finger put down again goes on dragging.
    assert!(feed(&mut pad, false, &[], 300).is_empty());
    feed(&mut pad, false, &[(2000, 2000)], 1000);
    assert!(feed(&mut pad, false, &[(2000, 1900)], 1008).iter().all(|e| e.0 == POINTER_LEFT && e.2 > 0));
    assert!(feed(&mut pad, false, &[], 1300).is_empty());
    // A tap lets it go.
    assert_eq!(tap(&mut pad, 2000), vec![(0, 0, 0, 0, 0)]);
    feed(&mut pad, false, &[(0, 0)], 2100);
    assert!(feed(&mut pad, false, &[(80, 0)], 2108).iter().all(|e| e.0 == 0));
}

#[test]
fn a_double_tap_keeps_the_lock_and_a_press_ends_it() {
    let mut pad = Trackpad::new();
    tap(&mut pad, 0);
    // Two taps: the button goes down at the second and stays down after it.
    assert_eq!(tap(&mut pad, 200), vec![(POINTER_LEFT, 0, 0, 0, 0)]);
    feed(&mut pad, false, &[(0, 0)], 1000);
    assert!(feed(&mut pad, false, &[(0, 80)], 1008).iter().all(|e| e.0 == POINTER_LEFT));
    // A press of the pad takes the button over; it is up when the pad is, and the lock is gone.
    assert!(feed(&mut pad, true, &[(0, 80)], 1016).iter().all(|e| e.0 == POINTER_LEFT));
    assert_eq!(feed(&mut pad, false, &[(0, 80)], 1024), vec![(0, 0, 0, 0, 0)]);
    assert!(feed(&mut pad, false, &[(0, 160)], 1032).iter().all(|e| e.0 == 0));
    assert!(feed(&mut pad, false, &[], 1040).is_empty());
}

#[test]
fn a_touch_long_after_a_tap_or_with_two_fingers_does_not_lock() {
    let mut pad = Trackpad::new();
    tap(&mut pad, 0);
    feed(&mut pad, false, &[(0, 0)], 500); // 420 ms after the tap
    assert!(feed(&mut pad, false, &[(80, 0)], 508).iter().all(|e| e.0 == 0));
    feed(&mut pad, false, &[], 600);
    tap(&mut pad, 1000);
    // Two fingers after a tap: a two-finger tap, the right click, not a lock.
    feed(&mut pad, false, &[(0, 0), (900, 0)], 1150);
    assert_eq!(feed(&mut pad, false, &[], 1200), vec![(POINTER_RIGHT, 0, 0, 0, 0), (0, 0, 0, 0, 0)]);
}
