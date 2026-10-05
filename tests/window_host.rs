//! Host tests of window surfaces (libmind/src/window.rs, issue 157): the header the broker writes, what the manager
//! accepts from a program it cannot trust, the input queue and the changed rectangle.
#[allow(dead_code)]
#[path = "../common/abi.rs"]
mod abi;
#[allow(dead_code)]
#[path = "../libmind/src/window.rs"]
mod window;

use window::{bytes, Kind, Surface, EVENTS, STATE_CLOSE};

#[allow(dead_code)]
#[repr(align(4096))]
struct Page([u8; 4096]);

fn region(len: usize) -> Vec<Page> { (0..len.div_ceil(4096)).map(|_| Page([0; 4096])).collect() }
fn surface(memory: &mut [Page]) -> Surface { unsafe { Surface::new(memory.as_mut_ptr().cast::<u8>(), memory.len() * 4096) } }
fn raw(memory: &mut [Page]) -> &mut [u8] { unsafe { core::slice::from_raw_parts_mut(memory.as_mut_ptr().cast::<u8>(), memory.len() * 4096) } }

#[test]
fn text_cells_title_and_damage() {
    let mut memory = region(bytes(Kind::Text, 24, 2));
    let s = surface(&mut memory);
    assert_eq!(s.check(), None, "no header yet");
    s.init(Kind::Text, 24, 2, "");
    assert_eq!(s.check(), Some((Kind::Text, 24, 2)));
    s.set_title("часы — clock");
    let mut title = [0u8; window::TITLE];
    let len = s.title(&mut title);
    assert_eq!(core::str::from_utf8(&title[..len]).unwrap(), "часы — clock");
    s.set_cell(3, 1, 'Ж', 0xFFFFFF, 0x000080);
    assert_eq!(s.cell(3, 1), Some(('Ж', 0xFFFFFF, 0x000080)));
    assert_eq!(s.cell(24, 0), None);
    let before = s.changes();
    s.changed(Some((20, 1, 10, 5)));
    assert_eq!(s.changes(), before + 1);
    assert_eq!(s.damage(), Some((20, 1, 4, 1)), "clipped to the surface");
}

#[test]
fn a_hostile_program_cannot_claim_more_than_the_memory() {
    let mut memory = region(bytes(Kind::Text, 24, 2));
    let s = surface(&mut memory);
    s.init(Kind::Text, 24, 2, "");
    assert!(!s.set_size(200, 100), "the program's own call refuses");
    // Written behind the library's back: a size beyond the memory, an unknown kind, a title without UTF-8.
    raw(&mut memory)[8..12].copy_from_slice(&(200u32 | 100 << 16).to_le_bytes());
    let s = surface(&mut memory);
    assert_eq!(s.check(), None);
    assert_eq!(s.cell(0, 0), None);
    raw(&mut memory)[8..12].copy_from_slice(&(24u32 | 2 << 16).to_le_bytes());
    raw(&mut memory)[4..8].copy_from_slice(&7u32.to_le_bytes());
    assert_eq!(surface(&mut memory).check(), None);
    raw(&mut memory)[4..8].copy_from_slice(&1u32.to_le_bytes());
    raw(&mut memory)[32..36].copy_from_slice(&[b'o', b'k', 0xFF, b'x']);
    let mut title = [0u8; window::TITLE];
    assert_eq!(surface(&mut memory).title(&mut title), 2);
}

#[test]
fn events_size_requests_and_state() {
    let mut memory = region(bytes(Kind::Pixels, 16, 16));
    let s = surface(&mut memory);
    s.init(Kind::Pixels, 16, 16, "p");
    for n in 0..EVENTS { assert!(s.push_event(n + 1)); }
    assert!(!s.push_event(99), "full");
    for n in 0..EVENTS { assert_eq!(s.event(), Some(n + 1)); }
    assert_eq!(s.event(), None);
    // A head counter nobody could have written is ignored.
    raw(&mut memory)[192..196].copy_from_slice(&1000u32.to_le_bytes());
    assert_eq!(surface(&mut memory).event(), None);
    let s = surface(&mut memory);
    s.ask_size(32, 8);
    assert_eq!(s.wanted(), Some((32, 8)));
    assert_eq!(s.wanted(), None, "taken once");
    s.set_state(STATE_CLOSE);
    assert_eq!(s.state(), STATE_CLOSE);
}

#[test]
fn cursor_resize_and_pixels() {
    // A text surface made for the screen's cells, drawn smaller: the size changes within the memory.
    let mut memory = region(bytes(Kind::Text, 80, 25));
    let s = surface(&mut memory);
    s.init(Kind::Text, 80, 25, "fm");
    assert!(s.fits(80, 25) && !s.fits(80, 40), "the memory is whole pages");
    assert!(s.set_size(40, 10));
    assert_eq!((s.size(), s.check()), ((40, 10), Some((Kind::Text, 40, 10))));
    assert_eq!(s.cursor(), None);
    s.set_cursor(Some((39, 9)));
    assert_eq!(s.cursor(), Some((39, 9)));
    assert!(s.set_size(20, 5));
    assert_eq!(s.cursor(), None, "a cursor outside the size is not shown");
    s.set_cursor(None);
    assert!(s.set_size(40, 10));
    assert_eq!(s.cursor(), None);
    // Pixels: the manager reads what the program wrote, inside the size only.
    let mut memory = region(bytes(Kind::Pixels, 8, 4));
    let p = surface(&mut memory);
    p.init(Kind::Pixels, 8, 4, "clock");
    unsafe { p.content().cast::<u32>().add(3 * 8 + 7).write(0x00A6E3A1); }
    assert_eq!(p.pixel(7, 3), Some(0x00A6E3A1));
    assert_eq!((p.pixel(8, 0), p.pixel(0, 4), s.pixel(0, 0)), (None, None, None), "outside, or not a pixel surface");
}

#[test]
fn pointer_events_of_a_window() {
    use abi::{event_key, pointer_absolute_fields, pointer_event, KEY_POINTER, POINTER_LEFT, POINTER_SCALE};
    let word = window::pointer_at(POINTER_LEFT, 77, 21, -1);
    assert_eq!(event_key(word), KEY_POINTER);
    assert_eq!(window::pointer_position(word), Some((77, 21)));
    assert_eq!(pointer_absolute_fields(word), Some((POINTER_LEFT, 77, 21, -1)));
    assert_eq!(window::pointer_position(window::pointer_at(0, 300, 5000, 9)), Some((300, POINTER_SCALE - 1)), "kept within the field");
    assert_eq!(pointer_absolute_fields(window::pointer_at(0, 1, 2, 9)), Some((0, 1, 2, 7)), "the wheel too");
    // The kernel's mouse events carry motion, not a position.
    assert_eq!(window::pointer_position(pointer_event(0, 5, -3, 0)), None);
    assert_eq!(window::pointer_position(abi::POINTER_ABSOLUTE), None, "not a pointer event");
}
