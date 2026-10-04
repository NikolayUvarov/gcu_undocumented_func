#![no_std]
#![no_main]
// Client: writes a string into its page and lends it read-only to the server for one CALL.
use core::fmt::Write;
use mind::abi::BootInfo;
use mind::gfx::Screen;
use mind::ipc::{Endpoint, Message};
use mind::mem::Pages;
use mind::util::FixedBuf;

const BACKGROUND: u32 = 0x00111111;

mind::entry!(main);
fn main(info: &'static BootInfo) {
    let Some(screen) = Screen::new(info) else { return };
    screen.clear(BACKGROUND);
    screen.text(40, 40, b"[ PING / CLIENT ]", 1, 0x00FF0000, None);
    let mut shared = Pages::new(4096).expect("ALLOC FAILED");
    let cap = shared.share().expect("MEM SHARE FAILED");
    let server = Endpoint::INIT; // the parent passed the server endpoint at SPAWN
    for counter in 1000.. {
        screen.fill(40, 70, 560, 130, BACKGROUND);
        let mut text = FixedBuf::<64>::new();
        let _ = write!(text, "HELLO FROM PING! ZERO-COPY IPC SUCCESS! COUNT: {}", counter);
        let bytes = shared.as_mut_slice(); bytes[..text.as_bytes().len()].copy_from_slice(text.as_bytes()); bytes[text.as_bytes().len()] = 0;
        screen.text(40, 100, b"CALLING SERVER WITH MEMORY CAP", 1, 0x00FFFFFF, None);
        let lent = mind::ipc::mint(cap, mind::abi::CAP_READ, 0, 0).expect("MEM MINT FAILED");
        let reply = server.call(&Message::new(counter, 0).with_cap(lent, 0), 0);
        let _ = mind::ipc::revoke(cap);
        match reply {
            Ok(reply) if reply.data[0] == counter => { screen.text(40, 160, b"SERVER CONFIRMED RECEIPT!", 1, 0x0000FF00, None); mind::println!("[PING] ACK {}", counter); }
            _ => screen.text(40, 160, b"CALL ERROR", 1, 0x00FF0000, None),
        }
        mind::input::wait_or_exit(1500);
    }
}
