#![no_std]
#![no_main]
// Композитор в ring 3: переносит изменившиеся пиксели активного экрана в кадр GOP.
use mind::abi::{BootInfo, SLOT_MEM};
use mind::dev::{compositor_pull, Frame};
use mind::mem::{Mapping, Pages};

const SOURCE_SLOT: usize = 9; // сюда ядро кладёт мандат на активный экран

mind::entry!(main);
fn main(info: &'static BootInfo) {
    let pixels = info.stride * info.height;
    // Ошибки отображения больше не превращаются в запись по адресу usize::MAX.
    let Ok(gop) = Mapping::new(SLOT_MEM) else { mind::println!("[COMPOSITOR] NO FRAMEBUFFER MAPPING"); return };
    let Some(mut shadow) = Pages::new(pixels * 4) else { mind::println!("[COMPOSITOR] NO MEMORY FOR SHADOW"); return };
    let (gop, shadow) = (gop.as_ptr::<u32>(), shadow.as_mut_slice().as_mut_ptr() as *mut u32);
    let mut source: Option<Mapping> = None;
    let mut valid = false;
    loop {
        let frame = compositor_pull(SOURCE_SLOT).unwrap_or(Frame::Unchanged);
        if frame == Frame::NewSource {
            drop(source.take()); // сначала снять старое отображение (квота разделяемой памяти)
            source = Mapping::new(SOURCE_SLOT).ok();
            let _ = mind::ipc::drop_cap(SOURCE_SLOT); // мандат больше не нужен: отображение держит память
        }
        if let (Some(screen), Frame::Dirty | Frame::NewSource) = (source.as_ref(), frame) {
            let screen = screen.as_ptr::<u32>();
            for i in 0..pixels {
                let pixel = unsafe { core::ptr::read_volatile(screen.add(i)) };
                if !valid || pixel != unsafe { core::ptr::read(shadow.add(i)) } {
                    unsafe { core::ptr::write_volatile(gop.add(i), pixel); core::ptr::write(shadow.add(i), pixel); }
                }
            }
            valid = true;
        }
        mind::time::sleep(15);
    }
}
