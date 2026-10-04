#![no_std]
#![no_main]
// keys: shows every key event the program receives (key code, modifiers, character), like showkey. Esc exits.
use core::fmt::Write;
use mind::abi::BootInfo;
use mind::gfx::Screen;
use mind::input::{Code, Key};
use mind::util::FixedBuf;

const BACKGROUND: u32 = 0x00101820; const TEXT: u32 = 0x00E0E0E0; const ACCENT: u32 = 0x0080D0FF;

fn describe(key: Key, out: &mut FixedBuf<96>) {
    let mods = [(key.shift(), 'S'), (key.ctrl(), 'C'), (key.alt(), 'A')];
    let _ = write!(out, "[KEYS] code={:?} mods=", key.code());
    if mods.iter().all(|m| !m.0) { let _ = out.write_char('-'); }
    for (on, letter) in mods { if on { let _ = out.write_char(letter); } }
    match key.char() {
        Some(ch) if !ch.is_control() => { let _ = write!(out, " char={} U+{:04X}", ch, ch as u32); }
        Some(ch) => { let _ = write!(out, " char=U+{:04X}", ch as u32); }
        None => {}
    }
}

mind::entry!(main);
fn main(info: &'static BootInfo) {
    let screen = Screen::new(info);
    if let Some(s) = screen {
        s.clear(BACKGROUND);
        s.text16(16, 16, "keys — коды клавиш: нажимайте клавиши, Esc — выход", ACCENT, Some(BACKGROUND));
    }
    mind::println!("[KEYS] READY");
    let mut row = 0usize;
    loop {
        let Some(key) = mind::input::wait_key(1000) else { continue };
        let mut line = FixedBuf::<96>::new();
        describe(key, &mut line);
        mind::println!("{}", core::str::from_utf8(line.as_bytes()).unwrap_or("?"));
        if let Some(s) = screen {
            let rows = (s.height.saturating_sub(64)) / 16;
            if rows > 0 {
                let y = 48 + (row % rows) * 16;
                s.fill(16, y, s.width - 32, 16, BACKGROUND);
                s.text16(16, y, core::str::from_utf8(&line.as_bytes()[7..]).unwrap_or("?"), TEXT, Some(BACKGROUND));
                row += 1;
            }
        }
        if key.code() == Code::Esc { mind::println!("[KEYS] DONE"); return; }
    }
}
