#![no_std]
#![no_main]
// Ring 3 PS/2 keyboard driver: IRQ1 -> scan codes -> key events (mind::keys: modifiers, US/Russian layout) for the
// focused task. Ctrl+Z is the attention key; Ctrl+Shift or Alt+Shift switches the layout.
use mind::abi::{BootInfo, SLOT_DEV0, SLOT_DEV1, SLOT_IRQ};
use mind::dev::{input_key, Irq, Ports};
use mind::keys::{Event, Layout, Ps2};

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let (data, status, irq) = (Ports(SLOT_DEV0), Ports(SLOT_DEV1), Irq(SLOT_IRQ));
    let mut decoder = Ps2::new();
    loop {
        if irq.wait().is_err() { mind::process::exit(); }
        // Drain all bytes from the controller buffer: no new IRQs arrive while the line is masked.
        while status.in8(0x64) & 1 != 0 {
            let aux = status.in8(0x64) & 0x20 != 0;
            let scancode = data.in8(0x60);
            if aux { continue; }
            match decoder.feed(scancode) {
                Some(Event::Key(event)) => { let _ = input_key(event, event, false); }
                Some(Event::Attention) => { let _ = input_key(0, 0, true); }
                Some(Event::Layout(layout)) => mind::println!("[KBD] LAYOUT {}", if layout == Layout::Ru { "RU" } else { "EN" }),
                None => {}
            }
        }
    }
}
