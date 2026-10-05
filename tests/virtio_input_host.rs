//! Host tests of the VirtIO input driver's decoding (virtio_input/src/events.rs, issue 161): a tablet's reports become
//! absolute pointer events with the position as a share of the screen, a mouse's relative ones.
#![allow(dead_code)]
#[path = "../common/abi.rs"]
mod abi;
#[path = "../virtio_input/src/events.rs"]
mod events;

use abi::{pointer_absolute_fields, pointer_fields, POINTER_LEFT, POINTER_RIGHT, POINTER_SCALE};
use events::*;

fn feed(pointer: &mut Pointer, reports: &[(u16, u16, i32)]) -> Vec<usize> {
    let mut out = Vec::new();
    for &(kind, code, value) in reports { pointer.feed(kind, code, value, &mut |event| out.push(event)); }
    out
}

#[test]
fn a_tablet_gives_positions() {
    let axis = Axis { min: 0, max: 32767 };
    assert_eq!((axis.scale(0), axis.scale(32767), axis.scale(16384), axis.scale(-5), axis.scale(40000)), (0, POINTER_SCALE - 1, 2047, 0, POINTER_SCALE - 1));
    let mut tablet = Pointer::new([axis, Axis { min: 100, max: 1100 }]);
    // One report: one event, after SYN_REPORT.
    let out = feed(&mut tablet, &[(EV_ABS, ABS_X, 32767), (EV_ABS, ABS_Y, 600)]);
    assert!(out.is_empty());
    let out = feed(&mut tablet, &[(EV_SYN, SYN_REPORT, 0)]);
    assert_eq!(out.iter().map(|&e| pointer_absolute_fields(e)).collect::<Vec<_>>(), [Some((0, POINTER_SCALE - 1, 2047, 0))]);
    // A button: the position stays; an empty report sends nothing.
    let out = feed(&mut tablet, &[(EV_KEY, BTN_LEFT, 1), (EV_SYN, SYN_REPORT, 0), (EV_SYN, SYN_REPORT, 0), (EV_KEY, BTN_RIGHT, 1), (EV_KEY, BTN_LEFT, 0), (EV_SYN, SYN_REPORT, 0)]);
    assert_eq!(out.iter().map(|&e| pointer_absolute_fields(e).unwrap().0).collect::<Vec<_>>(), [POINTER_LEFT, POINTER_RIGHT]);
    assert_eq!(pointer_absolute_fields(out[0]).unwrap().1, POINTER_SCALE - 1);
    // The wheel: Linux's away-from-the-user is positive, a pointer event's negative; QEMU's gear buttons too.
    let out = feed(&mut tablet, &[(EV_REL, REL_WHEEL, 1), (EV_SYN, SYN_REPORT, 0), (EV_KEY, BTN_GEAR_DOWN, 1), (EV_SYN, SYN_REPORT, 0), (EV_KEY, BTN_GEAR_DOWN, 0), (EV_SYN, SYN_REPORT, 0)]);
    assert_eq!(out.iter().map(|&e| pointer_absolute_fields(e).unwrap().3).collect::<Vec<_>>(), [-1, 1]);
}

#[test]
fn a_mouse_gives_movements() {
    let mut mouse = Pointer::new([Axis { min: 0, max: 32767 }; 2]);
    let out = feed(&mut mouse, &[(EV_REL, REL_X, 10), (EV_REL, REL_Y, -4), (EV_SYN, SYN_REPORT, 0)]);
    assert_eq!(out.iter().map(|&e| pointer_fields(e)).collect::<Vec<_>>(), [(0, 10, -4, 0)]);
    assert!(pointer_absolute_fields(out[0]).is_none());
    // A large movement takes several events.
    let out = feed(&mut mouse, &[(EV_REL, REL_X, 600), (EV_KEY, BTN_LEFT, 1), (EV_SYN, SYN_REPORT, 0)]);
    assert_eq!(out.iter().map(|&e| pointer_fields(e)).collect::<Vec<_>>(), [(POINTER_LEFT, 255, 0, 0), (POINTER_LEFT, 255, 0, 0), (POINTER_LEFT, 90, 0, 0)]);
}
