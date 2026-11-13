#![no_std]
#![no_main]
//! Test-only console program (QEMU `tools` suite, 000-APP-0032): says whether it holds a clipboard client in
//! SLOT_CLIPBOARD and, holding one, pastes 500 ms after its start, once the shell names a program in front, and prints
//! the text's length or the service's refusal, never the text. The suite also starts a copy whose request note asks for
//! no clipboard. Never packaged into the normal OS.
extern crate alloc;
use mind::abi::BootInfo;

mind::request!(REQUEST_CONSOLE | REQUEST_CLIPBOARD);

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    if mind::clipboard::client().is_none() { mind::println!("CLIP STUB: NO CLIPBOARD CLIENT"); mind::process::exit_with(0) }
    mind::println!("CLIP STUB: A CLIPBOARD CLIENT");
    let _ = mind::time::sleep(500);
    match mind::clipboard::paste() {
        Ok(text) => mind::println!("CLIP STUB: PASTED {} BYTES", text.len()),
        Err(error) => mind::println!("CLIP STUB: NOT PASTED: {}", error.text()),
    }
    mind::process::exit_with(0)
}
