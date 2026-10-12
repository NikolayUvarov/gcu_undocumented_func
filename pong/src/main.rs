#![no_std]
#![no_main]
// Server: spawns ping, receives a memory capability, reads the string from the shared page and replies. The screen
// keeps the last string read and the number of calls until the next one comes (issue 098).
use core::fmt::Write;
use mind::abi::{BootInfo, CAP_GRANT, CAP_WRITE, ERR_TIMEOUT};
use mind::gfx::Screen;
use mind::ipc::{self, Endpoint, Message};
use mind::mem::Mapping;
use mind::util::FixedBuf;

const BACKGROUND: u32 = 0x00111111;
const RECEIVED_CAP: usize = 9;
const LEFT: usize = 40; // a multiple of 8 and lines a multiple of 16: the text lies on the console's character cells
const WIDTH: usize = 640;

// One line of text in place of the old one.
fn line(screen: &Screen, y: usize, text: &str, color: u32) {
    screen.fill(LEFT, y, WIDTH, 16, BACKGROUND);
    screen.text16(LEFT, y, text, color, None);
}

mind::entry!(main);
fn main(info: &'static BootInfo) {
    mind::about!("pong — IPC demo server: starts ping, reads the string from the page ping lends and replies; the screen shows the last string and the calls.\nUsage: run pong &   (fg <pid> shows it)\nEsc: exit.");
    // In wm a window of its own (000-APP-0056: otherwise it ran unseen).
    let Some(screen) = Screen::new(mind::windowed::pixels(info, 600, 240, "pong")) else { return };
    screen.clear(BACKGROUND);
    line(&screen, 48, "[ PONG / SUPERVISOR ]", 0x0000FF00);
    let endpoint = Endpoint::create().expect("EP CREATE FAILED");
    let child = mind::process::spawn("ping", Some((endpoint.0, CAP_WRITE | CAP_GRANT))).expect("SPAWN FAILED");
    mind::println!("[PONG] SPAWNED PING PID={}", child);
    let mut text = FixedBuf::<96>::new();
    let _ = write!(text, "PING IS PID {}: IT CALLS EVERY 1.5 S WITH A PAGE OF MEMORY. ESC EXITS.", child);
    line(&screen, 80, text.as_str(), 0x00888888);
    line(&screen, 112, "WAITING FOR THE FIRST CALL...", 0x00888888);
    let mut calls = 0u64;
    loop {
        // A short wait, so Esc works between calls.
        match endpoint.recv_timeout(RECEIVED_CAP, 100) {
            Ok(request) => {
                calls += 1;
                if request.cap_received {
                    if let Ok(mapping) = Mapping::new(RECEIVED_CAP) {
                        let bytes = mapping.as_slice(); let len = bytes.iter().position(|&b| b == 0).unwrap_or(0).min(70);
                        let read = core::str::from_utf8(&bytes[..len]).unwrap_or("?");
                        let mut title = FixedBuf::<64>::new();
                        let _ = write!(title, "READ FROM SHARED RAM OF PID {}:", request.sender);
                        line(&screen, 160, title.as_str(), 0x00FF00FF);
                        line(&screen, 192, read, 0x00FFFF00);
                        mind::println!("[PONG] FROM PID {}: {}", request.sender, read);
                    }
                    let _ = ipc::drop_cap(RECEIVED_CAP);
                }
                if request.is_call { let _ = ipc::reply(&Message::new(request.data[0], 0)); }
                let mut status = FixedBuf::<64>::new();
                let _ = write!(status, "CALLS ANSWERED: {}  (THE LAST: {})", calls, request.data[0]);
                line(&screen, 112, status.as_str(), 0x00FFFFFF);
            }
            Err(mind::Error::Other(ERR_TIMEOUT)) => {}
            Err(_) => { mind::time::sleep(100); }
        }
        while let Some(key) = mind::input::read_key() { if key.is_escape() { mind::process::exit(); } }
    }
}
