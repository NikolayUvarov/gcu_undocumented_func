#![no_std]
#![no_main]
use mind::{abi, font, tui};
mod cycle;
mod face;
mod text;
mod view;
use abi::BootInfo;
use cycle::{point_at, Cycle, OrbitMode};
use face::{time_text, Face};
use view::View;

fn print(text: &[u8]) { mind::process::log(text) }

mind::entry!(main);
fn main(info: &'static BootInfo) {
    mind::about!("dzen-clock — the Dzen clock on its own screen, or in a window under wm.\nUsage: dzen-clock [--text]   (--text: the text face, the indicators in colored text cells, sized to the screen or the window)\nD digital time, C orbit, P orbit with 10 s ticks, H title and keys, T the text face or the pixel one (on its own screen), Esc: exit.");
    let mut text = mind::process::args_str().split_whitespace().any(|a| a == "--text" || a == "text");
    // T switches faces on the program's own screen; a window of wm is a text or a pixel window from the start.
    while (if text { text_face(info) } else { pixel_face(info) }) == Next::Switch {
        text = !text;
        print(if text { b"[DZEN-CLOCK] TEXT FACE\r\n" } else { b"[DZEN-CLOCK] PIXEL FACE\r\n" });
    }
}

/// Why a face ended.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Next { Exit, Switch }

// T on the program's own screen (not in a window).
fn switch_key(key: Option<mind::input::Key>) -> bool { !mind::windowed::active() && key.and_then(|k| k.char()).is_some_and(|c| c.eq_ignore_ascii_case(&'t')) }

fn pixel_face(info: &'static BootInfo) -> Next {
    // Started by a window manager: a 400 × 320 window instead of the screen (issue 088).
    let info = mind::windowed::pixels(info, 400, 320, "dzen-clock");
    print(
        b"\r\n[DZEN-CLOCK] STARTED. D: DIGITS, C: ORBIT, P: 10S TICKS, H: TEXT, CTRL+Z: SHELL, ESC: EXIT.\r\n",
    );
    let view = View::new(info);
    view.clear();
    view.hints(true, !mind::windowed::active());
    view.face(Face::DARK);
    view.digital(b"--:--:--", true);
    let mut previous_face = Face::DARK;
    let mut previous_time = None;
    let mut show_digits = true;
    let mut show_hints = true;
    let mut orbit_mode = OrbitMode::Off;
    let mut previous_mode = OrbitMode::Off;
    let mut cycle = Cycle::new();
    let mut previous_dot = None;
    let mut waiting = false;
    loop {
        let key = mind::input::read_key();
        if key.is_some_and(mind::input::is_escape) {
            print(b"[DZEN-CLOCK] RETURNING TO KERNEL.\r\n");
            return Next::Exit;
        }
        if switch_key(key) { return Next::Switch; }
        // D/d, H/h, C/c, P/p from either keyboard (key events carry the character).
        let key = key.and_then(|k| k.char()).map_or(0, |c| c.to_ascii_lowercase() as usize);
        if key == b'd' as usize {
            show_digits = !show_digits;
            let text = previous_time.map(time_text).unwrap_or(*b"--:--:--");
            view.digital(&text, show_digits);
            print(
                if show_digits {
                    b"[DZEN-CLOCK] DIGITS ON\r\n"
                } else {
                    b"[DZEN-CLOCK] DIGITS OFF\r\n"
                },
            );
        }
        if key == b'h' as usize {
            show_hints = !show_hints;
            view.hints(show_hints, !mind::windowed::active());
            print(
                if show_hints {
                    b"[DZEN-CLOCK] TEXT ON\r\n"
                } else {
                    b"[DZEN-CLOCK] TEXT OFF\r\n"
                },
            );
        }
        let requested = match key as u8 {
            b'c' => Some(OrbitMode::Simple),
            b'p' => Some(OrbitMode::Ticks),
            _ => None,
        };
        if let Some(requested) = requested {
            orbit_mode = orbit_mode.toggle(requested);
            print(
                match orbit_mode {
                    OrbitMode::Off => b"[DZEN-CLOCK] ORBIT OFF\r\n",
                    OrbitMode::Simple => b"[DZEN-CLOCK] ORBIT SIMPLE\r\n",
                    OrbitMode::Ticks => b"[DZEN-CLOCK] ORBIT 10S TICKS\r\n",
                },
            );
        }
        // Time now comes from the rtc driver over IPC rather than a kernel syscall.
        let seconds = mind::rtc::seconds_since_midnight().unwrap_or(abi::RTC_UNAVAILABLE);
        let now = mind::time::uptime_ms();
        cycle.observe(seconds, now);
        if let Some(current) = Face::at(seconds) {
            if current != previous_face {
                view.face(current);
                previous_face = current;
                print(b"[DZEN-CLOCK] ");
                print(&time_text(seconds));
                print(b"\r\n");
            }
            if previous_time != Some(seconds) && show_digits {
                view.digital(&time_text(seconds), true);
            }
            previous_time = Some(seconds);
            waiting = false;
        } else if previous_time.is_none() && !waiting {
            // An RTC update in progress is transient: retain the last valid face.
            print(b"[DZEN-CLOCK] WAITING FOR RTC...\r\n");
            waiting = true;
        }
        let dot = if orbit_mode != OrbitMode::Off {
            cycle
                .phase(now)
                .map(|phase| point_at(phase, view.half() * 3 / 4))
        } else {
            None
        };
        if dot != previous_dot || orbit_mode != previous_mode {
            view.cycle(previous_dot, dot, previous_mode, orbit_mode);
            previous_dot = dot;
            previous_mode = orbit_mode;
        }
        // Sleep instead of polling continuously, including when digits are hidden.
        mind::time::sleep(100);
    }
}

// The text face (issue 089): the same keys and log lines as the pixel face; drawn again every 100 ms.
fn text_face(info: &'static BootInfo) -> Next {
    let Some(mut term) = mind::tui::Terminal::open(info, "dzen-clock") else { return Next::Exit };
    print(b"\r\n[DZEN-CLOCK] STARTED (TEXT). D: DIGITS, C: ORBIT, P: 10S TICKS, H: TEXT, CTRL+Z: SHELL, ESC: EXIT.\r\n");
    let mut show = text::Show { digits: true, hints: true, mode: OrbitMode::Off, switch: !mind::windowed::active() };
    let mut cycle = Cycle::new();
    let mut previous_face = None;
    loop {
        let key = mind::input::read_key();
        if key.is_some_and(mind::input::is_escape) { print(b"[DZEN-CLOCK] RETURNING TO KERNEL.\r\n"); return Next::Exit; }
        if switch_key(key) { return Next::Switch; }
        match key.and_then(|k| k.char()).map(|c| c.to_ascii_lowercase()) {
            Some('d') => { show.digits = !show.digits; print(if show.digits { b"[DZEN-CLOCK] DIGITS ON\r\n" } else { b"[DZEN-CLOCK] DIGITS OFF\r\n" }); }
            Some('h') => { show.hints = !show.hints; print(if show.hints { b"[DZEN-CLOCK] TEXT ON\r\n" } else { b"[DZEN-CLOCK] TEXT OFF\r\n" }); }
            Some(c @ ('c' | 'p')) => {
                show.mode = show.mode.toggle(if c == 'c' { OrbitMode::Simple } else { OrbitMode::Ticks });
                print(match show.mode { OrbitMode::Off => b"[DZEN-CLOCK] ORBIT OFF\r\n", OrbitMode::Simple => b"[DZEN-CLOCK] ORBIT SIMPLE\r\n", OrbitMode::Ticks => b"[DZEN-CLOCK] ORBIT 10S TICKS\r\n" });
            }
            _ => {}
        }
        let seconds = mind::rtc::seconds_since_midnight().filter(|&s| s < 86400);
        let now = mind::time::uptime_ms();
        if let Some(s) = seconds { cycle.observe(s, now); }
        let current = seconds.and_then(Face::at);
        if let (Some(face), Some(s)) = (current, seconds) {
            if previous_face != Some(face) { previous_face = Some(face); print(b"[DZEN-CLOCK] "); print(&time_text(s)); print(b"\r\n"); }
        }
        {
            let mut grid = term.grid();
            text::draw(&mut grid, current.or(previous_face).unwrap_or(Face::DARK), seconds, cycle.phase(now), show);
        }
        term.present();
        mind::time::sleep(100);
    }
}
