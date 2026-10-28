#![allow(dead_code)]
#[path = "../common/abi.rs"] mod abi;
#[path = "../kernel/src/screen.rs"] mod screen;
mod cpu { pub unsafe fn write_back(_: usize, _: usize) {} }
mod mmu { pub unsafe fn reach_device(_: usize, _: usize) -> bool { panic!("early framebuffer must not change mappings") } }

use abi::*;
use core::mem::{align_of, offset_of, size_of};

// Independent ABI 10 wire record: never replace its fixed array size with the current image count.
#[repr(C)]
struct Abi10BootInfo {
    fb_ptr: *mut u32, width: usize, height: usize, stride: usize, programs: [ProgramImage; 36],
    heap_ptr: *mut u8, heap_len: usize, ap_trampoline: usize, cpu_count: usize, apic_ids: [u32; 8],
    memory_map: *const StatPhys, memory_map_len: usize, pixel_format: u32, pixel_masks: [u32; 3],
    acpi_rsdp: u64, cpu_features: u64, abi_version: u32, boot_volume: BootVolume, boot_slot: BootSlot,
    launch: LaunchRecord, efi_runtime: u64, device_tree: u64, photo_pause_s: u32,
}

#[test]
fn abi10_prefix_offsets_survive_additional_images() {
    assert_eq!(size_of::<usize>(), 8, "supported boot ABIs are 64-bit");
    assert_eq!(BOOT_PREFIX_IMAGES, 36);
    assert_eq!(offset_of!(Abi10BootInfo, abi_version), 720);
    macro_rules! same { ($($field:ident),+ $(,)?) => { $(assert_eq!(offset_of!(BootInfo, $field), offset_of!(Abi10BootInfo, $field), stringify!($field));)+ }; }
    same!(fb_ptr, width, height, stride, programs, heap_ptr, heap_len, ap_trampoline, cpu_count, apic_ids,
        memory_map, memory_map_len, pixel_format, pixel_masks, acpi_rsdp, cpu_features, abi_version,
        boot_volume, boot_slot, launch, efi_runtime, device_tree, photo_pause_s);
    assert_eq!(offset_of!(BootInfo, extra_programs), size_of::<Abi10BootInfo>());
    // ABI 13: the link's words follow the images (800-KRN-0128).
    assert_eq!(offset_of!(BootInfo, link_present), size_of::<Abi10BootInfo>() + (BOOT_IMAGES - 36) * size_of::<ProgramImage>());
    assert_eq!(size_of::<BootInfo>(), (offset_of!(BootInfo, link_words) + LINK_WORDS_MAX).next_multiple_of(align_of::<BootInfo>()));
    assert!(size_of::<BootInfo>() <= 4096, "the scheduler grants one page for task boot information");
}

#[test]
fn version_read_needs_only_the_old_record() {
    let mut old: Abi10BootInfo = unsafe { core::mem::zeroed() };
    old.abi_version = 10;
    old.heap_ptr = usize::MAX as *mut u8;
    old.memory_map = usize::MAX as *const StatPhys;
    old.acpi_rsdp = u64::MAX;
    assert_eq!(unsafe { BootInfo::read_abi_version(core::ptr::addr_of!(old).cast()) }, Some(10));
    let mut current: BootInfo = unsafe { core::mem::zeroed() };
    current.abi_version = ABI_VERSION;
    assert_eq!(unsafe { BootInfo::read_abi_version(&current) }, Some(ABI_VERSION));
    assert_eq!(unsafe { BootInfo::read_abi_version(core::ptr::null()) }, None);
    assert_eq!(unsafe { BootInfo::read_abi_version(1usize as *const BootInfo) }, None);
}

// 351-KRN-0166: the loader's handoff apart from the system calls, one version with no range (000-KRN-0175). Handoff 21
// (550-KRN-0172) adds a 44th image and handoff 23 (700-KRN-0176) a 45th, each moving every field after extra_programs.
#[test]
fn the_handoff_takes_only_its_own_loader() {
    assert_eq!(BOOT_HANDOFF_VERSION, 23);
    assert_eq!(BOOT_IMAGES, 45, "a change of the boot images is a change of the handoff");
    assert_eq!(size_of::<BootInfo>(), 1064 + size_of::<ProgramImage>(), "a change of BootInfo is a change of the handoff");
    let at = BOOT_SERVICES.iter().position(|&name| name == "rtw88_wifi").unwrap();
    assert_eq!((BOOT_SERVICES[at - 1], BOOT_FILES[at]), ("rtw89_wifi", "rtw88_wifi.elf"));
    assert!(at < BOOT_SERVICES.iter().position(|&name| name == "wlan").unwrap(), "init starts the radio before wlan");
    // bt_hci after rtw88_wifi: after usb_host, whose client it holds, and vfs_server, whose reader reads its patch.
    let bt = BOOT_SERVICES.iter().position(|&name| name == "bt_hci").unwrap();
    assert_eq!((bt, BOOT_FILES[bt]), (at + 1, "bt_hci.elf"));
    for before in ["usb_host", "vfs_server"] { assert!(BOOT_SERVICES.iter().position(|&name| name == before).unwrap() < bt, "{before} starts before bt_hci"); }
}

// 173-KRN-0036: boot_flags stays the last field.
#[test]
fn boot_flags_is_the_last_field() {
    assert_eq!(offset_of!(BootInfo, boot_flags), offset_of!(BootInfo, link_words) + LINK_WORDS_MAX);
}

// 351-KRN-0167: a kernel that refuses the handoff reads the trial and the ACPI root from ABI 10's record alone.
#[test]
fn a_refused_handoff_reads_the_trial_and_the_acpi_root_from_abi_10s_record() {
    let mut old: Abi10BootInfo = unsafe { core::mem::zeroed() };
    old.abi_version = 10;
    old.acpi_rsdp = 0x1234_5000;
    old.boot_slot.trial = 1;
    assert_eq!(unsafe { BootInfo::read_refused(core::ptr::addr_of!(old).cast()) }, (true, 0x1234_5000));
    old.boot_slot.trial = 0;
    assert_eq!(unsafe { BootInfo::read_refused(core::ptr::addr_of!(old).cast()) }, (false, 0x1234_5000));
}

#[test]
fn image_access_covers_both_arrays_in_order_and_rejects_out_of_range() {
    let mut info: BootInfo = unsafe { core::mem::zeroed() };
    info.programs = core::array::from_fn(|i| ProgramImage { data: (i + 1) as *const u8, len: i + 10 });
    info.extra_programs = core::array::from_fn(|i| ProgramImage { data: (BOOT_PREFIX_IMAGES + i + 1) as *const u8, len: BOOT_PREFIX_IMAGES + i + 10 });
    assert_eq!(info.images().count(), BOOT_IMAGES);
    for (i, image) in info.images().enumerate() {
        assert_eq!((image.data as usize, image.len), (i + 1, i + 10));
        let indexed = info.image(i).unwrap();
        assert_eq!((indexed.data, indexed.len), (image.data, image.len));
    }
    assert!(info.image(BOOT_IMAGES).is_none());
    assert!(info.image(usize::MAX).is_none());
}

#[test]
fn rejected_early_framebuffer_never_requests_a_kernel_mapping() {
    let mut old: Abi10BootInfo = unsafe { core::mem::zeroed() };
    old.width = 640; old.height = 480; old.stride = 640; old.pixel_format = PIXEL_BGR;
    old.fb_ptr = (1u64 << 32) as *mut u32;
    unsafe { screen::init_early(core::ptr::addr_of!(old).cast()); }
    assert!(!screen::showing());
}
