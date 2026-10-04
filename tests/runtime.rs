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
#[path = "../kernel/src/paging.rs"]
mod paging;
#[path = "../kernel/src/task_state.rs"]
mod task_state;
#[path = "../kernel/src/user_heap.rs"]
mod user_heap;

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
