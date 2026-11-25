//! The classic HID host (HID Profile 1.1): a keyboard's or mouse's control channel (PSM 0x0011) and interrupt channel
//! (0x0013), the boot protocol set on each connection, and its input reports handed up as `usb_hid` takes a USB
//! device's: a boot keyboard's 8 bytes, a boot mouse's buttons, X, Y and wheel.
use crate::ctl::Ctl;
use crate::l2cap::{self, Channels};
use crate::stack::{Ups, UP_KEYBOARD, UP_POINTER};

/// HIDP message types (the header's high four bits) and parameters.
pub const HANDSHAKE: u8 = 0x0;
pub const CONTROL: u8 = 0x1;
pub const SET_PROTOCOL: u8 = 0x7;
pub const DATA: u8 = 0xA;
pub const INPUT: u8 = 0x1;
pub const OUTPUT: u8 = 0x2;
pub const BOOT: u8 = 0;
pub const VIRTUAL_CABLE_UNPLUG: u8 = 0x5;
/// Boot protocol report IDs.
pub const KEYBOARD: u8 = 1;
pub const MOUSE: u8 = 2;
pub const HOSTS: usize = 2;
/// How long the device may take to answer Set Protocol.
pub const ANSWER_MS: u64 = 3_000;

#[derive(Clone, Copy, Debug)]
pub struct Device { pub handle: u16, pub control: Option<u16>, pub interrupt: Option<u16>, pub boot: bool, pub asked: Option<u64>, pub keys: bool, pub reports: u32 }

pub struct Hosts { pub list: [Option<Device>; HOSTS] }

impl Default for Hosts { fn default() -> Self { Self::new() } }

impl Hosts {
    pub const fn new() -> Self { Self { list: [None; HOSTS] } }

    fn at(&mut self, handle: u16) -> Option<&mut Device> { self.list.iter_mut().flatten().find(|d| d.handle == handle) }

    fn add(&mut self, handle: u16) -> Option<&mut Device> {
        if self.at(handle).is_none() {
            let slot = self.list.iter_mut().find(|d| d.is_none())?;
            *slot = Some(Device { handle, control: None, interrupt: None, boot: false, asked: None, keys: false, reports: 0 });
        }
        self.at(handle)
    }

    /// A secure link to a HID device that opened nothing: our control channel first.
    pub fn start(&mut self, ctl: &mut Ctl, chans: &mut Channels, handle: u16) {
        if chans.find(handle, l2cap::PSM_HID_CONTROL).is_some() || self.add(handle).is_none() { return; }
        chans.connect(ctl, handle, l2cap::PSM_HID_CONTROL);
    }

    pub fn opened(&mut self, ctl: &mut Ctl, chans: &mut Channels, handle: u16, local: u16, psm: u16, initiator: bool) {
        let now = ctl.now;
        let Some(d) = self.add(handle) else { chans.disconnect(ctl, handle, local); return };
        if psm == l2cap::PSM_HID_CONTROL {
            d.control = Some(local);
            // Boot protocol on every connection: the reports have the layout usb_hid reads.
            d.asked = Some(now);
            chans.send(ctl, handle, local, &[&[SET_PROTOCOL << 4 | BOOT]]);
            if initiator && d.interrupt.is_none() { chans.connect(ctl, handle, l2cap::PSM_HID_INTERRUPT); }
        } else {
            d.interrupt = Some(local);
            ctl.say(format_args!("HID {:03X}: READY", handle));
        }
    }

    pub fn refused(&mut self, ctl: &mut Ctl, chans: &mut Channels, handle: u16) {
        let Some(d) = self.at(handle) else { return };
        if let Some(c) = d.control { chans.disconnect(ctl, handle, c); }
        ctl.say(format_args!("HID {:03X}: ITS CHANNELS NOT OPENED", handle));
        self.drop(handle);
    }

    fn drop(&mut self, handle: u16) { for s in self.list.iter_mut() { if s.is_some_and(|d| d.handle == handle) { *s = None; } } }

    /// Held keys let go when a keyboard goes.
    fn release(ups: &mut Ups, d: &Device) { if d.keys { ups.push(UP_KEYBOARD, &[0; 8]); } }

    pub fn closed(&mut self, ctl: &mut Ctl, chans: &mut Channels, ups: &mut Ups, handle: u16, local: u16) {
        let Some(d) = self.at(handle) else { return };
        if d.interrupt == Some(local) { d.interrupt = None; }
        if d.control == Some(local) { d.control = None; }
        if d.control.is_none() && d.interrupt.is_none() {
            let d = *d;
            Self::release(ups, &d);
            ctl.say(format_args!("HID {:03X}: CLOSED AFTER {} REPORTS", handle, d.reports));
            self.drop(handle);
        } else if let (Some(c), None) = (d.control, d.interrupt) {
            // The interrupt channel gone alone: the control channel follows (HID 1.1, 5.2.2).
            chans.disconnect(ctl, handle, c);
        }
    }

    pub fn link_down(&mut self, handle: u16, ups: &mut Ups) {
        if let Some(d) = self.at(handle).copied() { Self::release(ups, &d); }
        self.drop(handle);
    }

    pub fn data(&mut self, ctl: &mut Ctl, chans: &mut Channels, ups: &mut Ups, handle: u16, local: u16, p: &[u8]) {
        let Some(d) = self.at(handle) else { return };
        let Some(&head) = p.first() else { return };
        let (kind, param) = (head >> 4, head & 0x0F);
        if d.control == Some(local) {
            match kind {
                HANDSHAKE if d.asked.is_some() => {
                    d.asked = None;
                    d.boot = param == 0;
                    ctl.say(format_args!("HID {:03X}: BOOT PROTOCOL {}", handle, if param == 0 { "SET" } else { "REFUSED (ITS REPORTS AS THEY ARE)" }));
                }
                CONTROL if param == VIRTUAL_CABLE_UNPLUG => {
                    ctl.say(format_args!("HID {:03X}: THE DEVICE UNPLUGGED ITS VIRTUAL CABLE", handle));
                    if let Some(c) = d.control { chans.disconnect(ctl, handle, c); }
                }
                _ => {}
            }
            return;
        }
        if d.interrupt != Some(local) || kind != DATA || param != INPUT || p.len() < 2 { return; }
        d.reports += 1;
        let report = &p[1..];
        match report[0] {
            KEYBOARD if report.len() >= 9 => { d.keys = true; ups.push(UP_KEYBOARD, &report[1..9]); }
            MOUSE if report.len() >= 4 => { let n = report.len().min(8); ups.push(UP_POINTER, &report[1..n]); }
            _ => {}
        }
        if d.reports == 1 { ctl.say(format_args!("HID {:03X}: FIRST REPORT {:02X?}", handle, &report[..report.len().min(10)])); }
    }

    /// A keyboard's lights (the output report's byte: Num, Caps, Scroll Lock bits) on its interrupt channel.
    pub fn lights(&self, ctl: &mut Ctl, chans: &Channels, report: u8) {
        for d in self.list.iter().flatten().filter(|d| d.keys) {
            if let Some(i) = d.interrupt { chans.send(ctl, d.handle, i, &[&[DATA << 4 | OUTPUT, KEYBOARD, report]]); }
        }
    }

    /// Set Protocol unanswered: the reports are taken as they come.
    pub fn tick(&mut self, ctl: &mut Ctl, _chans: &mut Channels) {
        for d in self.list.iter_mut().flatten() {
            if d.asked.is_some_and(|at| ctl.now > at + ANSWER_MS) { d.asked = None; ctl.say(format_args!("HID {:03X}: SET PROTOCOL NOT ANSWERED", d.handle)); }
        }
    }
}
