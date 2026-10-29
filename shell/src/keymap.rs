//! `keymap [us|ru] [--switch both|ctrl-shift|alt-shift|caps|none]`: the PS/2 keyboard's layout and layout switch,
//! through the shell's client of `ps2_kbd` in SLOT_KEYBOARD (idl/keyboard.wit, issue 085). Without arguments it shows
//! them.
use core::fmt::Write;
use mind::abi::SLOT_KEYBOARD;
use mind::idl::keyboard::{self, Layout, SwitchKey};
use mind::idl::shell as commands;
use mind::ipc::Endpoint;

const KEYBOARD: Endpoint = Endpoint(SLOT_KEYBOARD);
const USAGE: &str = "USAGE: KEYMAP [US|RU] [--SWITCH BOTH|CTRL-SHIFT|ALT-SHIFT|CAPS|NONE]";

fn switch_name(key: SwitchKey) -> &'static str {
    match key { SwitchKey::CtrlOrAltShift => "CTRL+SHIFT OR ALT+SHIFT", SwitchKey::CtrlShift => "CTRL+SHIFT", SwitchKey::AltShift => "ALT+SHIFT", SwitchKey::CapsLock => "CAPS LOCK", SwitchKey::None => "NONE" }
}

/// What `args` asks for: a layout and a switch, each optional; None for words it does not know.
pub fn parse(args: &str) -> Option<(Option<Layout>, Option<SwitchKey>)> {
    let (mut layout, mut switch) = (None, None);
    let mut words = args.split_whitespace();
    let any = |word: &str, names: &[&str]| names.iter().any(|n| word.eq_ignore_ascii_case(n));
    while let Some(word) = words.next() {
        if any(word, &["us", "en"]) { layout = Some(Layout::Us); }
        else if any(word, &["ru"]) { layout = Some(Layout::Ru); }
        else if any(word, &["--switch", "-s"]) {
            let key = words.next()?;
            switch = Some(if any(key, &["both"]) { SwitchKey::CtrlOrAltShift } else if any(key, &["ctrl-shift"]) { SwitchKey::CtrlShift }
                          else if any(key, &["alt-shift"]) { SwitchKey::AltShift } else if any(key, &["caps", "caps-lock"]) { SwitchKey::CapsLock }
                          else if any(key, &["none"]) { SwitchKey::None } else { return None });
        } else { return None; }
    }
    Some((layout, switch))
}

pub fn command(out: &mut impl Write, args: &[u8]) {
    let Some((layout, switch)) = core::str::from_utf8(args).ok().and_then(parse) else { let _ = writeln!(out, "{}", USAGE); return };
    let failed = |out: &mut dyn Write, error: mind::Error| { let _ = writeln!(out, "ERROR: NO KEYBOARD SERVICE ({:?})", error); };
    if let Some(key) = switch { if let Err(error) = keyboard::set_switch(KEYBOARD, key) { return failed(out, error); } }
    if let Some(layout) = layout { if let Err(error) = keyboard::set_layout(KEYBOARD, layout) { return failed(out, error); } }
    match keyboard::state(KEYBOARD) {
        Ok(state) => {
            let _ = write!(out, "LAYOUT: {}  SWITCH: {}  LAYOUTS:", if state.layout == Layout::Ru { "RU" } else { "US" }, switch_name(state.switch_key));
            if let Ok(names) = keyboard::layouts(KEYBOARD) { for name in names.as_slice() { let _ = write!(out, " "); for ch in name.as_str().chars() { let _ = write!(out, "{}", ch.to_ascii_uppercase()); } } }
            let _ = writeln!(out);
        }
        Err(error) => failed(out, error),
    }
}

/// The layout and switch now, in `shell.wit`'s words (1.10, 000-APP-0048: wm's Settings).
pub fn state() -> Option<commands::Keymap> {
    let state = keyboard::state(KEYBOARD).ok()?;
    let switch = match state.switch_key {
        SwitchKey::CtrlOrAltShift => commands::LayoutSwitch::CtrlOrAltShift, SwitchKey::CtrlShift => commands::LayoutSwitch::CtrlShift,
        SwitchKey::AltShift => commands::LayoutSwitch::AltShift, SwitchKey::CapsLock => commands::LayoutSwitch::CapsLock, SwitchKey::None => commands::LayoutSwitch::None,
    };
    Some(commands::Keymap { layout: if state.layout == Layout::Ru { commands::KeyboardLayout::Ru } else { commands::KeyboardLayout::Us }, switch })
}

/// Sets them as `keymap <layout> --switch <key>` does, and answers what the keyboard then has; None without a keyboard.
pub fn set(wanted: commands::Keymap) -> Option<commands::Keymap> {
    let key = match wanted.switch {
        commands::LayoutSwitch::CtrlOrAltShift => SwitchKey::CtrlOrAltShift, commands::LayoutSwitch::CtrlShift => SwitchKey::CtrlShift,
        commands::LayoutSwitch::AltShift => SwitchKey::AltShift, commands::LayoutSwitch::CapsLock => SwitchKey::CapsLock, commands::LayoutSwitch::None => SwitchKey::None,
    };
    keyboard::set_switch(KEYBOARD, key).ok()?;
    keyboard::set_layout(KEYBOARD, if wanted.layout == commands::KeyboardLayout::Ru { Layout::Ru } else { Layout::Us }).ok()?;
    state()
}
