//! Host tests of `record -s` (record/src/shrink.rs, 000-APP-0089): each block of pixels averaged into one, per colour.
extern crate alloc;
#[path = "../record/src/shrink.rs"]
mod shrink;

#[test]
fn a_frame_is_halved_by_averaging_each_block() {
    // 5 × 3 pixels, stride 6: the last column and row make no whole block of 2 and are left out.
    let mut pixels = vec![0u32; 6 * 3];
    pixels[0] = 0xFF0000; pixels[1] = 0x00FF00; pixels[6] = 0x0000FF; pixels[7] = 0x000000;
    for x in 2..4 { pixels[x] = 0x808080; pixels[6 + x] = 0x808080; }
    pixels[4] = 0xFFFFFF; pixels[12] = 0xFFFFFF;
    let mut out = Vec::new();
    shrink::shrink(&pixels, 5, 3, 6, 2, &mut out);
    assert_eq!(out, [0x404040, 0x808080]);
}

#[test]
fn a_scale_of_one_keeps_the_frame_and_rows_are_packed() {
    let pixels = [1, 2, 3, 0xDEAD, 4, 5, 6, 0xBEEF];
    let mut out = vec![9; 20];
    shrink::shrink(&pixels, 3, 2, 4, 1, &mut out);
    assert_eq!(out, [1, 2, 3, 4, 5, 6]);
}
