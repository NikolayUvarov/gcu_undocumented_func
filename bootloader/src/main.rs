#![no_std]
#![no_main]
use core::fmt::Write;
use core::panic::PanicInfo;
use uefi::prelude::*;
use uefi::proto::console::gop::{GraphicsOutput, Mode, ModeInfo, PixelFormat};
use uefi::proto::media::file::{File, FileAttribute, FileMode, FileType};
use uefi::proto::media::fs::SimpleFileSystem;
#[cfg(target_arch = "x86_64")]
use uefi::proto::pi::mp::MpServices;
use uefi::table::boot::{AllocateType, BootServices, MemoryType};
#[path = "../../common/abi.rs"] mod abi;
use abi::{BootInfo, ProgramImage, StatPhys, PIXEL_BGR, PIXEL_BITMASK, PIXEL_RGB}; mod elf_reloc;

const MEMORY_MAP_PAGES: usize = 16; // firmware memory map copied for the kernel (STAT PHYSMAP)

const FILE_BUFFER_PAGES: usize = 1024; // 4 MiB: the largest boot image

// Boot errors go to the serial line (COM1, or the PL011 of QEMU's aarch64 `virt`) and the UEFI console, then the
// machine stops: never a silent hang.
struct Serial;
impl Write for Serial {
    fn write_str(&mut self, text: &str) -> core::fmt::Result {
        for byte in text.bytes() {
            unsafe {
                #[cfg(target_arch = "x86_64")] {
                    for _ in 0..100_000 { let status: u8; core::arch::asm!("in al, dx", out("al") status, in("dx") 0x3FDu16); if status & 0x20 != 0 { break; } }
                    core::arch::asm!("out dx, al", in("al") byte, in("dx") 0x3F8u16);
                }
                #[cfg(target_arch = "aarch64")] {
                    const PL011: usize = 0x0900_0000;
                    for _ in 0..100_000 { if core::ptr::read_volatile((PL011 + 0x18) as *const u32) & 0x20 == 0 { break; } }
                    core::ptr::write_volatile(PL011 as *mut u32, byte as u32);
                }
            }
        }
        Ok(())
    }
}
// A mode the compositor can draw into: a linear framebuffer with 32-bit pixels.
fn drawable(info: &ModeInfo) -> bool {
    match (info.pixel_format(), info.pixel_bitmask()) {
        (PixelFormat::Rgb | PixelFormat::Bgr, _) => true,
        (PixelFormat::Bitmask, Some(m)) => 32 - (m.red | m.green | m.blue | m.reserved).leading_zeros() > 24,
        _ => false,
    }
}

// Keeps the firmware's mode if it has a linear framebuffer, else switches to the largest such mode up to 1920x1200.
fn select_display(services: &BootServices) -> Result<(*mut u32, ModeInfo), &'static str> {
    let handle = services.get_handle_for_protocol::<GraphicsOutput>().map_err(|_| "no graphics output")?;
    let mut gop = services.open_protocol_exclusive::<GraphicsOutput>(handle).map_err(|_| "cannot open graphics output")?;
    if !drawable(&gop.current_mode_info()) {
        let area = |mode: &Mode| { let (w, h) = mode.info().resolution(); if w <= 1920 && h <= 1200 { w * h } else { 0 } };
        let best = gop.modes(services).filter(|mode| drawable(mode.info())).max_by_key(area).ok_or("no mode with a linear framebuffer (BltOnly)")?;
        gop.set_mode(&best).map_err(|_| "cannot set a mode with a linear framebuffer")?;
    }
    let fb_ptr = gop.frame_buffer().as_mut_ptr().cast::<u32>();
    Ok((fb_ptr, gop.current_mode_info()))
}

fn halt() -> ! {
    #[cfg(target_arch = "x86_64")] loop { unsafe { core::arch::asm!("cli; hlt"); } }
    #[cfg(target_arch = "aarch64")] loop { unsafe { core::arch::asm!("msr daifset, #0xf", "wfi"); } }
}
#[cfg(target_arch = "x86_64")] const MACHINE: u16 = 0x3E;
#[cfg(target_arch = "aarch64")] const MACHINE: u16 = 0xB7;
fn fail(system_table: &mut SystemTable<Boot>, file: &str, reason: &str) -> ! {
    let _ = writeln!(Serial, "\r\nBOOT ERROR: {}: {}\r", file, reason);
    let _ = writeln!(system_table.stdout(), "BOOT ERROR: {}: {}", file, reason);
    halt()
}

fn keep_program(services: &BootServices, data: &[u8]) -> ProgramImage { let address = services.allocate_pages(AllocateType::MaxAddress(0xffff_ffff), MemoryType::LOADER_DATA, data.len().div_ceil(4096)).unwrap(); unsafe { core::ptr::copy_nonoverlapping(data.as_ptr(), address as *mut u8, data.len()); } ProgramImage { data: address as *const u8, len: data.len() } }
#[repr(C)] struct Elf64_Ehdr { e_ident: [u8; 16], e_type: u16, e_machine: u16, e_version: u32, e_entry: u64, e_phoff: u64, e_shoff: u64, e_flags: u32, e_ehsize: u16, e_phentsize: u16, e_phnum: u16, e_shentsize: u16, e_shnum: u16, e_shstrndx: u16 }
#[repr(C)] struct Elf64_Phdr { p_type: u32, p_flags: u32, p_offset: u64, p_vaddr: u64, p_paddr: u64, p_filesz: u64, p_memsz: u64, p_align: u64 }

// Header and segment bounds are checked before anything is read through them.
fn check_elf(data: &[u8]) -> Result<(&Elf64_Ehdr, &[Elf64_Phdr], u64, u64), &'static str> {
    if data.len() < core::mem::size_of::<Elf64_Ehdr>() { return Err("file too short for an ELF header"); }
    let ehdr = unsafe { &*(data.as_ptr() as *const Elf64_Ehdr) };
    if ehdr.e_ident[0..4] != [0x7f, b'E', b'L', b'F'] { return Err("bad ELF magic"); }
    if ehdr.e_ident[4] != 2 || ehdr.e_ident[5] != 1 || ehdr.e_machine != MACHINE || !matches!(ehdr.e_type, 2 | 3) { return Err("not a little-endian executable for this processor"); }
    if ehdr.e_phentsize as usize != core::mem::size_of::<Elf64_Phdr>() || ehdr.e_phoff % 8 != 0 { return Err("bad program header table"); }
    let table_end = (ehdr.e_phnum as u64).checked_mul(ehdr.e_phentsize as u64).and_then(|size| size.checked_add(ehdr.e_phoff));
    if table_end.is_none_or(|end| end > data.len() as u64) { return Err("program headers outside the file"); }
    let phdrs = unsafe { core::slice::from_raw_parts(data.as_ptr().add(ehdr.e_phoff as usize) as *const Elf64_Phdr, ehdr.e_phnum as usize) };
    let (mut min_vaddr, mut max_vaddr) = (u64::MAX, 0);
    for phdr in phdrs.iter().filter(|p| p.p_type == 1) {
        let file_end = phdr.p_offset.checked_add(phdr.p_filesz);
        let memory_end = phdr.p_vaddr.checked_add(phdr.p_memsz);
        if file_end.is_none_or(|end| end > data.len() as u64) || phdr.p_filesz > phdr.p_memsz || memory_end.is_none() { return Err("segment outside the file"); }
        min_vaddr = min_vaddr.min(phdr.p_vaddr); max_vaddr = max_vaddr.max(memory_end.unwrap());
    }
    if min_vaddr >= max_vaddr || max_vaddr - min_vaddr > 256 * 1024 * 1024 { return Err("no loadable segment or image too large"); }
    if !(min_vaddr..max_vaddr).contains(&ehdr.e_entry) { return Err("entry point outside the image"); }
    Ok((ehdr, phdrs, min_vaddr, max_vaddr))
}

fn load_elf(boot_services: &BootServices, file_data: &[u8]) -> Result<u64, &'static str> {
    let (ehdr, phdrs, min_vaddr, max_vaddr) = check_elf(file_data)?;
    let total_size = max_vaddr - min_vaddr;
    let base_addr = boot_services.allocate_pages(AllocateType::MaxAddress(0xffff_ffff), MemoryType::LOADER_DATA, total_size as usize / 4096 + 1).map_err(|_| "out of memory")?;
    for phdr in phdrs.iter().filter(|p| p.p_type == 1) {
        let dest = (base_addr + phdr.p_vaddr - min_vaddr) as *mut u8;
        unsafe {
            core::ptr::copy_nonoverlapping(file_data.as_ptr().add(phdr.p_offset as usize), dest, phdr.p_filesz as usize);
            core::ptr::write_bytes(dest.add(phdr.p_filesz as usize), 0, (phdr.p_memsz - phdr.p_filesz) as usize); // .bss
        }
    }
    let image = unsafe { core::slice::from_raw_parts_mut(base_addr as *mut u8, total_size as usize) };
    for phdr in phdrs.iter().filter(|p| p.p_type == 2) { elf_reloc::apply(image, min_vaddr, base_addr, phdr.p_vaddr, phdr.p_filesz as usize).map_err(|_| "invalid ELF relocations")?; }
    Ok(base_addr + ehdr.e_entry - min_vaddr)
}

// The x86 AP start page below 1 MiB and the APIC IDs of the enabled CPUs (BSP first, from MP services).
#[cfg(target_arch = "x86_64")]
fn processors(boot_services: &BootServices) -> (usize, [u32; 8], usize) {
    let ap_trampoline = boot_services.allocate_pages(AllocateType::MaxAddress(0xFFFFF), MemoryType::LOADER_DATA, 1).expect("AP bootstrap") as usize;
    let mut apic_ids = [0u32; 8]; let mut cpu_count = 1; apic_ids[0] = core::arch::x86_64::__cpuid(1).ebx >> 24;
    if let Ok(handle) = boot_services.get_handle_for_protocol::<MpServices>() {
        let mp = boot_services.open_protocol_exclusive::<MpServices>(handle).unwrap(); let bsp = mp.who_am_i().unwrap(); let count = mp.get_number_of_processors().unwrap();
        for i in 0..count.total { let processor = mp.get_processor_info(i).unwrap(); if i != bsp && processor.is_enabled() && cpu_count < apic_ids.len() { apic_ids[cpu_count] = processor.processor_id as u32; cpu_count += 1; } }
    }
    (ap_trampoline, apic_ids, cpu_count)
}
// aarch64: one CPU so far (the others start through PSCI, issue 203); its MPIDR affinity as the ID.
#[cfg(target_arch = "aarch64")]
fn processors(_boot_services: &BootServices) -> (usize, [u32; 8], usize) {
    let mpidr: u64; unsafe { core::arch::asm!("mrs {}, mpidr_el1", out(reg) mpidr); }
    let mut ids = [0u32; 8]; ids[0] = (mpidr & 0xFF_FFFF) as u32;
    (0, ids, 1)
}

// Reads a whole file of the boot volume into `buffer`.
fn read_file<'a>(root: &mut uefi::proto::media::file::Directory, name: &str, buffer: &'a mut [u8]) -> Result<&'a [u8], &'static str> {
    let mut name_buf = [0u16; 32];
    let path = uefi::CStr16::from_str_with_buf(name, &mut name_buf).map_err(|_| "bad file name")?;
    let handle = root.open(path, FileMode::Read, FileAttribute::empty()).map_err(|_| "file not found")?;
    let FileType::Regular(mut file) = handle.into_type().map_err(|_| "unreadable")? else { return Err("not a regular file") };
    let size = file.read(buffer).map_err(|_| "read error")?;
    if size == buffer.len() { return Err("file larger than 4 MiB"); }
    Ok(&buffer[..size])
}

#[entry]
fn main(_image: Handle, mut system_table: SystemTable<Boot>) -> Status {
    let loaded = {
        let boot_services = system_table.boot_services();
        (|| -> Result<_, (&'static str, &'static str)> {
            let sfs_handle = boot_services.get_handle_for_protocol::<SimpleFileSystem>().map_err(|_| ("boot volume", "no file system"))?;
            let mut sfs = boot_services.open_protocol_exclusive::<SimpleFileSystem>(sfs_handle).map_err(|_| ("boot volume", "cannot open"))?;
            let mut root = sfs.open_volume().map_err(|_| ("boot volume", "cannot open"))?;
            let file_buf_addr = boot_services.allocate_pages(AllocateType::MaxAddress(0xffff_ffff), MemoryType::LOADER_DATA, FILE_BUFFER_PAGES).map_err(|_| ("file buffer", "out of memory"))?;
            let file_buf = unsafe { core::slice::from_raw_parts_mut(file_buf_addr as *mut u8, FILE_BUFFER_PAGES * 4096) };
            let kernel = read_file(&mut root, "kernel.elf", file_buf).map_err(|e| ("kernel.elf", e))?;
            let kernel_entry = load_elf(boot_services, kernel).map_err(|e| ("kernel.elf", e))?;
            // Only system services are loaded into memory; loader reads applications from disk later.
            let mut programs = [ProgramImage { data: core::ptr::null(), len: 0 }; abi::BOOT_IMAGES];
            for (image, name) in programs.iter_mut().zip(abi::BOOT_FILES) {
                match read_file(&mut root, name, file_buf) {
                    Ok(data) => *image = keep_program(boot_services, data),
                    // aarch64 boots with the services it has so far (issue 201); init leaves out the rest.
                    Err("file not found") if cfg!(target_arch = "aarch64") && name != "init.elf" => {}
                    Err(e) => return Err((name, e)),
                }
            }
            Ok((kernel_entry, programs))
        })()
    };
    let (kernel_entry, programs) = match loaded { Ok(loaded) => loaded, Err((file, reason)) => fail(&mut system_table, file, reason) };
    let display = select_display(system_table.boot_services());
    let (fb_ptr, mode) = match display { Ok(display) => display, Err(reason) => fail(&mut system_table, "display", reason) };
    let (pixel_format, pixel_masks) = match (mode.pixel_format(), mode.pixel_bitmask()) {
        (PixelFormat::Rgb, _) => (PIXEL_RGB, [0; 3]),
        (PixelFormat::Bitmask, Some(m)) => (PIXEL_BITMASK, [m.red, m.green, m.blue]),
        _ => (PIXEL_BGR, [0; 3]),
    };
    // The ACPI 2.0 root pointer (or the 1.0 one), for the kernel's reset register.
    let acpi_rsdp = system_table.config_table().iter().find(|e| e.guid == uefi::table::cfg::ACPI2_GUID).or_else(|| system_table.config_table().iter().find(|e| e.guid == uefi::table::cfg::ACPI_GUID)).map_or(0, |e| e.address as u64);
    let (boot_info, kernel_stack) = {
        let boot_services = system_table.boot_services();
        let heap_len = 64 * 1024 * 1024; let heap_ptr = boot_services.allocate_pages(AllocateType::MaxAddress(0xffff_ffff), MemoryType::LOADER_DATA, heap_len / 4096).unwrap() as *mut u8; let (ap_trampoline, apic_ids, cpu_count) = processors(boot_services);
        let handoff = boot_services.allocate_pages(AllocateType::MaxAddress(0xffff_ffff), MemoryType::LOADER_DATA, 65).unwrap() as usize; let memory_map = boot_services.allocate_pages(AllocateType::MaxAddress(0xffff_ffff), MemoryType::LOADER_DATA, MEMORY_MAP_PAGES).unwrap() as *mut StatPhys;
        let info = BootInfo { fb_ptr, width: mode.resolution().0, height: mode.resolution().1, stride: mode.stride(), programs, heap_ptr, heap_len, ap_trampoline, cpu_count, apic_ids, memory_map, memory_map_len: 0, pixel_format, pixel_masks, acpi_rsdp, cpu_features: 0 }; unsafe { (handoff as *mut BootInfo).write(info); } (handoff, handoff + 65 * 4096)
    };
    let (_system_table, memory_map) = system_table.exit_boot_services(MemoryType::LOADER_DATA);
    // The final memory map, after boot services are gone, as the kernel will see the machine.
    unsafe {
        let info = boot_info as *mut BootInfo; let capacity = MEMORY_MAP_PAGES * 4096 / core::mem::size_of::<StatPhys>();
        for (index, entry) in memory_map.entries().take(capacity).enumerate() {
            (*info).memory_map.cast_mut().add(index).write(StatPhys { kind: entry.ty.0, index: index as u32, start: entry.phys_start, pages: entry.page_count });
            (*info).memory_map_len = index + 1;
        }
    }
    #[cfg(target_arch = "x86_64")]
    unsafe { core::arch::asm!("cli", "mov rsp, rcx", "xor ebp, ebp", "call rax", in("rax") kernel_entry, in("rcx") kernel_stack, in("rdi") boot_info, options(noreturn)); }
    #[cfg(target_arch = "aarch64")]
    unsafe { core::arch::asm!("msr daifset, #0xf", "mov sp, x1", "mov x29, xzr", "mov x30, xzr", "br x2", in("x0") boot_info, in("x1") kernel_stack, in("x2") kernel_entry, options(noreturn)); }
}
#[panic_handler] fn panic(info: &PanicInfo) -> ! { let _ = writeln!(Serial, "\r\nBOOT PANIC: {}\r", info); halt() }
#[no_mangle] pub extern "C" fn wcslen(mut s: *const u16) -> usize { let mut len = 0; unsafe { while *s != 0 { len += 1; s = s.add(1); } } len }
