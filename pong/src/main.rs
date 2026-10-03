#![no_std]
#![no_main]
// Server: spawns ping, receives a memory capability, reads the string from the shared page and replies.
use mind::abi::{BootInfo, CAP_GRANT, CAP_WRITE};
use mind::gfx::Screen;
use mind::ipc::{self, Endpoint, Message};
use mind::mem::Mapping;

const BACKGROUND: u32 = 0x00111111;
const RECEIVED_CAP: usize = 9;

mind::entry!(main);
fn main(info: &'static BootInfo) {
    let Some(screen) = Screen::new(info) else { return };
    screen.clear(BACKGROUND);
    screen.text(40, 40, b"[ PONG / SUPERVISOR ]", 1, 0x0000FF00, None);
    let endpoint = Endpoint::create().expect("EP CREATE FAILED");
    let child = mind::process::spawn("ping", Some((endpoint.0, CAP_WRITE | CAP_GRANT))).expect("SPAWN FAILED");
    mind::println!("[PONG] SPAWNED PING PID={}", child);
    loop {
        screen.fill(40, 100, 560, 130, BACKGROUND);
        screen.text(40, 100, b"WAITING FOR CALL...", 1, 0x00888888, None);
        let Ok(request) = endpoint.recv(RECEIVED_CAP) else { mind::input::wait_or_exit(1000); continue };
        if request.cap_received {
            if let Ok(mapping) = Mapping::new(RECEIVED_CAP) {
                let bytes = mapping.as_slice(); let len = bytes.iter().position(|&b| b == 0).unwrap_or(0).min(70);
                screen.text(40, 160, b"READ FROM SHARED RAM:", 1, 0x00FF00FF, None);
                screen.text(40, 190, &bytes[..len], 1, 0x00FFFF00, None);
                mind::println!("[PONG] FROM PID {}: {}", request.sender, core::str::from_utf8(&bytes[..len]).unwrap_or("?"));
            }
            let _ = ipc::drop_cap(RECEIVED_CAP);
        }
        if request.is_call { let _ = ipc::reply(&Message::new(request.data[0], 0)); }
        mind::input::wait_or_exit(10);
    }
}
