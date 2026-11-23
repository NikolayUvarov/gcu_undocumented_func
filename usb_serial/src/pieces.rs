// A write's pieces (000-LNK-0050): usb_host waits on each bulk transfer and serves its other clients only between two,
// so a long one held a speakerphone's rings 30-45 ms (000-DRV-0094's run C); 256 bytes are 5.6 ms at 460 800 bit/s.
// No system calls: tests/serial_host.rs.
pub const PIECE: usize = 256;

/// The offsets and lengths of `length` bytes in pieces of at most `most` (one at least), in order.
pub fn pieces(length: usize, most: usize) -> impl Iterator<Item = (usize, usize)> {
    let most = most.max(1);
    (0..length.div_ceil(most)).map(move |k| (k * most, most.min(length - k * most)))
}
