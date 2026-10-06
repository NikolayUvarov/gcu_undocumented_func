#![no_std]
#![no_main]
// LEGACY: the whole driver: the PS/2 keyboard controller on ISA ports (docs/legacy.md).
// Ring 3 PS/2 keyboard driver: IRQ1 -> scan codes -> key events (mind::keys: modifiers, US/Russian layout) for the
// focused task. Ctrl+Z is the attention key; Ctrl+Shift or Alt+Shift switches the layout. The layout and the switch
// can be changed through idl/keyboard.wit (the shell's `keymap`): IRQ1 arrives as a message on the service endpoint,
// between the requests. The mouse on the auxiliary port (mouse.rs, issue 156) sends pointer events the same way; its
// IRQ 12 arrives on the same endpoint.
mod mouse;

use mind::abi::{BootInfo, SLOT_DEV0, SLOT_DEV1, SLOT_IRQ};
use mind::dev::{input_key, Irq, Ports};
use mind::idl::{keyboard, wire};
use mind::ipc::Endpoint;
use mind::keys::Ps2;

const RECEIVED_CAP: usize = 9;
const SLOT_IRQ_AUX: usize = 5; // IRQ 12 of the auxiliary port (the mouse)

// Drains all bytes from the controller buffer: no new IRQs arrive while the line is masked.
fn drain(data: &Ports, status: &Ports, decoder: &mut Ps2, mouse: &mut Option<mouse::Mouse>) {
    while status.in8(0x64) & 1 != 0 {
        let aux = status.in8(0x64) & 0x20 != 0;
        let scancode = data.in8(0x60);
        if aux {
            // Pointer events go to the focused task like keys (only to one that asked for them, INPUT_POINTER).
            if let Some(event) = mouse.as_mut().and_then(|m| m.feed(scancode)) { let _ = input_key(event, event, false); }
            continue;
        }
        if let Some(event) = decoder.feed(scancode) { mind::keyboard::deliver("KBD", decoder, event); }
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let (data, status, irq) = (Ports(SLOT_DEV0), Ports(SLOT_DEV1), Irq(SLOT_IRQ));
    let mut decoder = Ps2::new();
    let aux = Irq(SLOT_IRQ_AUX);
    let mut mouse = mouse::Mouse::setup(&data, &status);
    match &mouse {
        Some(m) => mind::println!("[KBD] MOUSE ON THE AUXILIARY PORT{}", if m.wheel() { " WITH A WHEEL" } else { "" }),
        None => mind::println!("[KBD] NO MOUSE"),
    }
    if irq.bind(Endpoint::SERVICE).is_err() {
        // No service endpoint (an older init): keys only.
        loop {
            if irq.wait().is_err() { mind::process::exit(); }
            drain(&data, &status, &mut decoder, &mut None);
        }
    }
    if mouse.is_some() && aux.bind(Endpoint::SERVICE).is_err() { mind::println!("[KBD] NO IRQ 12: MOUSE OFF"); mouse = None; }
    loop {
        let Ok(request) = Endpoint::SERVICE.recv(RECEIVED_CAP) else { continue };
        if let Some(line) = request.irq {
            drain(&data, &status, &mut decoder, &mut mouse);
            let _ = if line == 12 { aux.ack() } else { irq.ack() };
            continue;
        }
        match keyboard::decode(&request, RECEIVED_CAP) {
            Ok((request, call)) => mind::keyboard::serve("KBD", &mut decoder, request, call),
            Err(reason) => if request.is_call { let _ = wire::reject(reason); },
        }
    }
}
