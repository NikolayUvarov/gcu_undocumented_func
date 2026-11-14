//! Host tests of USB keyboards' lock lights (000-DRV-0070, libmind/src/hid.rs and keys.rs): the decoder that chooses
//! the letters' case keeps Num, Caps and Scroll Lock, a press toggling each once (a held lock key does not repeat);
//! every keyboard is sent the boot output report when the locks change and when it is set up, so one plugged in after
//! Caps Lock was turned on lights it; Caps Lock as the layout switch lights nothing.
#![allow(dead_code)]
#[path = "../common/abi.rs"]
mod abi;
#[path = "../libmind/src/hid.rs"]
mod hid;
#[path = "../libmind/src/keys.rs"]
mod keys;

use abi::*;
use hid::{lights, Keyboard, LIGHTS_REQUEST, REPEAT_DELAY_MS, REPEAT_MS};
use keys::{Event, Locks, Ps2, Switch};

// usb_hid's loop: one decoder for every keyboard; after each round, each keyboard's report to send, if any.
struct Machine { decoder: Ps2, keyboards: Vec<Keyboard>, typed: String }

impl Machine {
    fn new(keyboards: usize) -> Self { Self { decoder: Ps2::new(), keyboards: (0..keyboards).map(|_| Keyboard::new()).collect(), typed: String::new() } }
    fn feed(&mut self, byte: u8) {
        if let Some(Event::Key(word)) = self.decoder.feed(byte) { if let Some(ch) = char::from_u32(event_char(word)).filter(|&c| c != '\0') { self.typed.push(ch); } }
    }
    // Keyboard `k`'s report at `now` (ms), then the reports each keyboard is sent.
    fn report(&mut self, k: usize, report: [u8; 8], now: u64) -> Vec<Option<u8>> {
        let mut bytes = Vec::new();
        self.keyboards[k].feed(&report, now, &mut |b| bytes.push(b));
        for b in bytes { self.feed(b); }
        self.lights()
    }
    // A key of keyboard `k` pressed and released: the reports sent on its press (its release sends none).
    fn tap(&mut self, k: usize, usage: u8, now: u64) -> Vec<Option<u8>> {
        let sent = self.report(k, [0, 0, usage, 0, 0, 0, 0, 0], now);
        assert!(self.report(k, [0; 8], now + 50).iter().all(Option::is_none), "a release changed a light");
        sent
    }
    fn lights(&mut self) -> Vec<Option<u8>> { let locks = self.decoder.locks(); self.keyboards.iter_mut().map(|k| k.show(locks)).collect() }
}

const CAPS: u8 = 0x39;
const NUM: u8 = 0x53;
const SCROLL: u8 = 0x47;
const KEYPAD_1: u8 = 0x59;

#[test]
fn the_report_is_the_led_pages_bits() {
    assert_eq!(lights(Locks::default()), 0);
    assert_eq!(lights(Locks { num: true, ..Default::default() }), 0x01);
    assert_eq!(lights(Locks { caps: true, ..Default::default() }), 0x02);
    assert_eq!(lights(Locks { scroll: true, ..Default::default() }), 0x04);
    assert_eq!(lights(Locks { num: true, caps: true, scroll: true }), 0x07);
    assert_eq!(LIGHTS_REQUEST, (0x21, 0x09, 0x0200)); // SET_REPORT to the interface, output report 0
}

#[test]
fn keyboards_set_up_are_sent_the_locks_once() {
    let mut m = Machine::new(2);
    assert_eq!(m.lights(), [Some(0), Some(0)], "all off at once, whatever the firmware left lit");
    assert_eq!(m.lights(), [None, None]);
    m.tap(0, 0x04, 0); // a key that is not a lock changes nothing
    assert_eq!(m.lights(), [None, None]);
}

#[test]
fn caps_lock_on_one_keyboard_lights_every_keyboard_and_the_letters_agree() {
    let mut m = Machine::new(2);
    m.lights();
    assert_eq!(m.report(1, [0, 0, CAPS, 0, 0, 0, 0, 0], 0), [Some(0x02), Some(0x02)], "on its press");
    assert_eq!(m.report(1, [0; 8], 80), [None, None], "its release toggles nothing");
    m.tap(0, 0x04, 100); // a, from the other keyboard
    m.report(0, [0x02, 0, 0x04, 0, 0, 0, 0, 0], 200); // Shift+a
    m.report(0, [0; 8], 250);
    assert_eq!(m.typed, "Aa");
    assert_eq!(m.tap(0, CAPS, 300), [Some(0), Some(0)]);
    m.tap(1, 0x04, 400);
    assert_eq!(m.typed, "Aaa");
}

#[test]
fn num_and_scroll_lock_light_their_own_bits() {
    let mut m = Machine::new(1);
    m.lights();
    m.tap(0, KEYPAD_1, 0);
    assert_eq!(m.typed, "", "keypad 1 is End with Num Lock off");
    assert_eq!(m.tap(0, NUM, 100), [Some(0x01)]);
    m.tap(0, KEYPAD_1, 200);
    assert_eq!(m.typed, "1");
    assert_eq!(m.tap(0, SCROLL, 300), [Some(0x05)]);
    assert_eq!(m.tap(0, CAPS, 400), [Some(0x07)]);
    assert_eq!(m.tap(0, NUM, 500), [Some(0x06)]);
    assert_eq!(m.tap(0, SCROLL, 600), [Some(0x02)]);
}

#[test]
fn a_keyboard_plugged_in_after_caps_lock_gets_it() {
    let mut m = Machine::new(1);
    m.lights();
    m.tap(0, CAPS, 0);
    m.tap(0, NUM, 100);
    assert_eq!(m.lights(), [None]);
    m.keyboards.push(Keyboard::new());
    assert_eq!(m.lights(), [None, Some(0x03)]);
    // The first one unplugged: the new one goes on with the same locks.
    m.keyboards.remove(0);
    m.tap(0, 0x05, 200);
    assert_eq!(m.typed, "B");
    assert_eq!(m.tap(0, CAPS, 300), [Some(0x01)]);
}

#[test]
fn a_held_lock_key_toggles_once() {
    let mut m = Machine::new(1);
    m.lights();
    assert_eq!(m.report(0, [0, 0, CAPS, 0, 0, 0, 0, 0], 1000), [Some(0x02)]);
    let mut bytes = Vec::new();
    for now in (1000..3000).step_by(REPEAT_MS as usize) { m.keyboards[0].repeat(now, &mut |b| bytes.push(b)); }
    assert!(bytes.is_empty(), "Caps Lock repeated: {bytes:02X?}");
    assert_eq!(m.report(0, [0; 8], 3000), [None]);
    // A held letter still repeats, and stops when a lock key is pressed after it, as on a PS/2 keyboard.
    m.report(0, [0, 0, 0x04, 0, 0, 0, 0, 0], 4000);
    bytes.clear();
    m.keyboards[0].repeat(4000 + REPEAT_DELAY_MS, &mut |b| bytes.push(b));
    assert_eq!(bytes, [0x1E]);
    assert_eq!(m.report(0, [0, 0, 0x04, NUM, 0, 0, 0, 0], 4600), [Some(0x03)]);
    bytes.clear();
    for now in (4600..6000).step_by(REPEAT_MS as usize) { m.keyboards[0].repeat(now, &mut |b| bytes.push(b)); }
    assert!(bytes.is_empty(), "{bytes:02X?}");
}

#[test]
fn caps_lock_as_the_layout_switch_lights_nothing() {
    let mut m = Machine::new(1);
    m.lights();
    m.tap(0, CAPS, 0);
    m.decoder.set_switch(Switch::CapsLock); // the shell's keymap --switch caps: the lock already on is released
    assert_eq!(m.lights(), [Some(0)]);
    assert_eq!(m.tap(0, CAPS, 100), [None], "it switched the layout");
    m.tap(0, 0x04, 200);
    assert_eq!(m.typed, "ф");
}

#[test]
fn pause_and_break_toggle_no_lock() {
    // Pause is E1 1D 45 E1 9D C5, Ctrl+Break E0 46 E0 C6: neither is Num Lock (45) or Scroll Lock (46).
    let mut decoder = Ps2::new();
    for b in [0xE1, 0x1D, 0x45, 0xE1, 0x9D, 0xC5, 0x1D, 0xE0, 0x46, 0xE0, 0xC6, 0x9D] { decoder.feed(b); }
    assert_eq!(decoder.locks(), Locks::default());
    for b in [0x45, 0xC5, 0x46, 0xC6] { decoder.feed(b); }
    assert_eq!(decoder.locks(), Locks { num: true, caps: false, scroll: true });
}
