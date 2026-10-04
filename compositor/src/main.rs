#![no_std]
#![no_main]
// Ring 3 compositor: copies changed pixels of the active screen into the GOP framebuffer. Between frames it serves
// idl/display.wit on its service endpoint: the mode, and a sealed copy of the screen in front (the shell's
// `screenshot`, issue 086).
use mind::abi::{pixel_to_device, BootInfo, ERR_TIMEOUT, PIXEL_BGR, SLOT_MEM};
use mind::dev::{compositor_pull, Frame};
use mind::idl::display::{self, Error, Mode, Request};
use mind::idl::wire;
use mind::ipc::Endpoint;
use mind::mem::{Mapping, Pages};

const RECEIVED_CAP: usize = 10;
const FRAME_MS: u32 = 15;

// One display.wit request: the mode, or a sealed read-only copy of what `source` shows.
fn serve(info: &BootInfo, source: Option<&Mapping>, request: Request, call: wire::Call) {
    let _ = match request {
        Request::Mode => display::reply_mode(call, &Mode { width: info.width as u32, height: info.height as u32, stride: info.stride as u32 }),
        Request::Capture => {
            let bytes = info.stride * info.height * 4;
            let copy = match source {
                Some(screen) if screen.len() >= bytes => mind::mem::sealed_copy(&screen.as_slice()[..bytes]).map_err(|_| Error::NoMemory),
                _ => Err(Error::NoScreen),
            };
            display::reply_capture(call, copy)
        }
    };
}

const SOURCE_SLOT: usize = 9; // the kernel puts the active screen capability here

mind::entry!(main);
fn main(info: &'static BootInfo) {
    let pixels = info.stride * info.height;
    // Mapping errors no longer turn into writes to address usize::MAX.
    let Ok(gop) = Mapping::new(SLOT_MEM) else { mind::println!("[COMPOSITOR] NO FRAMEBUFFER MAPPING"); return };
    let Some(mut shadow) = Pages::new(pixels * 4) else { mind::println!("[COMPOSITOR] NO MEMORY FOR SHADOW"); return };
    let (gop, shadow) = (gop.as_ptr::<u32>(), shadow.as_mut_slice().as_mut_ptr() as *mut u32);
    let mut source: Option<Mapping> = None;
    let mut valid = false;
    let native = info.pixel_format == PIXEL_BGR; // screens already hold the framebuffer's layout
    loop {
        let frame = compositor_pull(SOURCE_SLOT).unwrap_or(Frame::Unchanged);
        if frame == Frame::NewSource {
            drop(source.take()); // unmap the old mapping first (shared memory quota)
            source = Mapping::new(SOURCE_SLOT).ok();
            let _ = mind::ipc::drop_cap(SOURCE_SLOT); // the capability is no longer needed: the mapping keeps the memory
        }
        if let (Some(screen), Frame::Dirty | Frame::NewSource) = (source.as_ref(), frame) {
            let screen = screen.as_ptr::<u32>();
            for i in 0..pixels {
                let pixel = unsafe { core::ptr::read_volatile(screen.add(i)) };
                if !valid || pixel != unsafe { core::ptr::read(shadow.add(i)) } {
                    let device = if native { pixel } else { pixel_to_device(pixel, info.pixel_format, info.pixel_masks) };
                    unsafe { core::ptr::write_volatile(gop.add(i), device); core::ptr::write(shadow.add(i), pixel); }
                }
            }
            valid = true;
        }
        // Wait for the next frame, answering requests meanwhile (without a service endpoint: just wait).
        match Endpoint::SERVICE.recv_timeout(RECEIVED_CAP, FRAME_MS) {
            Ok(request) => match display::decode(&request, RECEIVED_CAP) {
                Ok((request, call)) => serve(info, source.as_ref(), request, call),
                Err(reason) => if request.is_call { let _ = wire::reject(reason); },
            },
            Err(mind::Error::Other(ERR_TIMEOUT)) => {}
            Err(_) => { mind::time::sleep(FRAME_MS as usize); }
        }
    }
}
