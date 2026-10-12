#![no_std]
#![no_main]
// VFS demo: lists the disk root and reads a file via vfs_server.
extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;
use mind::abi::BootInfo;
use mind::fs::{self, File};
use mind::gfx::Screen;
use mind::util::FixedBuf;

const BACKGROUND: u32 = 0x00101820; const TEXT: u32 = 0x00E0E0E0; const ACCENT: u32 = 0x0080D0FF;
// Drawn with the 8x16 font (MIND Mono 16); the services suite compares it with the font's bitmaps.
const TITLE: &str = "Files — демо VFS-сервера ╞═╡ Esc: выход";

// The bootloader of the architecture this program was built for.
#[cfg(target_arch = "aarch64")]
const BOOTLOADER: &str = "EFI/BOOT/BOOTAA64.EFI";
#[cfg(not(target_arch = "aarch64"))]
const BOOTLOADER: &str = "EFI/BOOT/BOOTX64.EFI";

mind::entry!(main);
fn main(info: &'static BootInfo) {
    mind::about!("files — VFS demo: lists the root of the disk and reads a file through vfs_server.\nUsage: files\nEsc: exit.");
    // In wm a window of its own (000-APP-0056: otherwise it ran unseen); its lines are kept to draw them again at the
    // size wm gives it.
    let info = mind::windowed::pixels(info, 608, 336, "files");
    let screen = Screen::new(info);
    let draw = |s: &Screen, lines: &[Vec<u8>]| {
        s.clear(BACKGROUND);
        s.text16(24, 24, TITLE, ACCENT, Some(BACKGROUND));
        for (i, text) in lines.iter().enumerate() { s.text(24, 64 + 12 * i, text, 1, TEXT, None); }
    };
    if let Some(s) = screen { draw(&s, &[]); }
    let mut shown: Vec<Vec<u8>> = Vec::new();
    let mut line = |text: &[u8]| {
        mind::process::log(text); mind::process::log(b"\n");
        if let Some(s) = screen { s.text(24, 64 + 12 * shown.len(), text, 1, TEXT, None); }
        shown.push(text.to_vec());
    };

    let mut out = FixedBuf::<96>::new();
    // The listing is collected on the program heap (mind::alloc) and sorted: directories first, then by name.
    let mut entries: Vec<(String, u32, bool)> = Vec::new();
    let listed = fs::list("", |entry| entries.push((String::from(core::str::from_utf8(entry.name).unwrap_or("?")), entry.size, entry.is_dir)));
    entries.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    for (name, size, is_dir) in &entries {
        out.clear();
        let _ = write!(out, "[FILES] {} {}{}", name, size, if *is_dir { " <DIR>" } else { "" });
        line(out.as_bytes());
    }
    match listed {
        Ok(count) => { out.clear(); let _ = write!(out, "[FILES] {} ENTRIES IN / (SORTED, HEAP ARENAS={})", count, mind::heap_stats().arenas); line(out.as_bytes()); }
        Err(error) => { out.clear(); let _ = write!(out, "[FILES] LIST FAILED: {:?}", error); line(out.as_bytes()); }
    }

    // Read the whole ELF in chunks and compute a simple checksum.
    for path in ["kernel.elf", BOOTLOADER] {
        out.clear();
        match File::open(path) {
            Ok(mut file) => {
                let mut chunk = [0u8; 4096]; let (mut total, mut sum, mut magic) = (0usize, 0u32, [0u8; 4]);
                while let Ok(n) = file.read(&mut chunk) {
                    if n == 0 { break; }
                    if total == 0 { magic.copy_from_slice(&chunk[..4]); }
                    sum = chunk[..n].iter().fold(sum, |s, &b| s.wrapping_mul(31).wrapping_add(b as u32)); total += n;
                }
                let _ = write!(out, "[FILES] READ {} {}/{} BYTES MAGIC={:02X}{:02X}{:02X}{:02X} SUM={:08X}", path, total, file.size(), magic[0], magic[1], magic[2], magic[3], sum);
            }
            Err(error) => { let _ = write!(out, "[FILES] OPEN {} FAILED: {:?}", path, error); }
        }
        line(out.as_bytes());
    }
    line(b"[FILES] DONE");
    loop {
        if let Some(resized) = mind::windowed::pixels_resized() { if let Some(s) = Screen::new(&resized) { draw(&s, &shown); } }
        mind::input::wait_or_exit(200);
    }
}
