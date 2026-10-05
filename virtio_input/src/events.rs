//! What a VirtIO input device reports, turned into pointer events (issue 161): Linux input events (`type`, `code`,
//! `value`) gathered until each `SYN_REPORT`, then one event word — absolute for a tablet (the position as a share of
//! the screen, `POINTER_SCALE` steps), relative for a mouse. No system calls: tests/virtio_input_host.rs.
use crate::abi::{pointer_absolute, pointer_event, POINTER_LEFT, POINTER_MIDDLE, POINTER_RIGHT, POINTER_SCALE};

pub const EV_SYN: u16 = 0; pub const EV_KEY: u16 = 1; pub const EV_REL: u16 = 2; pub const EV_ABS: u16 = 3;
pub const SYN_REPORT: u16 = 0;
pub const ABS_X: u16 = 0; pub const ABS_Y: u16 = 1;
pub const REL_X: u16 = 0; pub const REL_Y: u16 = 1; pub const REL_WHEEL: u16 = 8;
pub const BTN_LEFT: u16 = 0x110; pub const BTN_RIGHT: u16 = 0x111; pub const BTN_MIDDLE: u16 = 0x112;
// QEMU's tablet without a wheel axis reports the wheel as these buttons.
pub const BTN_GEAR_DOWN: u16 = 0x150; pub const BTN_GEAR_UP: u16 = 0x151;

/// The range of an absolute axis (`VIRTIO_INPUT_CFG_ABS_INFO`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Axis { pub min: i32, pub max: i32 }

impl Axis {
    /// `value` as a share of the axis in `POINTER_SCALE` steps.
    pub fn scale(self, value: i32) -> usize {
        let span = (self.max as i64 - self.min as i64).max(1);
        ((value as i64 - self.min as i64).clamp(0, span) * (POINTER_SCALE as i64 - 1) / span) as usize
    }
}

/// The state between reports.
pub struct Pointer {
    pub axes: [Axis; 2],
    x: usize, y: usize, buttons: u8,
    dx: i32, dy: i32, wheel: i32, moved: bool, absolute: bool,
}

impl Pointer {
    pub fn new(axes: [Axis; 2]) -> Self { Self { axes, x: POINTER_SCALE / 2, y: POINTER_SCALE / 2, buttons: 0, dx: 0, dy: 0, wheel: 0, moved: false, absolute: false } }

    /// One input event; at a `SYN_REPORT`, the pointer events it completes (a large relative movement takes several).
    pub fn feed(&mut self, kind: u16, code: u16, value: i32, out: &mut impl FnMut(usize)) {
        match (kind, code) {
            (EV_ABS, ABS_X) => { self.x = self.axes[0].scale(value); self.moved = true; self.absolute = true; }
            (EV_ABS, ABS_Y) => { self.y = self.axes[1].scale(value); self.moved = true; self.absolute = true; }
            (EV_REL, REL_X) => { self.dx += value; self.moved = true; }
            (EV_REL, REL_Y) => { self.dy += value; self.moved = true; }
            // Linux counts the wheel away from the user as positive; pointer events the other way.
            (EV_REL, REL_WHEEL) => { self.wheel -= value; self.moved = true; }
            (EV_KEY, BTN_GEAR_UP) if value != 0 => { self.wheel -= 1; self.moved = true; }
            (EV_KEY, BTN_GEAR_DOWN) if value != 0 => { self.wheel += 1; self.moved = true; }
            (EV_KEY, BTN_LEFT | BTN_RIGHT | BTN_MIDDLE) => {
                let bit = match code { BTN_LEFT => POINTER_LEFT, BTN_RIGHT => POINTER_RIGHT, _ => POINTER_MIDDLE };
                let buttons = if value != 0 { self.buttons | bit } else { self.buttons & !bit };
                if buttons != self.buttons { self.buttons = buttons; self.moved = true; }
            }
            (EV_SYN, SYN_REPORT) => self.report(out),
            _ => {}
        }
    }

    fn report(&mut self, out: &mut impl FnMut(usize)) {
        if !core::mem::take(&mut self.moved) { return; }
        let wheel = core::mem::take(&mut self.wheel);
        if self.absolute { out(pointer_absolute(self.buttons, self.x, self.y, wheel)); self.dx = 0; self.dy = 0; return; }
        // Relative: in steps the event word holds.
        let (mut dx, mut dy, mut wheel) = (core::mem::take(&mut self.dx), core::mem::take(&mut self.dy), wheel);
        loop {
            let (sx, sy, sw) = (dx.clamp(-256, 255), dy.clamp(-256, 255), wheel.clamp(-8, 7));
            out(pointer_event(self.buttons, sx, sy, sw));
            (dx, dy, wheel) = (dx - sx, dy - sy, wheel - sw);
            if dx == 0 && dy == 0 && wheel == 0 { break; }
        }
    }
}
