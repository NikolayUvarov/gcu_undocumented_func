#![no_std]
#![no_main]
// Ring 3 PS/2 keyboard driver: IRQ1 -> scan code -> input event to the kernel.
use mind::abi::{BootInfo, SLOT_DEV0, SLOT_DEV1, SLOT_IRQ};
use mind::dev::{input_event, Irq, Ports};

struct KbdState { shift: bool, control: bool, extended: bool }

impl KbdState {
    const fn new() -> Self { Self { shift: false, control: false, extended: false } }

    fn process(&mut self, code: u8) -> (u8, u8, bool) {
        if code == 0xe0 || code == 0xe1 { self.extended = true; return (0, 0, false); }
        if code & 0x7f == 0x1d { self.control = code & 0x80 == 0; self.extended = false; return (0, 0, false); }
        if self.extended { self.extended = false; return (0, 0, false); }
        if code & 0x7f == 0x2a || code & 0x7f == 0x36 { self.shift = code & 0x80 == 0; return (0, 0, false); }
        if code & 0x80 != 0 { return (0, 0, false); }
        if self.control && code == 0x2c { return (0, 0, true); } // Ctrl+Z
        const NORMAL: &[u8] = b"\0\x1b1234567890-=\x08\tqwertyuiop[]\n\0asdfghjkl;'`\0\\zxcvbnm,./\0*\0 ";
        const SHIFT: &[u8] = b"\0\x1b!@#$%^&*()_+\x08\tQWERTYUIOP{}\n\0ASDFGHJKL:\"~\0|ZXCVBNM<>?\0*\0 ";
        let shell = if self.shift { SHIFT } else { NORMAL }.get(code as usize).copied().unwrap_or(0);
        (code, shell, false)
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let (data, status, irq) = (Ports(SLOT_DEV0), Ports(SLOT_DEV1), Irq(SLOT_IRQ));
    let mut state = KbdState::new();
    loop {
        if irq.wait().is_err() { mind::process::exit(); }
        // Drain all bytes from the controller buffer: no new IRQs arrive while the line is masked.
        while status.in8(0x64) & 1 != 0 {
            let aux = status.in8(0x64) & 0x20 != 0;
            let scancode = data.in8(0x60);
            if aux { continue; }
            let (app, shell, background) = state.process(scancode);
            if app != 0 || shell != 0 || background { let _ = input_event(app, shell, background); }
        }
    }
}
