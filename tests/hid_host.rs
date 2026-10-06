//! Host tests of the USB HID decoding (libmind/src/hid.rs, issue 164): boot keyboard reports become PS/2 set 1 scan
//! codes with releases, modifiers, E0 keys and host-side repeat; report descriptors of a tablet and a mouse (QEMU's
//! usb-tablet and usb-mouse) give the fields their reports are read with.
#![allow(dead_code)]
#[path = "../common/abi.rs"]
mod abi;
#[path = "../libmind/src/hid.rs"]
mod hid;

use abi::{pointer_absolute_fields, pointer_fields, POINTER_LEFT, POINTER_RIGHT, POINTER_SCALE};
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
