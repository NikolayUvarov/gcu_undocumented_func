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
    pub const AREA: usize = 1024;
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
