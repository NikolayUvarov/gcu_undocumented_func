#![no_std]
#![no_main]
use mind::abi::BootInfo;
use mind::gfx::Screen;
use mind::tui::Terminal;
use mind::{tui, util};

mod text;

const BACKGROUND: u32 = 0x001E1E2E; const FOREGROUND: u32 = 0x00A6E3A1;
fn time_text(seconds: usize) -> [u8; 8] { let hour = seconds / 3600; let minute = (seconds / 60) % 60; let second = seconds % 60; [ b'0' + (hour / 10) as u8, b'0' + (hour % 10) as u8, b':', b'0' + (minute / 10) as u8, b'0' + (minute % 10) as u8, b':', b'0' + (second / 10) as u8, b'0' + (second % 10) as u8 ] }

mind::entry!(main);
fn main(info: &'static BootInfo) {
    mind::about!("clock — a digital clock from the RTC on its own screen, or in a window under wm.\nUsage: clock [--text]   (--text: large digits of text characters with the date, sized to the screen or the window)\nEsc: exit.");
    if mind::process::args_str().split_whitespace().any(|a| a == "--text") { return text_face(info); }
    // Started by a window manager: a 320 × 176 window instead of the screen (issue 088).
    let info = mind::windowed::pixels(info, 320, 176, "clock");
    let Some(screen) = Screen::new(info) else { return };
    screen.clear(BACKGROUND);
    screen.text(24, 24, b"CLOCK (IPC RTC)", 2, FOREGROUND, Some(BACKGROUND));
    let scale = (info.width / 80).min(info.height / 32).clamp(1, 8); let x = info.width.saturating_sub(64 * scale) / 2; let y = info.height.saturating_sub(8 * scale) / 2;
    screen.text(x, y, b"--:--:--", scale, FOREGROUND, Some(BACKGROUND));
    let mut previous_time = None;
    loop {
        mind::input::wait_or_exit(100);
        let Some(seconds) = mind::rtc::seconds_since_midnight() else { continue };
        if previous_time != Some(seconds) {
            let text = time_text(seconds);
            screen.text(x, y, &text, scale, FOREGROUND, Some(BACKGROUND));
            mind::process::log(b"[CLOCK] "); mind::process::log(&text); mind::process::log(b"\r\n");
            previous_time = Some(seconds);
        }
    }
}

// The text face (issue 089): drawn again at every change of the second, and at once when a window is resized.
fn text_face(info: &'static BootInfo) {
    let Some(mut term) = Terminal::open(info, "clock") else { return };
    let mut previous = None;
    loop {
        let seconds = mind::rtc::seconds_since_midnight();
        let date = mind::rtc::date().map(|(y, m, d)| (y, m, d, mind::rtc::days_from_civil(y, m, d).map_or(0, text::weekday)));
        { let mut grid = term.grid(); text::draw(&mut grid, seconds, date); }
        term.present();
        if seconds.is_some() && seconds != previous {
            previous = seconds;
            mind::process::log(b"[CLOCK] "); mind::process::log(&text::time_text(seconds)); mind::process::log(b"\r\n");
        }
        mind::input::wait_or_exit(100);
    }
}
