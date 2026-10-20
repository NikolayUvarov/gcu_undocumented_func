#![allow(dead_code)]
extern crate alloc;
#[path = "../common/abi.rs"]
mod abi;
#[path = "../dzen-clock/src/cycle.rs"]
mod dzen_cycle;
use dzen_cycle as cycle;
#[path = "../dzen-clock/src/face.rs"]
mod dzen_face;
use dzen_face as face;
#[path = "../common/font.rs"]
mod font;
#[path = "../dzen-clock/src/view.rs"]
mod dzen_view;
#[path = "../kernel/src/elf.rs"]
mod elf;
#[path = "../bootloader/src/elf_reloc.rs"]
mod elf_reloc;
#[path = "../kernel/src/memory.rs"]
mod memory;
// The kernel's context switch is not built on the host: what paging.rs reads of it.
mod context {
    pub static XSAVE: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
    pub static XCR0: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
    pub fn set_area(_: usize) {}
    pub fn area() -> usize { 1024 }
}
// The kernel's frame pool is not set up on the host: task memory comes from the allocator.
mod frames {
    pub fn ready() -> bool { false }
    pub fn allocate(_: core::alloc::Layout) -> Option<core::ptr::NonNull<u8>> { None }
    pub unsafe fn free(_: core::ptr::NonNull<u8>, _: core::alloc::Layout) {}
    pub fn owns(_: *const u8) -> bool { false }
}
#[path = "../kernel/src/paging.rs"]
mod paging;
#[path = "../kernel/src/arch/x86_64/mmu.rs"]
mod mmu;
#[path = "../libmind/src/idl/codec.rs"]
mod codec;
#[path = "../kernel/src/task_state.rs"]
mod task_state;
#[path = "../kernel/src/user_heap.rs"]
mod user_heap;
#[path = "../kernel/src/task_table.rs"]
mod task_table;

fn phys(kind: u32, start: u64, end: u64) -> abi::StatPhys {
    assert_eq!(start % 4096, 0);
    assert_eq!(end % 4096, 0);
    abi::StatPhys { kind, index: 0, start, pages: (end - start) / 4096 }
}

#[test]
fn realtek_probe_mapping_accepts_only_present_uncached_identity_leaves() {
    let pdpt = memory::Region::new(4096, 4096).unwrap();
    let pd = memory::Region::new(4096, 4096).unwrap();
    let pt = memory::Region::new(4096, 4096).unwrap();
    let physical = 0xfce0_0040u64;
    unsafe {
        let root = pdpt.ptr() as *mut u64;
        let directory = pd.ptr() as *mut u64;
        let leaves = pt.ptr() as *mut u64;
        let pde = directory.add((physical >> 21 & 511) as usize);
        let pte = leaves.add((physical >> 12 & 511) as usize);
        let valid = || mmu::identity_device_u32_in(root, physical);
        assert!(!valid(), "absent PDPT entry");
        root.add(3).write(pd.ptr() as u64 | 3);
        assert!(!valid(), "absent PDE");
        pde.write(0xfce0_0083);
        assert!(!valid(), "large write-back leaf");
        pde.write(0xfce0_009b);
        assert!(valid(), "large UC identity leaf");
        pde.write(0xfcc0_009b);
        assert!(!valid(), "large non-identity mapping");
        pde.write(pt.ptr() as u64 | 3);
        assert!(!valid(), "absent PTE");
        pte.write(0xfce0_0003);
        assert!(!valid(), "small write-back leaf");
        pte.write(0xfce0_001b);
        assert!(valid(), "small UC identity leaf");
        pte.write(0xfce0_101b);
        assert!(!valid(), "small non-identity mapping");
        assert!(!mmu::identity_device_u32_in(root, physical + 1));
        assert!(!mmu::identity_device_u32_in(root, 1 << 32));
        assert!(!mmu::identity_device_u32_in(root, u64::MAX - 3));
        root.add(3).write(0xc000_009b);
        assert!(!valid(), "unexpected 1 GiB leaf is not dereferenced as a table");
    }
}

#[test]
fn identity_cache_keeps_ryzen_loader_ram_write_back_above_three_gib() {
    // Adjacent types and addresses from the Ryzen report; the 64 MiB kernel arena is inside the loader-data range.
    let map = [
        phys(2, 0xC68F_B000, 0xC6D0_B000),
        phys(7, 0xA20_F000, 0xC20A_B000),
        phys(4, 0xC68F_6000, 0xC68F_B000),
        phys(2, 0xC20A_B000, 0xC68F_6000),
        phys(4, 0xC6D0_B000, 0xCA8F_C000),
    ];
    for address in (0xC200_0000..0xC6E0_0000).step_by(0x20_0000) {
        let (entry, table) = mmu::identity_region(&map, address).unwrap();
        assert!(table.is_none(), "adjacent RAM descriptors cover this whole region");
        assert_eq!(entry, address | 0x83, "RAM at {address:x} must use PAT entry 0, supervisor writable");
    }
}

#[test]
fn identity_cache_splits_ram_mmio_and_unknown_pages_in_one_large_region() {
    let start = 0xC800_0000;
    let map = [
        phys(7, start, start + 7 * 4096),
        phys(11, start + 7 * 4096, start + 8 * 4096),
        phys(2, start + 9 * 4096, start + 0x20_0000),
    ];
    let (entry, table) = mmu::identity_region(&map, start).unwrap();
    let table = table.expect("a 4 KiB table is required at a RAM/device boundary");
    assert_eq!(entry, table.ptr() as u64 | 3, "the table itself is cached and supervisor-only");
    let leaves = unsafe { core::slice::from_raw_parts(table.ptr() as *const u64, 512) };
    for (page, &leaf) in leaves.iter().enumerate() {
        let cache = if matches!(page, 7 | 8) { 0x18 } else { 0 };
        assert_eq!(leaf, (start + page as u64 * 4096) | 3 | cache, "page {page}");
    }
}

#[test]
fn identity_cache_never_assumes_low_addresses_are_ram() {
    for start in [0, 0x8000_0000, 0xC000_0000, 0xFEE0_0000] {
        for kind in [0, 8, 11, 12, 13, 14, u32::MAX] {
            let (entry, table) = mmu::identity_region(&[phys(kind, start, start + 0x20_0000)], start).unwrap();
            assert!(table.is_none());
            assert_eq!(entry, start | 0x9B, "kind {kind} at {start:x}");
        }
        let (entry, table) = mmu::identity_region(&[], start).unwrap();
        assert!(table.is_none());
        assert_eq!(entry, start | 0x9B, "an absent descriptor is not RAM");
    }
}

#[test]
fn identity_cache_reserved_overlap_stays_uncached_regardless_of_descriptor_order() {
    let start = 0xC800_0000;
    let mut map = [phys(2, start, start + 0x20_0000), phys(0, start + 4096, start + 8192)];
    for _ in 0..2 {
        let (_, table) = mmu::identity_region(&map, start).unwrap();
        let table = table.unwrap();
        let leaves = unsafe { core::slice::from_raw_parts(table.ptr() as *const u64, 512) };
        assert_eq!(leaves[0], start | 3);
        assert_eq!(leaves[1], (start + 4096) | 0x1B);
        assert_eq!(leaves[2], (start + 8192) | 3);
        map.reverse();
    }
}

#[test]
fn identity_cache_device_remap_changes_large_and_small_leaves_not_table_pointer() {
    let start = 0xC800_0000;
    let (mut entry, table) = mmu::identity_region(&[phys(2, start, start + 0x20_0000)], start).unwrap();
    assert!(table.is_none());
    unsafe { mmu::uncache_identity(&mut entry, start as usize); }
    assert_eq!(entry, start | 0x9B);
    let (mut entry, table) = mmu::identity_region(&[phys(2, start, start + 4096)], start).unwrap();
    let table = table.unwrap();
    let pointer = entry;
    unsafe { mmu::uncache_identity(&mut entry, start as usize); }
    assert_eq!(entry, pointer, "PCD/PWT on a table pointer do not change the memory type of its leaves");
    let leaves = unsafe { core::slice::from_raw_parts(table.ptr() as *const u64, 512) };
    for (page, &leaf) in leaves.iter().enumerate() {
        assert_eq!(leaf, (start + page as u64 * 4096) | 0x1B);
    }
}

#[test]
fn real_program_instances_have_fresh_bss_and_rebased_private_pointers() {
    for file in [
        "usb_root/app.elf",
        "usb_root/app2.elf",
        "usb_root/clock.elf",
        "usb_root/dzen-clock.elf",
    ] {
        let data = std::fs::read(file).unwrap();
        let image = elf::Image::parse(&data).unwrap();
        let mut first = vec![0xaa; image.size];
        let mut second = vec![0xbb; image.size];
        assert_eq!(image.load(&mut first, 0x100000).unwrap(), 0x100000);
        assert_eq!(image.load(&mut second, 0x200000).unwrap(), 0x200000);
        let phoff = u64::from_le_bytes(data[32..40].try_into().unwrap()) as usize;
        let count = u16::from_le_bytes(data[56..58].try_into().unwrap()) as usize;
        for p in data[phoff..phoff + count * 56].chunks_exact(56) {
            if u32::from_le_bytes(p[0..4].try_into().unwrap()) != 1 {
                continue;
            }
            let address = u64::from_le_bytes(p[16..24].try_into().unwrap()) as usize;
            let filesz = u64::from_le_bytes(p[32..40].try_into().unwrap()) as usize;
            let memsz = u64::from_le_bytes(p[40..48].try_into().unwrap()) as usize;
            assert!(first[address + filesz..address + memsz]
                .iter()
                .all(|b| *b == 0));
            assert!(second[address + filesz..address + memsz]
                .iter()
                .all(|b| *b == 0));
            if memsz > filesz {
                first[address + filesz] = 123;
                assert_eq!(second[address + filesz], 0);
            }
        }
        // GOT/function pointer relocations must point into the matching copy.
        let mut relocated = 0;
        for offset in (0..image.size.saturating_sub(7)).step_by(8) {
            let a = u64::from_le_bytes(first[offset..offset + 8].try_into().unwrap());
            let b = u64::from_le_bytes(second[offset..offset + 8].try_into().unwrap());
            if (0x100000..0x100000 + image.size as u64).contains(&a) && b == a + 0x100000 {
                relocated += 1;
            }
        }
        if file.ends_with("/app.elf") {
            assert!(relocated > 0, "app must exercise GOT relocation");
        }
    }
}

#[test]
fn rejects_corrupt_headers_segments_and_entries_without_panicking() {
    let data = std::fs::read("usb_root/app.elf").unwrap();
    for length in [0, 1, 7, 63, 64, 100] {
        assert!(elf::Image::parse(&data[..length]).is_err());
    }
    for (offset, value) in [(16, 2u8), (18, 1), (54, 0), (4, 1)] {
        let mut bad = data.clone();
        bad[offset] = value;
        assert!(elf::Image::parse(&bad).is_err());
    }
    for offset in [24, 32, 64 + 8, 64 + 16, 64 + 32, 64 + 40] {
        let mut bad = data.clone();
        bad[offset..offset + 8].fill(255);
        assert!(elf::Image::parse(&bad).is_err());
    }
    let image = elf::Image::parse(&data).unwrap();
    assert!(image.load(&mut [0; 16], 0x100000).is_err());
}

#[test]
fn input_event_words_round_trip() {
    let word = abi::input_event(0x1E, abi::KEY_CHAR, abi::MOD_SHIFT | abi::MOD_CTRL, true, 'Ж' as u32);
    assert_eq!((abi::event_byte(word), abi::event_key(word), abi::event_mods(word), abi::event_pressed(word), abi::event_char(word)), (0x1E, abi::KEY_CHAR, 3, true, 'Ж' as u32));
    let release = abi::input_event(0, abi::KEY_F1 + 11, 0, false, 0);
    assert_eq!((abi::event_byte(release), abi::event_key(release), abi::event_pressed(release), abi::event_char(release)), (0, abi::KEY_F1 + 11, false, 0));
    assert_eq!(abi::event_char(abi::input_event(0, abi::KEY_CHAR, 0, true, 0x10FFFF)), 0x10FFFF);
}

#[test]
fn idl_codec_round_trips_and_rejects_malformed_payloads() {
    use codec::{List, Reader, Text, Wire, Writer};
    let mut buffer = [0u8; 64];
    let mut w = Writer::new(&mut buffer);
    codec::encode_str::<8>("путь", &mut w).unwrap();
    codec::encode_slice::<u16, 3>(&[1, 2, 3], &mut w).unwrap();
    true.encode(&mut w).unwrap();
    0x0102_0304_0506_0708u64.encode(&mut w).unwrap();
    let length = w.len();
    assert_eq!(length, 2 + 8 + 2 + 6 + 1 + 8);
    let mut r = Reader::new(&buffer[..length]);
    assert_eq!(Text::<8>::decode(&mut r).unwrap().as_str(), "путь");
    assert_eq!(List::<u16, 3>::decode(&mut r).unwrap().as_slice(), &[1, 2, 3]);
    assert!(bool::decode(&mut r).unwrap());
    assert_eq!(u64::decode(&mut r), Some(0x0102_0304_0506_0708));
    assert!(r.done());
    // Bounds are checked on both sides.
    let mut w = Writer::new(&mut buffer);
    assert!(codec::encode_str::<3>("long", &mut w).is_none());
    assert!(codec::encode_slice::<u8, 2>(&[1, 2, 3], &mut w).is_none());
    assert!(Text::<4>::new("toolong").is_none());
    let malformed: [&[u8]; 5] = [
        &[5, 0, b'a', b'b'],         // length beyond the payload
        &[9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], // length beyond the declared bound (8)
        &[2, 0, 0xC3, 0x28],         // invalid UTF-8
        &[],                         // missing length
        &[1, 0, 0xFF],               // invalid UTF-8 (lone byte)
    ];
    for payload in malformed { assert!(Text::<8>::decode(&mut Reader::new(payload)).is_none(), "{:?}", payload); }
    assert!(bool::decode(&mut Reader::new(&[2])).is_none());
    assert!(List::<u8, 2>::decode(&mut Reader::new(&[3, 0, 1, 2, 3])).is_none());
    let mut r = Reader::new(&[1, 0, b'x', 7]);
    assert!(Text::<8>::decode(&mut r).is_some() && !r.done(), "trailing bytes are visible to the caller");
    assert_eq!(<List<Text<16>, 64> as Wire>::MAX, 2 + 64 * 18);
}

#[test]
fn pixels_reach_every_framebuffer_format() {
    use abi::{pixel_to_device, PIXEL_BGR, PIXEL_BITMASK, PIXEL_RGB};
    let orange = 0x00FF_8000;
    assert_eq!(pixel_to_device(orange, PIXEL_BGR, [0; 3]), 0x00FF_8000);
    assert_eq!(pixel_to_device(orange, PIXEL_RGB, [0; 3]), 0x0000_80FF);
    // 8-bit channels at the BGR positions give the same word; 10-bit channels (2:10:10:10) are scaled.
    assert_eq!(pixel_to_device(orange, PIXEL_BITMASK, [0xFF_0000, 0xFF00, 0xFF]), 0x00FF_8000);
    assert_eq!(pixel_to_device(0x00FF_0001, PIXEL_BITMASK, [0x3FF0_0000, 0xF_FC00, 0x3FF]), 0x3FC0_0004);
    // A 5:6:5 layout keeps the high bits of each channel.
    assert_eq!(pixel_to_device(0x00FF_FFFF, PIXEL_BITMASK, [0xF800, 0x07E0, 0x001F]), 0xFFFF);
}

#[test]
fn round_robin_over_a_cpus_slots_picks_as_over_the_whole_table() {
    // 171-KRN-0009: select picks among one CPU's sorted slots (next_in) what the pass over every slot picked (next_by).
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    let mut random = move || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; seed };
    for _ in 0..2000 {
        let len = 1 + (random() % 96) as usize;
        let cpu_of: Vec<u64> = (0..len).map(|_| random() % 4).collect();
        let ready: Vec<bool> = (0..len).map(|slot| slot != 0 && random() % 3 == 0).collect();
        let cpu = random() % 4;
        let mine: Vec<usize> = (1..len).filter(|&slot| cpu_of[slot] == cpu).collect();
        let current = (random() % len as u64) as usize;
        let on_cpu = |slot: usize| slot < len && cpu_of[slot] == cpu && ready[slot];
        assert_eq!(task_state::next_in(&mine, current, on_cpu), task_state::next_by(len, current, on_cpu), "{len} {current}");
        // A cursor past the table (it shrank since) starts from the first slot.
        assert_eq!(task_state::next_in(&mine, len + 5, on_cpu), mine.iter().copied().find(|&slot| on_cpu(slot)).unwrap_or(0));
    }
}

#[test]
fn a_task_slot_kept_across_a_shrink_reads_as_empty() {
    // A client waiting for a reply in the second chunk ends; its chunk goes; the server's reply then looks at its slot.
    let mut table: task_table::Table<String> = task_table::Table::new();
    assert!(table.grow());
    let client = task_table::CHUNK + 2;
    table[client] = Some("client".into());
    assert_eq!(table.len(), 2 * task_table::CHUNK);
    table.shrink();
    assert_eq!(table.len(), 2 * task_table::CHUNK, "a chunk with a task stays");
    table[client] = None;
    table.shrink();
    assert_eq!(table.len(), task_table::CHUNK, "the empty chunk at the end goes");
    assert!(table[client].is_none(), "the slot past the end reads as empty, not a panic");
    assert!(table[client].as_mut().is_none());
    table[client] = Some("lost".into());
    assert!(table[client].is_none(), "nothing written past the end stays");
    assert!(table[1].is_none());
    table[1] = Some("first".into());
    table.shrink();
    assert_eq!((table.len(), table[1].as_deref()), (task_table::CHUNK, Some("first")), "the first chunk is kept");
}

#[test]
fn application_slots_are_distinct_fixed_and_launchers_reach_the_new_ones() {
    // 211-KRN-0058: SLOT_SHELL and SLOT_CLIPBOARD below SLOT_DYNAMIC, each named slot once, the loader's own kept.
    use abi::*;
    let named = [SLOT_INIT, SLOT_RTC, SLOT_VFS, SLOT_AUDIO, SLOT_LOADER, SLOT_TTS, SLOT_FILE, SLOT_WINDOW, SLOT_CONSOLE,
        SLOT_SYSINFO, SLOT_LIFECYCLE, SLOT_LOG, SLOT_AUTHORITY, SLOT_KEYBOARD, SLOT_DISPLAY, SLOT_NET, SLOT_SOCKET,
        SLOT_NETWORK, SLOT_NETPOLICY, SLOT_TLS, SLOT_WINDOWS, SLOT_WINDOW_MANAGER, SLOT_GPIO, SLOT_CAMERA, SLOT_BLOCKSTORE,
        SLOT_BLOCKSTORE_READ, SLOT_FIRMWARE, SLOT_PARSE, SLOT_TPM, SLOT_SHELL, SLOT_CLIPBOARD, SLOT_BATTERY, SLOT_WIFI, SLOT_SSH, SLOT_UPDATE];
    let mut seen = std::collections::BTreeSet::new();
    for slot in named {
        assert!((1..SLOT_DYNAMIC).contains(&slot), "slot {} is not a fixed one", slot);
        assert!(seen.insert(slot), "slot {} is named twice", slot);
    }
    assert_eq!(seen.len(), SLOT_DYNAMIC - 1, "every fixed application slot has one name");
    assert_eq!((SLOT_SHELL, SLOT_CLIPBOARD, SLOT_BATTERY, SLOT_WIFI, SLOT_SSH, SLOT_UPDATE, SLOT_DYNAMIC, ABI_VERSION), (30, 31, 32, 33, 34, 35, 36, 13));
    for slot in [SLOT_SHELL, SLOT_CLIPBOARD, SLOT_PARSE, SLOT_CAMERA, SLOT_WINDOW, SLOT_BATTERY, SLOT_WIFI, SLOT_NET, SLOT_SSH] { assert!(LAUNCH_SLOTS.contains(&slot), "a launcher may fill {}", slot); }
    assert!(!LAUNCH_SLOTS.contains(&SLOT_UPDATE), "the updater's client is the shell's alone");
    for slot in [SLOT_RTC, SLOT_VFS, SLOT_AUDIO, SLOT_LOADER, SLOT_TTS] { assert!(!LAUNCH_SLOTS.contains(&slot), "{} stays the loader's", slot); }
    assert!(LAUNCH_SLOTS.iter().all(|&slot| slot < SLOT_DYNAMIC));
    assert!(CAP_SLOTS - SLOT_DYNAMIC >= 64, "dynamic slots left in the initial capability space");
}
