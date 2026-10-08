#![no_std]
#![no_main]
// df: the mounted volumes with their FAT type, cluster size, size, used and free space (KiB), from vfs_server.
// A console program.
use mind::abi::BootInfo;

mind::request!(REQUEST_CONSOLE);

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("df — the volumes: FAT type, cluster size, size, used and free space (KiB).\nUsage: df");
    mind::println!("VOLUME  LABEL        TYPE   CLUSTER    SIZE KB    USED KB    FREE KB  USE");
    for (name, shown) in [("", "A:"), ("ram", "ram:"), ("models", "models:")] {
        match mind::fs::volume(name) {
            Err(mind::fs::Error::NotFound) if name == "models" => {} // no model disk (251)
            Ok(v) => {
                let used = v.bytes - v.free.min(v.bytes);
                let percent = if v.bytes == 0 { 0 } else { (used * 100).div_ceil(v.bytes) };
                mind::println!("{:<7} {:<12} FAT{:<2} {:>8} {:>10} {:>10} {:>10} {:>3}%", shown, v.label(), v.fat_bits, v.cluster, v.bytes / 1024, used / 1024, v.free / 1024, percent);
            }
            Err(error) => mind::println!("{:<7} not mounted ({:?})", shown, error),
        }
    }
}
