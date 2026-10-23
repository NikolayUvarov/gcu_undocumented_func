#![no_std]
#![no_main]
// memstack: test program of a stack larger than 64 KiB (262-KRN-0164). Its ELF asks for 4 MiB (build.rs); `memstack
// <KiB>` recurses through that much of it and prints the sum, so more than 4 MiB faults at the guard page below.
use mind::abi::BootInfo;

mind::request!(REQUEST_CONSOLE);

// Recurses through `kib` frames of 1 KiB each; the frames stay live across each call, so none is folded away.
#[inline(never)]
fn deep(kib: usize) -> u64 {
    let mut frame = [0u8; 1024];
    frame[kib % 1024] = kib as u8;
    core::hint::black_box(&mut frame);
    if kib == 0 { return 0; }
    deep(kib - 1).wrapping_add(frame[kib % 1024] as u64)
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("memstack — test program of a 4 MiB stack: recurses through as many KiB of it as asked.\nUsage: memstack <KiB>");
    let Some(kib) = mind::process::args_str().split_whitespace().next().and_then(|s| s.parse::<usize>().ok()) else {
        mind::println!("USAGE: MEMSTACK <KiB>");
        return;
    };
    mind::println!("[MEMSTACK] {} KiB OF STACK USED, SUM={}", kib, deep(kib));
}
