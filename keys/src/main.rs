#![no_std]
#![no_main]
// keys: shows every key event the program receives (key code, modifiers, character), like showkey, and pointer
// events (buttons, movement, wheel; issue 156). Esc exits.
use core::fmt::Write;
use mind::abi::BootInfo;
use mind::input::{Code, Input, Key};
use mind::tui::{Line, Rect, Terminal, DARK};
use mind::util::FixedBuf;

const HISTORY: usize = 64;

fn describe(key: Key, out: &mut FixedBuf<96>) {
    let mods = [(key.shift(), 'S'), (key.ctrl(), 'C'), (key.alt(), 'A')];
    let _ = write!(out, "code={:?} mods=", key.code());
    if mods.iter().all(|m| !m.0) { let _ = out.write_char('-'); }
    for (on, letter) in mods { if on { let _ = out.write_char(letter); } }
    match key.char() {
        Some(ch) if !ch.is_control() => { let _ = write!(out, " char={} U+{:04X}", ch, ch as u32); }
        Some(ch) => { let _ = write!(out, " char=U+{:04X}", ch as u32); }
        None => {}
    }
}

struct Lines { text: [FixedBuf<96>; HISTORY], count: usize }

fn draw(term: &mut Terminal, lines: &Lines) {
    let theme = DARK;
    let mut grid = term.grid();
    grid.clear(theme.panel);
    let area = Rect::new(0, 0, grid.cols, grid.rows - 1);
    grid.frame_titled(area, Line::Double, "keys — коды клавиш", theme.frame, theme.header);
    let inner = area.inner();
    grid.text(inner.x + 1, inner.y, "Нажимайте клавиши: код, модификаторы (S/C/A) и символ. Esc — выход.", theme.dim);
    let rows = inner.h.saturating_sub(2);
    let first = lines.count.saturating_sub(rows.min(HISTORY));
    for (row, index) in (first..lines.count).enumerate() {
        let line = &lines.text[index % HISTORY];
        let style = if index + 1 == lines.count { theme.accent } else { theme.panel };
        grid.text(inner.x + 1, inner.y + 2 + row, core::str::from_utf8(line.as_bytes()).unwrap_or("?"), style);
    }
    let mut count = FixedBuf::<32>::new();
    let _ = write!(count, " {} ", lines.count);
    grid.text_right(area.right() - 2, area.bottom() - 1, core::str::from_utf8(count.as_bytes()).unwrap_or(""), theme.frame);
    // Every key including F10 is shown, so only Esc exits.
    let rows = grid.rows;
    grid.text_padded(0, rows - 1, " Esc — выход   Ctrl+Shift / Alt+Shift — раскладка EN/RU", grid.cols, theme.status);
}

mind::entry!(main);
fn main(info: &'static BootInfo) {
    mind::about!("keys — shows every key event the program receives: key code, modifiers, character.\nUsage: keys\nEsc: exit.");
    let mut term = Terminal::open(info, "keys");
    let mut lines = Lines { text: core::array::from_fn(|_| FixedBuf::new()), count: 0 };
    if let Some(term) = term.as_mut() { draw(term, &lines); term.present(); }
    mind::input::pointer(true); // mouse events too (issue 156)
    // Without process control a program cannot take keys from others (issue 154).
    mind::println!("[KEYS] LISTEN: {:?}", mind::input::listen(mind::abi::KEY_F1, 0, true));
    mind::println!("[KEYS] READY");
    loop {
        let key = match mind::input::read_input() {
            None => { mind::time::sleep(1000); continue } // input ends the sleep early
            Some(Input::Pointer(p)) => {
                let line = &mut lines.text[lines.count % HISTORY];
                line.clear();
                let _ = write!(line, "pointer buttons={} dx={} dy={} wheel={}", p.buttons, p.dx, p.dy, p.wheel);
                lines.count += 1;
                mind::println!("[KEYS] {}", core::str::from_utf8(line.as_bytes()).unwrap_or("?"));
                if let Some(term) = term.as_mut() { draw(term, &lines); term.present(); }
                continue;
            }
            Some(Input::Key(event)) => match mind::input::Key::from_event(event.to_word()) { Some(key) => key, None => continue },
        };
        let line = &mut lines.text[lines.count % HISTORY];
        line.clear();
        describe(key, line);
        lines.count += 1;
        mind::println!("[KEYS] {}", core::str::from_utf8(line.as_bytes()).unwrap_or("?"));
        if let Some(term) = term.as_mut() { draw(term, &lines); term.present(); }
        if key.code() == Code::Esc { mind::println!("[KEYS] DONE"); return; }
    }
}
