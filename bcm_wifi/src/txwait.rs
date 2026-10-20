// The wait for a frame's transmit status and the transmit ring's room (550-DRV-0027, the audit's H12): the deadline
// is checked before every entry the status FIFO gives, so no stream of other statuses outlasts it, and a slot is
// reused only once the DMA engine has left it. Plain logic, no hardware, so a host test drives it.

// A final status's word (low 16 bits of the FIFO's first register): valid, not intermediate.
pub const VALID: u32 = 1;
pub const INTERMEDIATE: u32 = 0x40;

// A frame's cookie: engine 1 in bits 15-12 (b43's choice, never 0 or 0xFFFF), a generation in bits 11-4, the slot in
// bits 3-0, so that a late status of a slot's earlier frame never answers for the next.
pub fn cookie(generation: u8, slot: usize) -> u16 { 0x2000 | (generation as u16) << 4 | (slot as u16 & 0xF) }

// Whether the ring may take a frame in slot `next`: not while the engine, at `current`, would then reach it again.
pub fn room(next: usize, current: usize, slots: usize) -> bool { (next + 1) % slots != current % slots }

// Waits for `cookie`'s final status: `fifo` gives the next entry (both registers) or None when empty, `now` the time
// in ms, `idle` runs while the FIFO is empty. The status word, or None at `end`: a status read after `end` does not
// count. At most one entry is taken between two looks at the clock.
pub fn wait(mut fifo: impl FnMut() -> Option<(u32, u32)>, cookie: u16, mut now: impl FnMut() -> usize, end: usize,
            mut idle: impl FnMut()) -> Option<u32> {
    loop {
        if now() >= end { return None; }
        match fifo() {
            // 0xFFFFFFFF: the chip is gone, nothing to take.
            Some((first, _)) if first != u32::MAX && first & VALID != 0 => {
                if first & INTERMEDIATE == 0 && (first >> 16) as u16 == cookie && now() < end { return Some(first & 0xFFFF); }
            }
            _ => idle(),
        }
    }
}
