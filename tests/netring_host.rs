//! Host tests of the shared frame ring (libmind/src/netring.rs, issue 107): order and wrap-around, a full ring, the
//! checksum fields, and what a hostile peer can do with lengths and counters.
#[path = "../libmind/src/netring.rs"]
mod netring;

use netring::{Frame, Ring, BYTES, FRAME_MAX, SLOTS};

#[repr(align(4096))]
struct Region([u8; BYTES]);

fn region() -> Box<Region> { Box::new(Region([0; BYTES])) }
fn ring(region: &mut Region) -> Ring { let r = unsafe { Ring::new(region.0.as_mut_ptr()) }; r.reset(); r }
fn frame(n: u8, len: usize) -> Vec<u8> { (0..len).map(|i| n.wrapping_add(i as u8)).collect() }

#[test]
fn frames_keep_order_through_many_wraps() {
    let mut region = region();
    let r = ring(&mut region);
    let mut out = [0u8; FRAME_MAX];
    for n in 0..(5 * SLOTS) {
        let f = frame(n as u8, 60 + n % 1400);
        assert!(r.send(&f, 34, 16));
        assert!(r.sending());
        assert_eq!(r.take_sent(&mut out), Some(Frame { len: f.len(), start: 34, offset: 16 }));
        assert_eq!(&out[..f.len()], &f[..]);
        assert!(r.deliver(&f));
        assert_eq!(r.receive(&mut out), Some(Frame { len: f.len(), start: 0, offset: 0 }));
    }
    assert!(!r.sending());
    assert_eq!(r.receive(&mut out), None);
}

#[test]
fn a_full_ring_refuses_until_a_slot_is_taken() {
    let mut region = region();
    let r = ring(&mut region);
    let mut out = [0u8; FRAME_MAX];
    for n in 0..SLOTS { assert!(r.deliver(&frame(n as u8, 64))); }
    assert!(!r.deliver(&frame(0, 64)));
    assert!(r.receive(&mut out).is_some());
    assert!(r.deliver(&frame(0, 64)));
    assert!(!r.send(&[0u8; FRAME_MAX + 1], 0, 0), "an oversized frame is refused");
}

#[test]
fn a_hostile_peer_cannot_make_the_reader_overrun() {
    let mut region = region();
    let r = ring(&mut region);
    let mut out = [0u8; FRAME_MAX];
    // A length beyond the slot's frame: the slot is skipped as empty, the next frame still arrives.
    assert!(r.deliver(&frame(1, 64)));
    assert!(r.deliver(&frame(2, 80)));
    region.0[4096 + SLOTS * 2048..][..2].copy_from_slice(&60000u16.to_le_bytes());
    assert_eq!(r.receive(&mut out).map(|f| f.len), Some(0));
    assert_eq!(r.receive(&mut out).map(|f| f.len), Some(80));
    // A head counter nobody could have written (more than SLOTS ahead) is ignored.
    region.0[128..132].copy_from_slice(&1000u32.to_le_bytes());
    assert_eq!(r.receive(&mut out), None);
    // A tail counter ahead of the head makes the writer wait instead of overwriting.
    let mut region = self::region();
    let r = ring(&mut region);
    region.0[64..68].copy_from_slice(&5u32.to_le_bytes());
    assert!(!r.send(&frame(0, 64), 0, 0));
}
