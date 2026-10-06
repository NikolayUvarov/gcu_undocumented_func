//! The keyboard service (idl/keyboard.wit, the shell's `keymap`) over a scan-code decoder, shared by the keyboard
//! drivers (ps2_kbd, virtio_input). `tag` names the driver in its log lines.
use crate::idl::codec::Text;
use crate::idl::keyboard::{self, Request, State, SwitchKey};
use crate::idl::wire;
use crate::keys::{Layout, Ps2, Switch};

pub fn layout_name(layout: Layout) -> &'static str { if layout == Layout::Ru { "RU" } else { "EN" } }

pub fn state(decoder: &Ps2) -> State {
    let layout = if decoder.layout() == Layout::Ru { keyboard::Layout::Ru } else { keyboard::Layout::Us };
    let switch_key = match decoder.switch() {
        Switch::CtrlOrAltShift => SwitchKey::CtrlOrAltShift, Switch::CtrlShift => SwitchKey::CtrlShift, Switch::AltShift => SwitchKey::AltShift,
        Switch::CapsLock => SwitchKey::CapsLock, Switch::None => SwitchKey::None,
    };
    State { layout, switch_key }
}

/// Answers one request.
pub fn serve(tag: &str, decoder: &mut Ps2, request: Request, call: wire::Call) {
    let _ = match request {
        Request::State => keyboard::reply_state(call, &state(decoder)),
        Request::SetLayout { layout } => {
            decoder.set_layout(if layout == keyboard::Layout::Ru { Layout::Ru } else { Layout::Us });
            crate::println!("[{}] LAYOUT {} (SET)", tag, layout_name(decoder.layout()));
            keyboard::reply_set_layout(call, Ok(()))
        }
        Request::SetSwitch { key } => {
            decoder.set_switch(match key {
                SwitchKey::CtrlOrAltShift => Switch::CtrlOrAltShift, SwitchKey::CtrlShift => Switch::CtrlShift, SwitchKey::AltShift => Switch::AltShift,
                SwitchKey::CapsLock => Switch::CapsLock, SwitchKey::None => Switch::None,
            });
            crate::println!("[{}] SWITCH {:?}", tag, decoder.switch());
            keyboard::reply_set_switch(call, Ok(()))
        }
        Request::Layouts => keyboard::reply_layouts(call, &[Text::new("us").unwrap_or_default(), Text::new("ru").unwrap_or_default()]),
    };
}

/// Sends a decoded event to the focused task (keys, the attention key, the modifiers after a layout switch).
pub fn deliver(tag: &str, decoder: &Ps2, event: crate::keys::Event) {
    use crate::dev::input_key;
    match event {
        crate::keys::Event::Key(word) => { let _ = input_key(word, word, false); }
        crate::keys::Event::Attention => { let _ = input_key(0, 0, true); }
        crate::keys::Event::Layout(layout) => {
            crate::println!("[{}] LAYOUT {}", tag, layout_name(layout));
            // The switch took a modifier's release: programs still learn which modifiers are held.
            if let crate::keys::Event::Key(word) = decoder.modifiers() { let _ = input_key(word, word, false); }
        }
    }
}
