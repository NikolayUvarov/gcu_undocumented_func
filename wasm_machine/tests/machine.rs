//! Host tests of the page's devices (215-PRT-0017): the PCI configuration space the kernel reads, a BAR sized and
//! placed, the PL031, and virtio-input's events into the buffers a driver offered, as `virtio_input` offers them.
use wasm_machine::{Board, Machine, LINES, RTC};

const BASE: u64 = 0x1000;
const TABLET_BAR: u64 = 0x1_0001_0000;
const KEYBOARD_BAR: u64 = 0x1_0002_0000;
// The keyboard's queue 0 in RAM: descriptors, the driver's (available) ring, the device's (used) ring, one buffer.
const DESC: u64 = 0x2000;
const AVAIL: u64 = 0x3000;
const USED: u64 = 0x4000;
const BUFFER: u64 = 0x5000;

struct Page { ram: Vec<u8>, wall: u64, raised: Vec<u8> }
impl Board for Page {
    fn ram(&mut self) -> (&mut [u8], u64) { (&mut self.ram, BASE) }
    fn wall_s(&mut self) -> u64 { self.wall }
    fn raise(&mut self, line: u8) { self.raised.push(line); }
}
impl Page {
    fn new() -> Self { Page { ram: vec![0; 0x10000], wall: 0x1_2345_6789, raised: Vec::new() } }
    fn put(&mut self, at: u64, bytes: &[u8]) { let at = (at - BASE) as usize; self.ram[at..at + bytes.len()].copy_from_slice(bytes); }
    fn get(&self, at: u64, len: usize) -> &[u8] { let at = (at - BASE) as usize; &self.ram[at..at + len] }
    fn u16_at(&self, at: u64) -> u16 { u16::from_le_bytes(self.get(at, 2).try_into().unwrap()) }
}

#[test]
fn the_configuration_space_shows_two_modern_virtio_inputs_on_bus_0() {
    let mut m = Machine::new();
    for slot in [1, 2] {
        assert_eq!(m.config_read(0, slot, 0, 0x00), 0x1052_1AF4, "virtio 1.0 input, Red Hat's vendor");
        assert_eq!(m.config_read(0, slot, 0, 0x08) >> 8, 0x09_8000, "class: input, other");
        assert_eq!(m.config_read(0, slot, 0, 0x3C) & 0xFF, LINES[slot as usize - 1] as u32);
        // The capabilities: common, notify, ISR and device configuration, in a list from 0x40.
        assert_eq!(m.config_read(0, slot, 0, 0x34), 0x40);
        let kinds: Vec<u32> = [0x40, 0x50, 0x64, 0x74].iter().map(|&at| m.config_read(0, slot, 0, at) >> 24).collect();
        assert_eq!(kinds, [1, 2, 3, 4]);
    }
    assert_eq!(m.config_read(0, 3, 0, 0), u32::MAX, "nothing at device 3");
    assert_eq!(m.config_read(1, 1, 0, 0), u32::MAX, "nothing on bus 1");
    // A BAR sized answers its 16 KiB, then takes the address written.
    m.config_write(0, 1, 0, 0x10, u32::MAX);
    assert_eq!(m.config_read(0, 1, 0, 0x10), 0xFFFF_C004);
    m.config_write(0, 1, 0, 0x10, 0x0004_0000);
    m.config_write(0, 1, 0, 0x14, 0x2);
    assert_eq!((m.config_read(0, 1, 0, 0x10), m.config_read(0, 1, 0, 0x14)), (0x0004_0004, 0x2));
}

#[test]
fn the_pl031_reads_the_pages_wall_clock_and_writes_nothing() {
    let (mut m, mut page) = (Machine::new(), Page::new());
    assert_eq!(m.read(&mut page, RTC, 4), Some(0x2345_6789), "RTCDR: the low 32 bits of the seconds");
    assert_eq!(m.read(&mut page, RTC + 0xFE0, 4), Some(0x31), "PL031's peripheral ID");
    assert!(m.write(&mut page, RTC, 4, 0));
    assert_eq!(m.read(&mut page, 0x2_0000_0000, 4), None, "no device there");
    assert!(!m.write(&mut page, 0x2_0000_0000, 4, 0));
}

// The keyboard set up as a driver does: features, its queue 0's three rings, the queue on, DRIVER_OK.
fn keyboard_ready(m: &mut Machine, page: &mut Page) {
    let w = |m: &mut Machine, page: &mut Page, at: u64, size: u32, value: u64| assert!(m.write(page, KEYBOARD_BAR + at, size, value));
    w(m, page, 0x16, 2, 0);
    w(m, page, 0x20, 8, DESC);
    w(m, page, 0x28, 8, AVAIL);
    w(m, page, 0x30, 8, USED);
    w(m, page, 0x1C, 2, 1);
    w(m, page, 0x14, 1, 0x0F);
}

// One 8-byte buffer offered: descriptor 0, the available ring's entry and index.
fn offer(page: &mut Page, index: u16) {
    let mut desc = BUFFER.to_le_bytes().to_vec();
    desc.extend_from_slice(&8u32.to_le_bytes());
    desc.extend_from_slice(&[2, 0, 0, 0]); // VIRTQ_DESC_F_WRITE
    page.put(DESC, &desc);
    page.put(AVAIL + 4 + 2 * (index as u64 % 64), &0u16.to_le_bytes());
    page.put(AVAIL + 2, &(index + 1).to_le_bytes());
}

#[test]
fn a_key_goes_into_the_buffer_the_driver_offered_and_its_line_rises() {
    let (mut m, mut page) = (Machine::new(), Page::new());
    assert_eq!(m.read(&mut page, KEYBOARD_BAR + 0x18, 2), Some(64), "the queue's size");
    keyboard_ready(&mut m, &mut page);
    offer(&mut page, 0);
    m.input(&mut page, 1, 1, 30, 1); // EV_KEY, KEY_A, pressed
    assert_eq!(page.get(BUFFER, 8), [1, 0, 30, 0, 1, 0, 0, 0]);
    assert_eq!(page.u16_at(USED + 2), 1, "the used ring's index");
    assert_eq!(page.get(USED + 4, 8), [0, 0, 0, 0, 8, 0, 0, 0], "descriptor 0, 8 bytes");
    assert_eq!(page.raised, [LINES[1]]);
    // The ISR reads once, then clears.
    assert_eq!(m.read(&mut page, KEYBOARD_BAR + 0x2000, 1), Some(1));
    assert_eq!(m.read(&mut page, KEYBOARD_BAR + 0x2000, 1), Some(0));
}

#[test]
fn an_event_with_no_buffer_waits_for_the_drivers_next_one() {
    let (mut m, mut page) = (Machine::new(), Page::new());
    keyboard_ready(&mut m, &mut page);
    m.input(&mut page, 1, 1, 28, 0); // KEY_ENTER, released: no buffer yet
    assert!(page.raised.is_empty());
    assert_eq!(page.u16_at(USED + 2), 0);
    offer(&mut page, 0);
    assert!(m.write(&mut page, KEYBOARD_BAR + 0x1000, 2, 0), "the driver's notify");
    assert_eq!(page.get(BUFFER, 8), [1, 0, 28, 0, 0, 0, 0, 0]);
    assert_eq!(page.raised, [LINES[1]]);
    // The tablet's events stay the tablet's: nothing set up there, nothing delivered.
    m.input(&mut page, 0, 3, 0, 100);
    assert_eq!(page.raised, [LINES[1]]);
    assert_eq!(m.read(&mut page, TABLET_BAR + 0x2000, 1), Some(0));
}

#[test]
fn a_reset_forgets_the_queue_and_keeps_the_events() {
    let (mut m, mut page) = (Machine::new(), Page::new());
    keyboard_ready(&mut m, &mut page);
    assert!(m.write(&mut page, KEYBOARD_BAR + 0x14, 1, 0));
    m.input(&mut page, 1, 1, 30, 1);
    assert!(page.raised.is_empty(), "no queue after the reset");
    keyboard_ready(&mut m, &mut page);
    offer(&mut page, 0);
    assert!(m.write(&mut page, KEYBOARD_BAR + 0x1000, 2, 0));
    assert_eq!(page.get(BUFFER, 8), [1, 0, 30, 0, 1, 0, 0, 0]);
}
