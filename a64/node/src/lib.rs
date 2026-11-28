//! One MIND Core aarch64 program on the A64 emulator in Node (215-PRT-0016, step 0 of 215): a stand-in for the kernel.
//! It loads a static PIE at USER_IMAGE and applies its relocations as the kernel does, gives it an information page
//! and a mailbox, answers LOG, UPTIME, EXIT, ALLOC and FREE, and refuses every other system call, naming it to the host.
//! A program linked at a fixed address (the differential one) runs until its first `svc`, which hands its results over.
#![no_std]

mod layout;
use a64::{run, Access, Bus, Cpu, Exit, Fault, FaultKind};
use core::ptr::addr_of_mut;
use layout::*;

#[link(wasm_import_module = "env")]
extern "C" {
    fn host_log(text: *const u8, len: usize);
    fn host_now_ms() -> f64;
    fn host_refused(number: u64);
    fn host_results(words: *const u64, count: usize);
}

static mut ELF: [u8; 16 << 20] = [0; 16 << 20];
static mut IMAGE_MEMORY: [u8; IMAGE_BYTES] = [0; IMAGE_BYTES];
static mut LOW_MEMORY: [u8; LOW_BYTES] = [0; LOW_BYTES];
static mut STACK_MEMORY: [u8; STACK_BYTES] = [0; STACK_BYTES];
static mut PAGE_MEMORY: [u8; 8192] = [0; 8192];
static mut HEAP_MEMORY: [u8; HEAP_BYTES] = [0; HEAP_BYTES];
static mut HEAP_USED: u64 = 0;
static mut CPU: Cpu = Cpu::new();
static mut BARE: bool = false;
static mut EXECUTED: u64 = 0;
static mut TEXT: [u8; 4096] = [0; 4096];

struct Memory;

impl Memory {
    fn region(&mut self, va: u64, size: u32) -> Option<&'static mut [u8]> {
        let regions: [(u64, &'static mut [u8]); 5] = unsafe {
            [
                (IMAGE, &mut *addr_of_mut!(IMAGE_MEMORY)),
                (LOW, &mut *addr_of_mut!(LOW_MEMORY)),
                (STACK, &mut *addr_of_mut!(STACK_MEMORY)),
                (PAGES, &mut *addr_of_mut!(PAGE_MEMORY)),
                (HEAP, &mut *addr_of_mut!(HEAP_MEMORY)),
            ]
        };
        for (base, bytes) in regions {
            let offset = va.wrapping_sub(base);
            if offset < bytes.len() as u64 && offset + size as u64 <= bytes.len() as u64 {
                return Some(&mut bytes[offset as usize..offset as usize + size as usize]);
            }
        }
        None
    }
}

impl Bus for Memory {
    fn read(&mut self, va: u64, size: u32, access: Access) -> Result<u64, Fault> {
        let bytes = self.region(va, size).ok_or(Fault { address: va, access, kind: FaultKind::Translation(3) })?;
        let mut word = [0u8; 8];
        word[..size as usize].copy_from_slice(bytes);
        Ok(u64::from_le_bytes(word))
    }
    fn write(&mut self, va: u64, size: u32, value: u64) -> Result<(), Fault> {
        let bytes = self.region(va, size).ok_or(Fault { address: va, access: Access::Write, kind: FaultKind::Translation(3) })?;
        bytes.copy_from_slice(&value.to_le_bytes()[..size as usize]);
        Ok(())
    }
    fn counter(&mut self) -> u64 { unsafe { (host_now_ms() * 1e6) as u64 } }
    fn frequency(&mut self) -> u64 { 1_000_000_000 }
}

#[no_mangle]
pub extern "C" fn elf_buffer() -> *mut u8 { addr_of_mut!(ELF) as *mut u8 }

fn word(elf: &[u8], at: usize) -> u64 { u64::from_le_bytes(elf[at..at + 8].try_into().unwrap()) }
fn half(elf: &[u8], at: usize) -> usize { u16::from_le_bytes([elf[at], elf[at + 1]]) as usize }

/// Loads the ELF file of `len` bytes from `elf_buffer`: 0, or a negative reason.
#[no_mangle]
pub extern "C" fn load(len: usize) -> i32 {
    let elf: &[u8] = unsafe { &(&*addr_of_mut!(ELF))[..len] };
    if len < 64 || &elf[..4] != b"\x7fELF" || half(elf, 18) != 183 { return -1; }
    let pie = half(elf, 16) == 3;
    let base = if pie { IMAGE } else { 0 };
    let mut memory = Memory;
    let (phoff, phentsize, phnum) = (word(elf, 32) as usize, half(elf, 54), half(elf, 56));
    let mut dynamic = None;
    for i in 0..phnum {
        let p = phoff + i * phentsize;
        let kind = u32::from_le_bytes(elf[p..p + 4].try_into().unwrap());
        let (offset, vaddr, filesz, memsz) = (word(elf, p + 8) as usize, word(elf, p + 16), word(elf, p + 32), word(elf, p + 40));
        if kind == 2 { dynamic = Some((offset, word(elf, p + 32) as usize)); }
        if kind != 1 { continue; }
        for b in 0..memsz {
            let value = if b < filesz { elf[offset + b as usize] as u64 } else { 0 };
            if memory.write(base + vaddr + b, 1, value).is_err() { return -2; }
        }
    }
    // R_AARCH64_RELATIVE alone, as the kernel's loader (bootloader/src/elf_reloc.rs).
    if let Some((at, size)) = dynamic {
        let (mut rela, mut relasz) = (None, 0);
        for d in (at..at + size).step_by(16) {
            match word(elf, d) { 7 => rela = Some(word(elf, d + 8)), 8 => relasz = word(elf, d + 8), _ => {} }
        }
        if let Some(rela) = rela {
            for r in (0..relasz).step_by(24) {
                let entry = base + rela + r;
                let (Ok(offset), Ok(info), Ok(addend)) =
                    (memory.read(entry, 8, Access::Read), memory.read(entry + 8, 8, Access::Read), memory.read(entry + 16, 8, Access::Read))
                else { return -3 };
                if info != 1027 { return -4; }
                if memory.write(base + offset, 8, base.wrapping_add(addend)).is_err() { return -5; }
            }
        }
    }
    let _ = memory.write(PAGES + BOOT_INFO_ABI_VERSION as u64, 4, ABI_VERSION as u64);
    let cpu = unsafe { &mut *addr_of_mut!(CPU) };
    *cpu = Cpu::new();
    cpu.pc = base + word(elf, 24);
    cpu.sp = STACK + STACK_BYTES as u64 - 64;
    cpu.x[0] = PAGES;
    cpu.x[1] = PAGES + 4096;
    unsafe { BARE = !pie; EXECUTED = 0; HEAP_USED = 0; }
    0
}

/// Runs the program to its end: 0 at EXIT (or a bare program's first `svc`), else a negative reason; `pc` then names it.
#[no_mangle]
pub extern "C" fn go() -> i32 {
    let (cpu, mut memory) = (unsafe { &mut *addr_of_mut!(CPU) }, Memory);
    loop {
        let (exit, done) = run(cpu, &mut memory, 50_000_000);
        unsafe { EXECUTED += done; }
        match exit {
            Exit::Budget => continue,
            Exit::Svc(_) if unsafe { BARE } => {
                let mut words = [0u64; 64];
                let count = (cpu.x[1] as usize).min(64);
                for (i, w) in words.iter_mut().take(count).enumerate() { *w = memory.read(cpu.x[0] + 8 * i as u64, 8, Access::Read).unwrap_or(0); }
                unsafe { host_results(words.as_ptr(), count) };
                return 0;
            }
            Exit::Svc(_) => {
                let mailbox = PAGES + 4096; // the page after the information page, as libmind's info_address has it
                let field = |memory: &mut Memory, at: usize| memory.read(mailbox + at as u64, 8, Access::Read).unwrap_or(0);
                let (number, arg1, arg2) = (field(&mut memory, MAILBOX_NUMBER), field(&mut memory, MAILBOX_ARG1), field(&mut memory, MAILBOX_ARG2));
                let result = match number {
                    SYSCALL_LOG => {
                        let text = unsafe { &mut *addr_of_mut!(TEXT) };
                        let len = (arg2 as usize).min(text.len());
                        for (i, b) in text.iter_mut().take(len).enumerate() { *b = memory.read(arg1 + i as u64, 1, Access::Read).unwrap_or(b'?' as u64) as u8; }
                        unsafe { host_log(text.as_ptr(), len) };
                        len as u64
                    }
                    SYSCALL_UPTIME => unsafe { host_now_ms() as u64 },
                    SYSCALL_EXIT => return 0,
                    SYSCALL_ALLOC => {
                        let bytes = (arg1 + 4095) & !4095;
                        let used = unsafe { HEAP_USED };
                        if bytes == 0 || used + bytes > HEAP_BYTES as u64 { 0 } else { unsafe { HEAP_USED = used + bytes; } HEAP + used }
                    }
                    SYSCALL_FREE => 0,
                    other => { unsafe { host_refused(other) }; ERR_INVALID }
                };
                let _ = memory.write(mailbox + MAILBOX_RESULT as u64, 8, result);
            }
            Exit::Fault(_) => return -1,
            Exit::Undefined(_) => return -2,
            Exit::Brk(_) => return -3,
            Exit::Wait => continue,
        }
    }
}

#[no_mangle]
pub extern "C" fn executed() -> f64 { unsafe { EXECUTED as f64 } }
#[no_mangle]
pub extern "C" fn pc() -> f64 { unsafe { (*addr_of_mut!(CPU)).pc as f64 } }

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! { core::arch::wasm32::unreachable() }
