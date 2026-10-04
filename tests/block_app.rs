//! Test-only stand-in for vfs_server (QEMU `block` suite): with the block clients init gives vfs_server (badged with
//! the write right), writes a pattern to 8 sectors near the end of each disk as sealed read-only memory, flushes, reads
//! them back and reports. The harness then checks the disk image on the host. Never packaged into the normal OS.
//! The calls of idl/block.wit 1.1 are encoded by hand (word calls: method | major << 8 | fields << 16).
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

// A CALL on `endpoint` with two words and an optional capability (moved when `moved`); the reply's two words.
unsafe fn ipc(mb: *mut SyscallMailbox, endpoint: usize, data: [usize; 2], cap: usize, moved: bool) -> [usize; 2] {
    (*mb).msg = [cap, 0xFF | if moved { CAP_TRANSFER_MOVE } else { 0 }, data[0], data[1]];
    if call(mb, SYSCALL_IPC_CALL, endpoint, 0) != 0 { return [usize::MAX, 0]; }
    [(*mb).msg[2], (*mb).msg[3]]
}

// idl/block.wit 1.1, major version 1.
const SECTORS: usize = 1 | 1 << 8; const KIND: usize = 2 | 1 << 8; const ATTACH: usize = 3 | 1 << 8; const READ: usize = 4 | 1 << 8;
const WRITABLE: usize = 5 | 1 << 8; const WRITE: usize = 6 | 1 << 8; const FLUSH: usize = 7 | 1 << 8;
fn ok(reply: [usize; 2]) -> bool { reply[0] & 0xFF == 0 }

// A sealed read-only copy of `data`: a detached memory object, a read-only child, the writable object dropped.
unsafe fn sealed(mb: *mut SyscallMailbox, data: &[u8]) -> usize {
    let block = call(mb, SYSCALL_ALLOC, data.len(), 0);
    if block == 0 { return ERR_NO_MEMORY; }
    core::slice::from_raw_parts_mut(block as *mut u8, data.len()).copy_from_slice(data);
    let object = call(mb, SYSCALL_MEM_DETACH, block, 0);
    (*mb).msg = [0; 4];
    let child = call(mb, SYSCALL_CAP_MINT, object, CAP_READ as usize);
    call(mb, SYSCALL_CAP_DROP, object, 0);
    child
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
            let info = ipc(mb, slot, [SECTORS, 0], 0, false);
            let mut line = Line { bytes: [0; 160], len: 0 };
            line.text(b"[BLOCKTEST] SLOT=").number(slot).text(b" BADGE=").number(badge);
            if !ok(info) { line.text(b" NO DEVICE\n"); print(mb, &line.bytes[..line.len]); continue; }
            let sectors = info[1];
            let kind = (ipc(mb, slot, [KIND, 0], 0, false)[0] >> 16) & 0xFF;
            let writable = ipc(mb, slot, [WRITABLE, 0], 0, false)[0] >> 16 & 1 != 0;
            line.text(b" KIND=").number(kind).text(b" SECTORS=").number(sectors).text(if writable { b" WRITABLE" } else { b" READ-ONLY" });
            let attached = ok(ipc(mb, slot, [ATTACH, 0], shared, false));
            // Each kind writes its own 8 sectors at the end of the disk (outside the test image's file system).
            let lba = sectors - 8 * kind;
            let mut pattern_data = [0u8; 8 * BLOCK_SECTOR];
            for s in 0..8 { pattern(kind, s, &mut pattern_data[s * BLOCK_SECTOR..(s + 1) * BLOCK_SECTOR]); }
            let copy = sealed(mb, &pattern_data);
            let written = ipc(mb, slot, [WRITE | 8 << 16, lba], copy, true);
            // Unsealed memory is refused: the client's own transfer buffer is still writable by it.
            let unsealed = ipc(mb, slot, [WRITE | 1 << 16, lba], call(mb, SYSCALL_MEM_SHARE, call(mb, SYSCALL_ALLOC, 4096, 0), 0), true);
            let flushed = ipc(mb, slot, [FLUSH, 0], 0, false);
            data.fill(0);
            let read = ipc(mb, slot, [READ | 8 << 16, lba], 0, false);
            let mut expected = [0u8; BLOCK_SECTOR];
            let same = (0..8).all(|s| { pattern(kind, s, &mut expected); data[s * BLOCK_SECTOR..(s + 1) * BLOCK_SECTOR] == expected });
            line.text(b" LBA=").number(lba).text(if attached { b" ATTACH=OK" } else { b" ATTACH=FAIL" })
                .text(if ok(written) && written[0] >> 16 == 8 { b" WRITE=OK" } else { b" WRITE=FAIL" })
                .text(if unsealed[0] & 0xFF == 2 && unsealed[1] == ERR_INVALID { b" UNSEALED=REFUSED" } else { b" UNSEALED=ACCEPTED" })
                .text(if ok(flushed) { b" FLUSH=OK" } else { b" FLUSH=FAIL" })
                .text(if ok(read) && read[0] >> 16 == 8 && same { b" READBACK=OK\n" } else { b" READBACK=FAIL\n" });
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
