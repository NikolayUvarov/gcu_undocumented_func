#![no_std]
#![no_main]
use core::fmt::Write;
use core::panic::PanicInfo;
use uefi::prelude::*;
use uefi::proto::console::gop::{GraphicsOutput, Mode, ModeInfo, PixelFormat};
use uefi::proto::media::file::{File, FileAttribute, FileMode, FileType};
#[cfg(target_arch = "x86_64")]
use uefi::proto::pi::mp::MpServices;
use uefi::table::boot::{AllocateType, BootServices, MemoryType, OpenProtocolAttributes, OpenProtocolParams, SearchType};
use core::ffi::c_void;
use core::sync::atomic::{AtomicPtr, Ordering::Relaxed};
#[path = "../../common/abi.rs"] mod abi;
use abi::{BootInfo, ProgramImage, StatPhys, ABI_VERSION, PIXEL_BGR, PIXEL_BITMASK, PIXEL_RGB}; mod elf_reloc; mod verify;

const MEMORY_MAP_PAGES: usize = 16; // firmware memory map copied for the kernel (STAT PHYSMAP)

const FILE_BUFFER_PAGES: usize = 1024; // 4 MiB: the largest boot image
const MANIFEST_PAGES: usize = 16; // 64 KiB: the largest boot manifest (350-UPD-0003)

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

// The mark EDK2 and Apple's firmware put on the handle of each active console output (gEfiConsoleOutDeviceGuid).
#[repr(C)]
#[uefi::proto::unsafe_protocol("d3b36f2c-d551-11d4-9a46-0090273fc14d")]
struct ConsoleOutDevice { _opaque: u8 }

// A line of the loader's progress on the text console (211-KRN-0016): without COM1 it shows where a boot stops.
fn say(system_table: &SystemTable<Boot>, args: core::fmt::Arguments) {
    let mut console = unsafe { system_table.unsafe_clone() };
    let _ = writeln!(console.stdout(), "MIND CORE BOOT: {}", args);
}

// A graphics mode and its framebuffer as the progress lines show them.
struct Shown(ModeInfo, u64);
impl core::fmt::Display for Shown {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let format = match self.0.pixel_format() { PixelFormat::Rgb => "RGB", PixelFormat::Bgr => "BGR", PixelFormat::Bitmask => "BITMASK", PixelFormat::BltOnly => "BLT ONLY" };
        write!(f, "{}x{} STRIDE {} {} FB {:#018X}", self.0.resolution().0, self.0.resolution().1, self.0.stride(), format, self.1)
    }
}

// The screen's GOP, as Linux's EFI stub picks it (find_gop): the first of an active console output with a linear
// framebuffer, else the first with one, else the first. With two GPUs (a MacBook Pro's) or the console splitter's own
// GOP the first one listed may not be the screen's. Opened without taking it from the console: progress keeps showing.
// Keeps the firmware's mode if it has a linear framebuffer, else switches to the largest such mode up to 1920x1200.
fn select_display(system_table: &SystemTable<Boot>) -> Result<(*mut u32, ModeInfo), &'static str> {
    let services = system_table.boot_services();
    let handles = services.locate_handle_buffer(SearchType::from_proto::<GraphicsOutput>()).map_err(|_| "no graphics output")?;
    let params = |handle| OpenProtocolParams { handle, agent: services.image_handle(), controller: None };
    let open = |handle| unsafe { services.open_protocol::<GraphicsOutput>(params(handle), OpenProtocolAttributes::GetProtocol) };
    say(system_table, format_args!("{} GRAPHICS OUTPUTS", handles.len()));
    let (mut console, mut linear) = (None, None);
    for (index, &handle) in handles.iter().enumerate() {
        let Ok(mut gop) = open(handle) else { say(system_table, format_args!("GOP {}: CANNOT OPEN", index)); continue };
        // A BltOnly mode has no framebuffer; uefi panics on reading it (the console splitter's GOP with two GPUs).
        let info = gop.current_mode_info();
        let fb = if info.pixel_format() == PixelFormat::BltOnly { 0 } else { gop.frame_buffer().as_mut_ptr() as u64 };
        let is_console = services.test_protocol::<ConsoleOutDevice>(params(handle)).is_ok();
        say(system_table, format_args!("GOP {}: {}{}", index, Shown(info, fb), if is_console { " CONSOLE" } else { "" }));
        let usable = info.pixel_format() != PixelFormat::BltOnly && fb != 0;
        if usable && is_console && console.is_none() { console = Some(index); }
        if usable && linear.is_none() { linear = Some(index); }
    }
    let chosen = console.or(linear).unwrap_or(0);
    let mut gop = open(*handles.get(chosen).ok_or("no graphics output")?).map_err(|_| "cannot open graphics output")?;
    if !drawable(&gop.current_mode_info()) {
        let area = |mode: &Mode| { let (w, h) = mode.info().resolution(); if w <= 1920 && h <= 1200 { w * h } else { 0 } };
        let best = gop.modes(services).filter(|mode| drawable(mode.info())).max_by_key(area).ok_or("no mode with a linear framebuffer (BltOnly)")?;
        gop.set_mode(&best).map_err(|_| "cannot set a mode with a linear framebuffer")?;
    }
    let (fb_ptr, info) = (gop.frame_buffer().as_mut_ptr().cast::<u32>(), gop.current_mode_info());
    say(system_table, format_args!("USING GOP {}: {}", chosen, Shown(info, fb_ptr as u64)));
    Ok((fb_ptr, info))
}

fn halt() -> ! {
    #[cfg(target_arch = "x86_64")] loop { unsafe { core::arch::asm!("cli; hlt"); } }
    #[cfg(target_arch = "aarch64")] loop { unsafe { core::arch::asm!("msr daifset, #0xf", "wfi"); } }
}
#[cfg(target_arch = "x86_64")] const MACHINE: u16 = 0x3E;
#[cfg(target_arch = "aarch64")] const MACHINE: u16 = 0xB7;
fn fail(system_table: &mut SystemTable<Boot>, file: &str, reason: &str) -> ! {
    let _ = writeln!(Serial, "\r\nBOOT ERROR: {}: {}\r", file, reason);
    show_text(system_table);
    let _ = writeln!(system_table.stdout(), "BOOT ERROR: {}: {}", file, reason);
    halt()
}

// Apple's console control (211-KRN-0015): a Mac's firmware keeps its console in graphics mode, where text never shows.
#[repr(C)]
#[uefi::proto::unsafe_protocol("f42f7782-012e-4c12-9956-49f94304f721")]
struct ConsoleControl {
    get_mode: usize,
    set_mode: unsafe extern "efiapi" fn(this: *mut ConsoleControl, mode: u32) -> Status,
    lock_std_in: usize,
}

// Puts the console in text mode where the firmware has Apple's console control; elsewhere it is already.
fn show_text(system_table: &SystemTable<Boot>) {
    let services = system_table.boot_services();
    let Ok(handle) = services.get_handle_for_protocol::<ConsoleControl>() else { return };
    let params = OpenProtocolParams { handle, agent: services.image_handle(), controller: None };
    if let Ok(mut control) = unsafe { services.open_protocol::<ConsoleControl>(params, OpenProtocolAttributes::GetProtocol) } {
        let set_mode = control.set_mode;
        let _ = unsafe { set_mode(&mut *control, 0) }; // EfiConsoleControlScreenText
    }
}

// The firmware's system table while boot services run: a panic is reported on the screen too.
static BOOT_TABLE: AtomicPtr<c_void> = AtomicPtr::new(core::ptr::null_mut());

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
    if size == buffer.len() { return Err("file larger than its buffer"); }
    Ok(&buffer[..size])
}

#[entry]
fn main(image: Handle, mut system_table: SystemTable<Boot>) -> Status {
    BOOT_TABLE.store(system_table.as_ptr().cast_mut(), Relaxed);
    show_text(&system_table);
    say(&system_table, format_args!("STARTED; READING THE KERNEL AND THE SERVICES FROM ITS OWN VOLUME"));
    let loaded = {
        let boot_services = system_table.boot_services();
        (|| -> Result<_, (&'static str, &'static str)> {
            // The volume this loader was read from (211-KRN-0012), not the first one the firmware lists: that may be
            // another disk's EFI partition, such as a Mac's internal disk.
            let mut sfs = boot_services.get_image_file_system(image).map_err(|_| ("boot volume", "no file system on the loader's device"))?;
            let mut root = sfs.open_volume().map_err(|_| ("boot volume", "cannot open"))?;
            let file_buf_addr = boot_services.allocate_pages(AllocateType::MaxAddress(0xffff_ffff), MemoryType::LOADER_DATA, FILE_BUFFER_PAGES).map_err(|_| ("file buffer", "out of memory"))?;
            let file_buf = unsafe { core::slice::from_raw_parts_mut(file_buf_addr as *mut u8, FILE_BUFFER_PAGES * 4096) };
            // Nothing is loaded before the manifest's signature checks; each image is checked against it (350-UPD-0003).
            let manifest_addr = boot_services.allocate_pages(AllocateType::MaxAddress(0xffff_ffff), MemoryType::LOADER_DATA, MANIFEST_PAGES).map_err(|_| ("MANIFEST", "out of memory"))?;
            let manifest_buf = unsafe { core::slice::from_raw_parts_mut(manifest_addr as *mut u8, MANIFEST_PAGES * 4096) };
            let mut signature = [0u8; 128];
            let signature = read_file(&mut root, "MANIFEST.SIG", &mut signature).map_err(|e| ("MANIFEST.SIG", e))?;
            let text = read_file(&mut root, "MANIFEST", manifest_buf).map_err(|e| ("MANIFEST", e))?;
            let manifest = verify::Manifest::verified(text, signature).map_err(|e| ("MANIFEST", e))?;
            let kernel = read_file(&mut root, "kernel.elf", file_buf).map_err(|e| ("kernel.elf", e))?;
            manifest.check("kernel.elf", kernel).map_err(|e| ("kernel.elf", e))?;
            let kernel_entry = load_elf(boot_services, kernel).map_err(|e| ("kernel.elf", e))?;
            let mut checked = 1;
            // Only system services are loaded into memory; loader reads applications from disk later.
            let mut programs = [ProgramImage { data: core::ptr::null(), len: 0 }; abi::BOOT_IMAGES];
            for (image, name) in programs.iter_mut().zip(abi::BOOT_FILES) {
                match read_file(&mut root, name, file_buf) {
                    Ok(data) => {
                        manifest.check(name, data).map_err(|e| (name, e))?;
                        *image = keep_program(boot_services, data);
                        checked += 1;
                    }
                    // aarch64 boots with the services it has so far (issue 201); init leaves out the rest.
                    Err("file not found") if cfg!(target_arch = "aarch64") && name != "init.elf" => {}
                    Err(e) => return Err((name, e)),
                }
            }
            // The launch record: which manifest, signed by which key, covered what was loaded (MC-9.5).
            let digest = manifest.digest();
            let _ = write!(Serial, "\r\nBOOT: MANIFEST ");
            for b in &digest[..8] { let _ = write!(Serial, "{:02x}", b); }
            let _ = writeln!(Serial, " KEY {}{} VERIFIED, {} IMAGES CHECKED\r", manifest.key(), if verify::TEST_KEY { " (THE TEST KEY)" } else { "" }, checked);
            Ok((kernel_entry, programs))
        })()
    };
    let (kernel_entry, programs) = match loaded { Ok(loaded) => loaded, Err((file, reason)) => fail(&mut system_table, file, reason) };
    say(&system_table, format_args!("KERNEL AND {} SERVICES READ", programs.iter().filter(|p| p.len > 0).count()));
    let display = select_display(&system_table);
    let (fb_ptr, mode) = match display { Ok(display) => display, Err(reason) => fail(&mut system_table, "display", reason) };
    let (pixel_format, pixel_masks) = match (mode.pixel_format(), mode.pixel_bitmask()) {
        (PixelFormat::Rgb, _) => (PIXEL_RGB, [0; 3]),
        (PixelFormat::Bitmask, Some(m)) => (PIXEL_BITMASK, [m.red, m.green, m.blue]),
        _ => (PIXEL_BGR, [0; 3]),
    };
    // The ACPI 2.0 root pointer (or the 1.0 one), for the kernel's reset register.
    let acpi_rsdp = system_table.config_table().iter().find(|e| e.guid == uefi::table::cfg::ACPI2_GUID).or_else(|| system_table.config_table().iter().find(|e| e.guid == uefi::table::cfg::ACPI_GUID)).map_or(0, |e| e.address as u64);
    let (boot_info, kernel_stack, cpu_count) = {
        let boot_services = system_table.boot_services();
        let heap_len = 64 * 1024 * 1024; let heap_ptr = boot_services.allocate_pages(AllocateType::MaxAddress(0xffff_ffff), MemoryType::LOADER_DATA, heap_len / 4096).unwrap() as *mut u8; let (ap_trampoline, apic_ids, cpu_count) = processors(boot_services);
        let handoff = boot_services.allocate_pages(AllocateType::MaxAddress(0xffff_ffff), MemoryType::LOADER_DATA, 65).unwrap() as usize; let memory_map = boot_services.allocate_pages(AllocateType::MaxAddress(0xffff_ffff), MemoryType::LOADER_DATA, MEMORY_MAP_PAGES).unwrap() as *mut StatPhys;
        let info = BootInfo { fb_ptr, width: mode.resolution().0, height: mode.resolution().1, stride: mode.stride(), programs, heap_ptr, heap_len, ap_trampoline, cpu_count, apic_ids, memory_map, memory_map_len: 0, pixel_format, pixel_masks, acpi_rsdp, cpu_features: 0, abi_version: ABI_VERSION }; unsafe { (handoff as *mut BootInfo).write(info); } (handoff, handoff + 65 * 4096, cpu_count)
    };
    say(&system_table, format_args!("{} CPUS; EXITING BOOT SERVICES", cpu_count));
    BOOT_TABLE.store(core::ptr::null_mut(), Relaxed);
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
#[panic_handler] fn panic(info: &PanicInfo) -> ! {
    let _ = writeln!(Serial, "\r\nBOOT PANIC: {}\r", info);
    // Taken once: a panic inside the console's own write ends on the serial line.
    if let Some(mut system_table) = unsafe { SystemTable::<Boot>::from_ptr(BOOT_TABLE.swap(core::ptr::null_mut(), Relaxed)) } {
        show_text(&system_table);
        let _ = writeln!(system_table.stdout(), "BOOT PANIC: {}", info);
    }
    halt()
}
#[no_mangle] pub extern "C" fn wcslen(mut s: *const u16) -> usize { let mut len = 0; unsafe { while *s != 0 { len += 1; s = s.add(1); } } len }
