#![no_std]
#![no_main]
use mind::abi::BootInfo;
use mind::gfx::Screen;

const BACKGROUND: u32 = 0x001E1E2E; const FOREGROUND: u32 = 0x00A6E3A1;
fn time_text(seconds: usize) -> [u8; 8] { let hour = seconds / 3600; let minute = (seconds / 60) % 60; let second = seconds % 60; [ b'0' + (hour / 10) as u8, b'0' + (hour % 10) as u8, b':', b'0' + (minute / 10) as u8, b'0' + (minute % 10) as u8, b':', b'0' + (second / 10) as u8, b'0' + (second % 10) as u8 ] }

mind::entry!(main);
fn main(info: &'static BootInfo) {
    mind::about!("clock — a digital clock from the RTC on its own screen, or in a window under wm.\nUsage: clock\nEsc: exit.");
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
