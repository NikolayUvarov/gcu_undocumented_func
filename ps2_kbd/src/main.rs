#![no_std]
#![no_main]
// LEGACY: the whole driver: the PS/2 keyboard controller on ISA ports (docs/legacy.md).
// Ring 3 PS/2 keyboard driver: IRQ1 -> scan codes -> key events (mind::keys: modifiers, US/Russian layout) for the
// focused task. Ctrl+Z is the attention key; Ctrl+Shift or Alt+Shift switches the layout. The layout and the switch
// can be changed through idl/keyboard.wit (the shell's `keymap`): IRQ1 arrives as a message on the service endpoint,
// between the requests.
use mind::abi::{BootInfo, SLOT_DEV0, SLOT_DEV1, SLOT_IRQ};
use mind::dev::{input_key, Irq, Ports};
use mind::idl::codec::Text;
use mind::idl::keyboard::{self, Request, State, SwitchKey};
use mind::idl::wire;
use mind::ipc::Endpoint;
use mind::keys::{Event, Layout, Ps2, Switch};

const RECEIVED_CAP: usize = 9;

// Drains all bytes from the controller buffer: no new IRQs arrive while the line is masked.
fn drain(data: &Ports, status: &Ports, decoder: &mut Ps2) {
    while status.in8(0x64) & 1 != 0 {
        let aux = status.in8(0x64) & 0x20 != 0;
        let scancode = data.in8(0x60);
        if aux { continue; }
        match decoder.feed(scancode) {
            Some(Event::Key(event)) => { let _ = input_key(event, event, false); }
            Some(Event::Attention) => { let _ = input_key(0, 0, true); }
            Some(Event::Layout(layout)) => mind::println!("[KBD] LAYOUT {}", name(layout)),
            None => {}
        }
    }
}

fn name(layout: Layout) -> &'static str { if layout == Layout::Ru { "RU" } else { "EN" } }

fn state(decoder: &Ps2) -> State {
    let layout = if decoder.layout() == Layout::Ru { keyboard::Layout::Ru } else { keyboard::Layout::Us };
    let switch_key = match decoder.switch() {
        Switch::CtrlOrAltShift => SwitchKey::CtrlOrAltShift, Switch::CtrlShift => SwitchKey::CtrlShift, Switch::AltShift => SwitchKey::AltShift,
        Switch::CapsLock => SwitchKey::CapsLock, Switch::None => SwitchKey::None,
    };
    State { layout, switch_key }
}

fn serve(decoder: &mut Ps2, request: Request, call: wire::Call) {
    let _ = match request {
        Request::State => keyboard::reply_state(call, &state(decoder)),
        Request::SetLayout { layout } => {
            decoder.set_layout(if layout == keyboard::Layout::Ru { Layout::Ru } else { Layout::Us });
            mind::println!("[KBD] LAYOUT {} (SET)", name(decoder.layout()));
            keyboard::reply_set_layout(call, Ok(()))
        }
        Request::SetSwitch { key } => {
            decoder.set_switch(match key {
                SwitchKey::CtrlOrAltShift => Switch::CtrlOrAltShift, SwitchKey::CtrlShift => Switch::CtrlShift, SwitchKey::AltShift => Switch::AltShift,
                SwitchKey::CapsLock => Switch::CapsLock, SwitchKey::None => Switch::None,
            });
            mind::println!("[KBD] SWITCH {:?}", decoder.switch());
            keyboard::reply_set_switch(call, Ok(()))
        }
        Request::Layouts => keyboard::reply_layouts(call, &[Text::new("us").unwrap_or_default(), Text::new("ru").unwrap_or_default()]),
    };
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let (data, status, irq) = (Ports(SLOT_DEV0), Ports(SLOT_DEV1), Irq(SLOT_IRQ));
    let mut decoder = Ps2::new();
    if irq.bind(Endpoint::SERVICE).is_err() {
        // No service endpoint (an older init): keys only.
        loop {
            if irq.wait().is_err() { mind::process::exit(); }
            drain(&data, &status, &mut decoder);
        }
    }
    loop {
        let Ok(request) = Endpoint::SERVICE.recv(RECEIVED_CAP) else { continue };
        if request.irq.is_some() {
            drain(&data, &status, &mut decoder);
            let _ = irq.ack();
            continue;
        }
        match keyboard::decode(&request, RECEIVED_CAP) {
            Ok((request, call)) => serve(&mut decoder, request, call),
            Err(reason) => if request.is_call { let _ = wire::reject(reason); },
        }
    }
}
