#![no_std]
#![no_main]
use mind::abi::BootInfo;
use mind::gfx::Screen;
use mind::tui::Terminal;
use core::fmt::Write;
use mind::{tui, util};

mod text;

const BACKGROUND: u32 = 0x001E1E2E; const FOREGROUND: u32 = 0x00A6E3A1;
fn time_text(seconds: usize) -> [u8; 8] { let hour = seconds / 3600; let minute = (seconds / 60) % 60; let second = seconds % 60; [ b'0' + (hour / 10) as u8, b'0' + (hour % 10) as u8, b':', b'0' + (minute / 10) as u8, b'0' + (minute % 10) as u8, b':', b'0' + (second / 10) as u8, b'0' + (second % 10) as u8 ] }

mind::request!(REQUEST_LINE);

mind::entry!(main);
fn main(info: &'static BootInfo) {
    mind::about!("clock — a digital clock from the RTC on its own screen, or in a window under wm.\nUsage: clock [--text | --line]   (--text: large digits of text characters with the date, sized to the screen or the window;\n--line: a console program, the time and the date on one line, written again every second)\nEsc: exit.");
    if mind::process::args_str().split_whitespace().any(|a| a == "--line") { return line_face(); }
    if mind::process::args_str().split_whitespace().any(|a| a == "--text") { return text_face(info); }
    // Started by a window manager: a 320 × 176 window instead of the screen (issue 088), drawn again at the size of
    // its frame when that changes (issue u009).
    let info = mind::windowed::pixels(info, 320, 176, "clock");
    let Some(mut screen) = Screen::new(info) else { return };
    let mut rtc = mind::rtc::Clock::new(); // the RTC read once a minute (000-APP-0012)
    let mut previous_time = None;
    let (mut x, mut y, mut scale) = face(&screen, previous_time);
    loop {
        mind::input::wait_or_exit(100);
        if let Some(resized) = mind::windowed::pixels_resized() {
            if let Some(new) = Screen::new(&resized) { screen = new; (x, y, scale) = face(&screen, previous_time); }
            mind::println!("[CLOCK] SIZE {}X{}", resized.width, resized.height);
        }
        let Some(seconds) = rtc.seconds_since_midnight() else { continue };
        if previous_time != Some(seconds) {
            let text = time_text(seconds);
            screen.text(x, y, &text, scale, FOREGROUND, Some(BACKGROUND));
            mind::process::log(b"[CLOCK] "); mind::process::log(&text); mind::process::log(b"\r\n");
            previous_time = Some(seconds);
        }
    }
}

// Started as a console program (`clock --line`, issue u016): the time and the date on one line, written again with \r
// every second; Esc in the shell or `console` stops it.
fn line_face() {
    let mut rtc = mind::rtc::Clock::new(); // the RTC read once a minute (000-APP-0012)
    let mut previous = None;
    loop {
        if let Some(seconds) = rtc.seconds_since_midnight().filter(|&s| previous != Some(s)) {
            let mut line = util::FixedBuf::<40>::new();
            let _ = write!(line, "\r{}", core::str::from_utf8(&time_text(seconds)).unwrap_or(""));
            if let Some((year, month, day)) = rtc.date() { let _ = write!(line, "  {:04}-{:02}-{:02}", year, month, day); }
            mind::process::log(line.as_bytes());
            previous = Some(seconds);
        }
        mind::time::sleep(100);
    }
}

// The whole face at the screen's size: the title where it fits above the time, and the time (dashes before the
// first); returns where the time goes and its scale.
fn face(screen: &Screen, seconds: Option<usize>) -> (usize, usize, usize) {
    screen.clear(BACKGROUND);
    let scale = (screen.width / 80).min(screen.height / 32).clamp(1, 8);
    let (x, y) = (screen.width.saturating_sub(64 * scale) / 2, screen.height.saturating_sub(8 * scale) / 2);
    if y >= 48 && screen.width >= 264 { screen.text(24, 24, b"CLOCK (IPC RTC)", 2, FOREGROUND, Some(BACKGROUND)); }
    screen.text(x, y, &seconds.map_or(*b"--:--:--", time_text), scale, FOREGROUND, Some(BACKGROUND));
    (x, y, scale)
}

// The text face (issue 089): drawn again at every change of the second, and at once when a window is resized.
fn text_face(info: &'static BootInfo) {
    let Some(mut term) = Terminal::open(info, "clock") else { return };
    let mut rtc = mind::rtc::Clock::new(); // the RTC read once a minute (000-APP-0012)
    let mut previous = None;
    loop {
        let seconds = rtc.seconds_since_midnight();
        let date = rtc.date().map(|(y, m, d)| (y, m, d, mind::rtc::days_from_civil(y, m, d).map_or(0, text::weekday)));
        { let mut grid = term.grid(); text::draw(&mut grid, seconds, date); }
        term.present();
        if seconds.is_some() && seconds != previous {
            previous = seconds;
            mind::process::log(b"[CLOCK] "); mind::process::log(&text::time_text(seconds)); mind::process::log(b"\r\n");
        }
        mind::input::wait_or_exit(100);
    }
}
