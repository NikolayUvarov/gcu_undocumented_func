use crate::{abi::BootInfo, memory::Region, paging};
use core::arch::{asm, global_asm};
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

pub const MAX: usize = 8;
pub static COUNT: AtomicUsize = AtomicUsize::new(1);
static LAPIC: AtomicUsize = AtomicUsize::new(0xfee00000);
pub static ONLINE: [AtomicBool; MAX] = [const { AtomicBool::new(false) }; MAX];
pub static TICKS: [AtomicU64; MAX] = [const { AtomicU64::new(0) }; MAX];

#[repr(C, align(16))]
struct Cpu {
    gdt: [u64; 7],
    tss: [u8; 104],
    idle_stack: usize,
    apic_id: u32,
}
static mut CPUS: [Cpu; MAX] = [const {
    Cpu {
        gdt: [0; 7],
        tss: [0; 104],
        idle_stack: 0,
        apic_id: 0,
    }
}; MAX];

unsafe fn cpu(index: usize) -> *mut Cpu {
    core::ptr::addr_of_mut!(CPUS).cast::<Cpu>().add(index)
}

pub fn id() -> usize {
    let apic = unsafe { read(0x20) >> 24 };
    (0..COUNT.load(Ordering::Acquire))
        .find(|&i| unsafe { (*cpu(i)).apic_id == apic })
        .unwrap_or(0)
}
pub fn apic_id(index: usize) -> u32 {
    unsafe { (*cpu(index)).apic_id }
}
pub unsafe fn read(register: usize) -> u32 {
    core::ptr::read_volatile((LAPIC.load(Ordering::Relaxed) + register) as *const u32)
}
pub unsafe fn write(register: usize, value: u32) {
    core::ptr::write_volatile(
        (LAPIC.load(Ordering::Relaxed) + register) as *mut u32,
        value,
    );
    let _ = read(0x20);
}
pub unsafe fn eoi() {
    write(0xb0, 0);
}

pub unsafe fn prepare(info: &BootInfo) -> Result<(), &'static str> {
    assert!((1..=MAX).contains(&info.cpu_count));
    COUNT.store(info.cpu_count, Ordering::Release);
    let lo: u32;
    let hi: u32;
    asm!("rdmsr", in("ecx") 0x1bu32, out("eax") lo, out("edx") hi);
    assert_eq!(hi, 0, "LAPIC must be below 4 GiB");
    assert_eq!(lo & (1 << 10), 0, "x2APIC is not supported yet");
    LAPIC.store((lo as usize) & 0xfffff000, Ordering::Release);
    for i in 0..info.cpu_count {
        assert!(info.apic_ids[i] < 256, "xAPIC ID out of range");
        let c = &mut *cpu(i);
        c.apic_id = info.apic_ids[i];
        c.gdt = [
            0,
            0x00af9a000000ffff,
            0x00cf92000000ffff,
            0x00cff2000000ffff,
            0x00affa000000ffff,
            0,
            0,
        ];
        let stack = Region::new(64 * 1024, 16)?;
        core::ptr::write_unaligned(
            c.tss.as_mut_ptr().add(4).cast::<u64>(),
            stack.ptr() as u64 + 64 * 1024,
        );
        core::mem::forget(stack);
        for ist in 0..2 {
            let stack = Region::new(16 * 1024, 16)?;
            core::ptr::write_unaligned(
                c.tss.as_mut_ptr().add(36 + ist * 8).cast::<u64>(),
                stack.ptr() as u64 + 16 * 1024,
            );
            core::mem::forget(stack);
        }
        core::ptr::write_unaligned(c.tss.as_mut_ptr().add(102).cast::<u16>(), 104);
        let base = c.tss.as_ptr() as u64;
        c.gdt[5] = 103 | ((base & 0xffffff) << 16) | (0x89 << 40) | (((base >> 24) & 255) << 56);
        c.gdt[6] = base >> 32;
        let stack = Region::new(64 * 1024, 16)?;
        c.idle_stack = stack.ptr() as usize + stack.len() - 8;
        core::mem::forget(stack);
    }
    load(0);
    lapic_init(true);
    ONLINE[0].store(true, Ordering::Release);
    Ok(())
}

pub unsafe fn load(index: usize) {
    #[repr(C, packed)]
    struct Pointer {
        limit: u16,
        base: u64,
    }
    let c = &*cpu(index);
    let pointer = Pointer {
        limit: 55,
        base: c.gdt.as_ptr() as u64,
    };
    asm!("lgdt [{}]", in(reg) &pointer);
    asm!("push 8", "lea rax, [rip + 2f]", "push rax", "retfq", "2:",
         "mov ax, 0x10", "mov ds, ax", "mov es, ax", "mov ss, ax",
         "xor eax, eax", "mov fs, ax", "mov gs, ax", out("rax") _);
    asm!("ltr ax", in("ax") 0x28u16);
    for msr in [0xc0000100u32, 0xc0000101, 0xc0000102] {
        asm!("wrmsr", in("ecx") msr, in("eax") 0u32, in("edx") 0u32);
    }
}

pub unsafe fn lapic_init(bsp: bool) {
    let mut lo: u32;
    let hi: u32;
    asm!("rdmsr", in("ecx") 0x1bu32, out("eax") lo, out("edx") hi);
    lo |= 1 << 11;
    asm!("wrmsr", in("ecx") 0x1bu32, in("eax") lo, in("edx") hi);
    write(0x80, 0); // TPR: accept all priorities.
    write(0xf0, 0x1ff); // enabled, spurious vector 255
    for register in [0x320, 0x330, 0x340, 0x360, 0x370] {
        write(register, 1 << 16);
    }
    write(0x350, if bsp { 7 << 8 } else { 1 << 16 }); // BSP PIC via ExtINT
    eoi();
}

pub unsafe fn ipi(target: u32, command: u32) {
    crate::interrupts::without(|| {
        for _ in 0..1_000_000 {
            if read(0x300) & (1 << 12) == 0 {
                write(0x310, target << 24);
                write(0x300, command);
                return;
            }
            core::hint::spin_loop();
        }
        panic!("LAPIC IPI timeout");
    });
}

pub unsafe fn tick_others() {
    for i in 1..COUNT.load(Ordering::Acquire) {
        if ONLINE[i].load(Ordering::Acquire) {
            ipi(apic_id(i), 0x30);
        }
    }
}

pub fn halt_all() -> ! {
    unsafe {
        asm!("cli");
        let this = id();
        for i in 0..COUNT.load(Ordering::Acquire) {
            if i != this && ONLINE[i].load(Ordering::Acquire) {
                ipi(apic_id(i), 0x31);
            }
        }
        loop {
            asm!("hlt");
        }
    }
}

fn delay(ms: u64) {
    let start = crate::interrupts::milliseconds();
    while crate::interrupts::milliseconds().wrapping_sub(start) < ms {
        unsafe {
            asm!("sti", "hlt");
        }
    }
}

// Position independent real-mode bootstrap, copied to a UEFI-reserved page.
// Only label differences are assembled; runtime physical addresses are patched.
global_asm!(
    r#"
    .section .text.ap_boot,"ax"
    .code16
    .global ap_boot_start, ap_boot_end, ap_boot_gdt, ap_boot_gdtr, ap_boot_far
    .global ap_boot_cr3, ap_boot_stack, ap_boot_index, ap_boot_entry, ap_boot_long
ap_boot_start:
    cli
    cld
    mov ax, cs
    mov ds, ax
    .set gdtr_offset, ap_boot_gdtr - ap_boot_start
    .set cr3_offset, ap_boot_cr3 - ap_boot_start
    lgdt [gdtr_offset]
    mov eax, [cr3_offset]
    mov cr3, eax
    mov eax, cr4
    or eax, 0x620
    mov cr4, eax
    mov ecx, 0xc0000080
    rdmsr
    or eax, 0x900
    wrmsr
    mov eax, cr0
    and eax, 0xfffffff3
    or eax, 0x80010003
    mov cr0, eax
    .byte 0x66, 0xff, 0x2e
    .word ap_boot_far - ap_boot_start
    .code64
ap_boot_long:
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov rsp, [rip + ap_boot_stack]
    mov rdi, [rip + ap_boot_index]
    mov rax, [rip + ap_boot_entry]
    jmp rax
    .balign 8
ap_boot_gdt:
    .quad 0, 0x00af9a000000ffff, 0x00cf92000000ffff
ap_boot_gdtr:
    .word 23
    .long 0
ap_boot_far:
    .long 0
    .word 8
ap_boot_cr3:
    .quad 0
ap_boot_stack:
    .quad 0
ap_boot_index:
    .quad 0
ap_boot_entry:
    .quad 0
ap_boot_end:
    .previous
"#
);
unsafe extern "C" {
    static ap_boot_start: u8;
    static ap_boot_end: u8;
    static ap_boot_gdt: u8;
    static ap_boot_gdtr: u8;
    static ap_boot_far: u8;
    static ap_boot_cr3: u8;
    static ap_boot_stack: u8;
    static ap_boot_index: u8;
    static ap_boot_entry: u8;
    static ap_boot_long: u8;
}

pub unsafe fn start(info: &BootInfo) {
    let source = core::ptr::addr_of!(ap_boot_start) as usize;
    let size = core::ptr::addr_of!(ap_boot_end) as usize - source;
    assert!(size < 4096 && info.ap_trampoline < 0x100000);
    let base = info.ap_trampoline;
    core::ptr::copy_nonoverlapping(source as *const u8, base as *mut u8, size);
    let target = |symbol: *const u8| base + symbol as usize - source;
    ((target(core::ptr::addr_of!(ap_boot_gdtr)) + 2) as *mut u32)
        .write_unaligned(target(core::ptr::addr_of!(ap_boot_gdt)) as u32);
    (target(core::ptr::addr_of!(ap_boot_far)) as *mut u32)
        .write_unaligned(target(core::ptr::addr_of!(ap_boot_long)) as u32);
    (target(core::ptr::addr_of!(ap_boot_cr3)) as *mut u64)
        .write_unaligned(paging::kernel_root() as u64);
    (target(core::ptr::addr_of!(ap_boot_entry)) as *mut u64)
        .write_unaligned(ap_entry as *const () as u64);
    for i in 1..info.cpu_count {
        (target(core::ptr::addr_of!(ap_boot_stack)) as *mut u64)
            .write_unaligned((*cpu(i)).idle_stack as u64);
        (target(core::ptr::addr_of!(ap_boot_index)) as *mut u64).write_unaligned(i as u64);
        ipi(apic_id(i), 0xc500);
        delay(10);
        ipi(apic_id(i), 0x8500);
        delay(10);
        ipi(apic_id(i), 0x600 | (base >> 12) as u32);
        delay(10);
        if !ONLINE[i].load(Ordering::Acquire) {
            ipi(apic_id(i), 0x600 | (base >> 12) as u32);
        }
        let start = crate::interrupts::milliseconds();
        while !ONLINE[i].load(Ordering::Acquire) {
            assert!(
                crate::interrupts::milliseconds() - start < 1000,
                "AP startup timeout"
            );
            delay(10);
        }
    }
}

extern "C" fn ap_entry(index: usize) -> ! {
    unsafe {
        paging::enable_protection();
        load(index);
        crate::interrupts::load();
        lapic_init(false);
        ONLINE[index].store(true, Ordering::Release);
        asm!("sti");
    }
    loop {
        crate::scheduler::idle();
    }
}
