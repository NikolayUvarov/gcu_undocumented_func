//! Test-only stand-in for vfs_server (QEMU `block` suite): with the block clients init gives vfs_server (badged with
//! the write right), writes a pattern to 8 sectors near the end of each disk, flushes, reads them back and reports.
//! The harness then checks the disk image on the host. Never packaged into the normal OS.
#![no_std]
#![no_main]
use core::arch::asm;
#[path = "../common/abi.rs"]
mod abi;
use abi::*;

unsafe fn call(mb: *mut SyscallMailbox, number: usize, a: usize, b: usize) -> usize {
    (*mb).syscall_num = number;
    (*mb).arg1 = a;
    (*mb).arg2 = b;
    asm!("int 0x80");
    (*mb).result
}

unsafe fn print(mb: *mut SyscallMailbox, message: &[u8]) { call(mb, SYSCALL_LOG, message.as_ptr() as usize, message.len()); }

// A CALL on `endpoint` with two words and an optional capability; the reply's two words.
unsafe fn ipc(mb: *mut SyscallMailbox, endpoint: usize, data: [usize; 2], cap: usize) -> [usize; 2] {
    (*mb).msg = [cap, 0, data[0], data[1]];
    if call(mb, SYSCALL_IPC_CALL, endpoint, 0) != 0 { return [ERR_PEER, 0]; }
    [(*mb).msg[2], (*mb).msg[3]]
}

struct Line { bytes: [u8; 160], len: usize }
impl Line {
    fn text(&mut self, text: &[u8]) -> &mut Self { for &b in text { if self.len < self.bytes.len() { self.bytes[self.len] = b; self.len += 1; } } self }
    fn number(&mut self, mut value: usize) -> &mut Self {
        let mut digits = [0u8; 20]; let mut n = 0;
        loop { digits[n] = b'0' + (value % 10) as u8; n += 1; value /= 10; if value == 0 { break; } }
        for i in (0..n).rev() { self.text(&[digits[i]]); }
        self
    }
}

// Sector `s` of the pattern for drive kind `kind`: a readable header, then bytes that differ per sector and kind.
fn pattern(kind: usize, s: usize, out: &mut [u8]) {
    for (j, byte) in out.iter_mut().enumerate() { *byte = (j ^ s * 31 ^ kind * 77) as u8; }
    let header = b"MIND BLOCK WRITE TEST KIND=";
    out[..header.len()].copy_from_slice(header);
    out[header.len()] = b'0' + kind as u8;
    out[header.len() + 1] = b' ';
    out[header.len() + 2] = b'0' + s as u8;
}

#[no_mangle]
#[link_section = ".text._start"]
pub extern "sysv64" fn _start(_: &BootInfo, mb: *mut SyscallMailbox) {
    unsafe {
        print(mb, b"[BLOCKTEST] START\n");
        let buffer = call(mb, SYSCALL_ALLOC, 8 * BLOCK_SECTOR, 0);
        let shared = call(mb, SYSCALL_MEM_SHARE, buffer, 0);
        if buffer == 0 || shared >= ERR_FIRST { print(mb, b"[BLOCKTEST] NO MEMORY\n"); return; }
        let data = core::slice::from_raw_parts_mut(buffer as *mut u8, 8 * BLOCK_SECTOR);
        for slot in SLOT_BLOCK_FIRST..SLOT_BLOCK_FIRST + BLOCK_DEVICES {
            if call(mb, SYSCALL_CAP_INFO, slot, 0) != CAP_KIND_ENDPOINT { continue; }
            let badge = (*mb).arg2;
            let info = ipc(mb, slot, [BLOCK_INFO, 0], 0);
            let mut line = Line { bytes: [0; 160], len: 0 };
            line.text(b"[BLOCKTEST] SLOT=").number(slot).text(b" BADGE=").number(badge);
            if info[0] >= ERR_FIRST { line.text(b" NO DEVICE\n"); print(mb, &line.bytes[..line.len]); continue; }
            let (sectors, kind) = (info[0], info[1] & 0xFF);
            line.text(b" KIND=").number(kind).text(b" SECTORS=").number(sectors).text(if info[1] & BLOCK_INFO_READ_ONLY != 0 { b" READ-ONLY" } else { b" WRITABLE" });
            let attached = ipc(mb, slot, [BLOCK_ATTACH, 0], shared)[0] == 0;
            // Each kind writes its own 8 sectors at the end of the disk (outside the test image's file system).
            let lba = sectors - 8 * kind;
            for s in 0..8 { pattern(kind, s, &mut data[s * BLOCK_SECTOR..(s + 1) * BLOCK_SECTOR]); }
            let written = ipc(mb, slot, [BLOCK_WRITE | 8 << 8, lba], 0)[0];
            let flushed = ipc(mb, slot, [BLOCK_FLUSH, 0], 0)[0];
            data.fill(0);
            let read = ipc(mb, slot, [BLOCK_READ | 8 << 8, lba], 0)[0];
            let mut expected = [0u8; BLOCK_SECTOR];
            let same = (0..8).all(|s| { pattern(kind, s, &mut expected); data[s * BLOCK_SECTOR..(s + 1) * BLOCK_SECTOR] == expected });
            line.text(b" LBA=").number(lba).text(if attached { b" ATTACH=OK" } else { b" ATTACH=FAIL" })
                .text(if written == 8 { b" WRITE=OK" } else { b" WRITE=FAIL" }).text(if flushed == 0 { b" FLUSH=OK" } else { b" FLUSH=FAIL" })
                .text(if read == 8 && same { b" READBACK=OK\n" } else { b" READBACK=FAIL\n" });
            print(mb, &line.bytes[..line.len]);
        }
        print(mb, b"[BLOCKTEST] DONE\n");
        // Stays as the service, refusing every request, so that its log can be read and clients do not wait.
        loop {
            if call(mb, SYSCALL_IPC_RECV, SLOT_SERVICE, 9) != 0 { continue; }
            let (cap, is_call) = ((*mb).msg[0] != 0, (*mb).msg[1] & MSG_FLAG_CALL != 0);
            if is_call { (*mb).msg = [0, 0, ERR_INVALID, 0]; call(mb, SYSCALL_IPC_REPLY, 0, 0); }
            if cap { call(mb, SYSCALL_CAP_DROP, 9, 0); }
        }
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! { loop {} }
