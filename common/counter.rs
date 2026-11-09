#![allow(dead_code)]
// The generic timer's counter on a core whose reads can be wrong now and then (100000-PRT-0012): the A64's erratum the
// device tree names `allwinner,erratum-unknown1`. Pure: the read is given; the kernel's clock calls it.

/// The most reads of one value, as Linux takes (drivers/clocksource/arm_arch_timer.c, the sun50i workaround).
pub const TRIES: u32 = 150;

/// A read whose low 9 bits are not all ones or all zeros, the values the A64's wrong reads have; the last read when
/// TRIES gave none.
pub fn sun50i_stable(mut read: impl FnMut() -> u64) -> u64 {
    let mut value = read();
    for _ in 1..TRIES {
        if value.wrapping_add(1) & 0x1FF > 1 { break; }
        value = read();
    }
    value
}
