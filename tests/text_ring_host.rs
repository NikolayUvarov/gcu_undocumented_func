//! Host tests of the text ring (common/text_ring.rs): a task's text input and the console channel's both ways
//! (000-KRN-0094, 100000-KRN-0222): its bounds, a full ring, and the wrap past its end.
#[path = "../common/abi.rs"]
#[allow(dead_code)]
mod abi;
#[path = "../common/text_ring.rs"]
mod text_ring;

use abi::TEXT_QUEUE;
use text_ring::Text;

#[test]
fn a_full_ring_takes_nothing_more_and_gives_back_what_it_took() {
    let mut ring = Box::new(Text::new());
    let data: Vec<u8> = (0..TEXT_QUEUE + 100).map(|i| i as u8).collect();
    assert_eq!(ring.push(&data), TEXT_QUEUE);
    assert_eq!(ring.push(b"more"), 0);
    let mut out = vec![0u8; TEXT_QUEUE + 1];
    assert_eq!(ring.pop(&mut out), TEXT_QUEUE);
    assert_eq!(&out[..TEXT_QUEUE], &data[..TEXT_QUEUE]);
    assert_eq!((ring.pop(&mut out), ring.len), (0, 0));
}

#[test]
fn bytes_come_out_in_order_across_the_end() {
    let mut ring = Box::new(Text::new());
    let mut sent = 0u32; let mut got = Vec::new();
    // Pushes of 700 bytes and pops of 500, many times round the ring: the order holds and no byte is lost.
    for _ in 0..40 {
        let chunk: Vec<u8> = (0..700).map(|i| ((sent + i) % 251) as u8).collect();
        let taken = ring.push(&chunk); sent += taken as u32;
        let mut out = [0u8; 500]; let n = ring.pop(&mut out); got.extend_from_slice(&out[..n]);
    }
    let mut out = vec![0u8; TEXT_QUEUE]; let n = ring.pop(&mut out); got.extend_from_slice(&out[..n]);
    assert_eq!(got.len() as u32, sent);
    assert!(got.iter().enumerate().all(|(i, &b)| b == (i as u32 % 251) as u8));
}

#[test]
fn an_empty_buffer_takes_nothing() {
    let mut ring = Box::new(Text::new());
    assert_eq!(ring.push(b"abc"), 3);
    assert_eq!(ring.pop(&mut []), 0);
    let mut one = [0u8; 1];
    assert_eq!((ring.pop(&mut one), one[0], ring.len), (1, b'a', 2));
    assert_eq!(ring.push(&[]), 0);
}
