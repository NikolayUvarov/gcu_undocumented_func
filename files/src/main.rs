#![no_std]
#![no_main]
// VFS demo: lists the disk root and reads a file via vfs_server.
use core::fmt::Write;
use mind::abi::BootInfo;
use mind::fs::{self, File};
use mind::gfx::Screen;
use mind::util::FixedBuf;

const BACKGROUND: u32 = 0x00101820; const TEXT: u32 = 0x00E0E0E0; const ACCENT: u32 = 0x0080D0FF;

mind::entry!(main);
fn main(info: &'static BootInfo) {
    let screen = Screen::new(info);
    if let Some(s) = screen { s.clear(BACKGROUND); s.text(24, 24, b"FILES - VFS SERVER DEMO (ESC: EXIT)", 2, ACCENT, None); }
    let mut y = 64;
    let mut line = |text: &[u8]| { mind::process::log(text); mind::process::log(b"\n"); if let Some(s) = screen { s.text(24, y, text, 1, TEXT, None); y += 12; } };

    let mut out = FixedBuf::<96>::new();
    let listed = fs::list("", |entry| {
        out.clear();
        let _ = write!(out, "[FILES] {} {}{}", core::str::from_utf8(entry.name).unwrap_or("?"), entry.size, if entry.is_dir { " <DIR>" } else { "" });
        line(out.as_bytes());
    });
    match listed {
        Ok(count) => { out.clear(); let _ = write!(out, "[FILES] {} ENTRIES IN /", count); line(out.as_bytes()); }
        Err(error) => { out.clear(); let _ = write!(out, "[FILES] LIST FAILED: {:?}", error); line(out.as_bytes()); }
    }

    // Read the whole ELF in chunks and compute a simple checksum.
    for path in ["kernel.elf", "EFI/BOOT/BOOTX64.EFI"] {
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
    loop { mind::input::wait_or_exit(200); }
}
