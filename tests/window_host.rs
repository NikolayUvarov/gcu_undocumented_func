//! Host tests of window surfaces (libmind/src/window.rs, issue 157): the header the broker writes, what the manager
//! accepts from a program it cannot trust, the input queue and the changed rectangle.
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
