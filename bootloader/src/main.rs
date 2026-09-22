#![no_std]
#![no_main]

use core::panic::PanicInfo;
use uefi::prelude::*;
use uefi::proto::console::gop::GraphicsOutput;
use uefi::proto::media::file::{File, FileAttribute, FileMode, FileType};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::proto::pi::mp::MpServices;
use uefi::table::boot::{AllocateType, MemoryType};

#[path = "../../common/abi.rs"] mod abi;
use abi::{BootInfo, ProgramImage};
mod elf_reloc;

fn keep_program(services: &uefi::table::boot::BootServices, data: &[u8]) -> ProgramImage {
    let address = services.allocate_pages(AllocateType::MaxAddress(0xffff_ffff), MemoryType::LOADER_DATA, data.len().div_ceil(4096)).unwrap();
    unsafe { core::ptr::copy_nonoverlapping(data.as_ptr(), address as *mut u8, data.len()); }
    ProgramImage { data: address as *const u8, len: data.len() }
}

#[repr(C)] struct Elf64_Ehdr { e_ident: [u8; 16], e_type: u16, e_machine: u16, e_version: u32, e_entry: u64, e_phoff: u64, e_shoff: u64, e_flags: u32, e_ehsize: u16, e_phentsize: u16, e_phnum: u16, e_shentsize: u16, e_shnum: u16, e_shstrndx: u16 }
#[repr(C)] struct Elf64_Phdr { p_type: u32, p_flags: u32, p_offset: u64, p_vaddr: u64, p_paddr: u64, p_filesz: u64, p_memsz: u64, p_align: u64 }

fn load_elf(boot_services: &uefi::table::boot::BootServices, file_data: &[u8]) -> u64 {
    let ehdr = unsafe { &*(file_data.as_ptr() as *const Elf64_Ehdr) };
    if ehdr.e_ident[0..4] != [0x7f, b'E', b'L', b'F'] { panic!("Invalid ELF magic"); }
    let phdrs = unsafe { core::slice::from_raw_parts(file_data.as_ptr().add(ehdr.e_phoff as usize) as *const Elf64_Phdr, ehdr.e_phnum as usize) };
    let mut min_vaddr = u64::MAX; let mut max_vaddr = 0;
    for phdr in phdrs { if phdr.p_type == 1 { if phdr.p_vaddr < min_vaddr { min_vaddr = phdr.p_vaddr; } let end = phdr.p_vaddr + phdr.p_memsz; if end > max_vaddr { max_vaddr = end; } } }
    let total_size = max_vaddr - min_vaddr;
    let pages = ((total_size as usize) / 4096) + 1;
    let base_addr = boot_services.allocate_pages(AllocateType::MaxAddress(0xffff_ffff), MemoryType::LOADER_DATA, pages).unwrap();
    for phdr in phdrs {
        if phdr.p_type == 1 {
            let dest = (base_addr + phdr.p_vaddr - min_vaddr) as *mut u8; let src = unsafe { file_data.as_ptr().add(phdr.p_offset as usize) };
            unsafe { core::ptr::copy_nonoverlapping(src, dest, phdr.p_filesz as usize); if phdr.p_memsz > phdr.p_filesz { core::ptr::write_bytes(dest.add(phdr.p_filesz as usize), 0, (phdr.p_memsz - phdr.p_filesz) as usize); } }
        }
    }
    let image = unsafe { core::slice::from_raw_parts_mut(base_addr as *mut u8, total_size as usize) };
    for phdr in phdrs { if phdr.p_type == 2 { elf_reloc::apply(image, min_vaddr, base_addr, phdr.p_vaddr, phdr.p_filesz as usize).expect("Invalid ELF relocations"); } }
    base_addr + ehdr.e_entry - min_vaddr
}

#[entry]
fn main(_image: Handle, system_table: SystemTable<Boot>) -> Status {
    let (boot_info, kernel_entry, kernel_stack) = {
        let boot_services = system_table.boot_services();
        let sfs_handle = boot_services.get_handle_for_protocol::<SimpleFileSystem>().unwrap();
        let mut sfs = boot_services.open_protocol_exclusive::<SimpleFileSystem>(sfs_handle).unwrap();
        let mut root = sfs.open_volume().unwrap();

        let file_buf_addr = boot_services.allocate_pages(AllocateType::MaxAddress(0xffff_ffff), MemoryType::LOADER_DATA, 1024).unwrap();
        let file_buf = unsafe { core::slice::from_raw_parts_mut(file_buf_addr as *mut u8, 1024 * 4096) };

        let mut name_buf = [0u16; 32];
        
        // 1. СНАЧАЛА грузим ядро, ДО создания замыкания, чтобы не конфликтовать заимствованиями!
        let k_name = uefi::CStr16::from_str_with_buf("kernel.elf", &mut name_buf).unwrap();
        let k_handle = root.open(k_name, FileMode::Read, FileAttribute::empty()).unwrap();
        let mut k_file = match k_handle.into_type().unwrap() { FileType::Regular(f) => f, _ => panic!("err") };
        let k_size = k_file.read(file_buf).unwrap();
        let kernel_entry = load_elf(boot_services, &file_buf[..k_size]);

        // 2. ТЕПЕРЬ объявляем замыкание для приложений
        let mut load_file = |name: &str| -> ProgramImage {
            let n = uefi::CStr16::from_str_with_buf(name, &mut name_buf).unwrap();
            let h = root.open(n, FileMode::Read, FileAttribute::empty()).unwrap();
            let mut f = match h.into_type().unwrap() { FileType::Regular(f) => f, _ => panic!("err") };
            let sz = f.read(file_buf).unwrap();
            keep_program(boot_services, &file_buf[..sz])
        };

        // Загружаем все 6 программ
        let app = load_file("app.elf");
        let app2 = load_file("app2.elf");
        let clock = load_file("clock.elf");
        let dzen_clock = load_file("dzenclk.elf");
        let ping = load_file("ping.elf");
        let pong = load_file("pong.elf");

        let heap_len = 64 * 1024 * 1024;
        let heap_ptr = boot_services.allocate_pages(AllocateType::MaxAddress(0xffff_ffff), MemoryType::LOADER_DATA, heap_len / 4096).unwrap() as *mut u8;
        let ap_trampoline = boot_services.allocate_pages(AllocateType::MaxAddress(0xFFFFF), MemoryType::LOADER_DATA, 1).expect("AP bootstrap") as usize;
        let mut apic_ids = [0u32; 8]; let mut cpu_count = 1;
        apic_ids[0] = core::arch::x86_64::__cpuid(1).ebx >> 24;
        if let Ok(handle) = boot_services.get_handle_for_protocol::<MpServices>() {
            let mp = boot_services.open_protocol_exclusive::<MpServices>(handle).unwrap();
            let bsp = mp.who_am_i().unwrap(); let count = mp.get_number_of_processors().unwrap();
            for i in 0..count.total { let processor = mp.get_processor_info(i).unwrap(); if i != bsp && processor.is_enabled() && cpu_count < apic_ids.len() { apic_ids[cpu_count] = processor.processor_id as u32; cpu_count += 1; } }
        }

        let gop_handle = boot_services.get_handle_for_protocol::<GraphicsOutput>().unwrap();
        let mut gop = boot_services.open_protocol_exclusive::<GraphicsOutput>(gop_handle).unwrap();
        let mode = gop.current_mode_info();
        let fb_ptr = gop.frame_buffer().as_mut_ptr().cast::<u32>();
        let handoff = boot_services.allocate_pages(AllocateType::MaxAddress(0xffff_ffff), MemoryType::LOADER_DATA, 65).unwrap() as usize;
        
        let info = BootInfo { fb_ptr, width: mode.resolution().0, height: mode.resolution().1, stride: mode.stride(), programs: [app, app2, clock, dzen_clock, ping, pong], heap_ptr, heap_len, ap_trampoline, cpu_count, apic_ids };
        unsafe { (handoff as *mut BootInfo).write(info); }
        (handoff, kernel_entry, handoff + 65 * 4096)
    };

    let (_system_table, _memory_map) = system_table.exit_boot_services(MemoryType::LOADER_DATA);
    unsafe { core::arch::asm!("cli", "mov rsp, rcx", "xor ebp, ebp", "call rax", in("rax") kernel_entry, in("rcx") kernel_stack, in("rdi") boot_info, options(noreturn)); }
}

#[panic_handler] fn panic(_info: &PanicInfo) -> ! { loop {} }
#[no_mangle] pub extern "C" fn wcslen(mut s: *const u16) -> usize { let mut len = 0; unsafe { while *s != 0 { len += 1; s = s.add(1); } } len }
