//! wm's pointer (000-APP-0061): the 8 × 12 arrow drawn at a size of its own or with wm's scale, in a colour chosen
//! from `COLOURS`, both kept in `data/wm.conf`. No system calls: tests/wm_host.rs.

/// The arrow at 100%: `X` its edge, `W` its fill, the hot spot at the top left.
pub const ARROW: [&[u8; 8]; 12] = [b"X       ", b"XX      ", b"XWX     ", b"XWWX    ", b"XWWWX   ", b"XWWWWX  ", b"XWWWWWX ", b"XWWWWWWX", b"XWWWXXXX", b"XWXWX   ", b"XX XWX  ", b"    XX  "];
/// The sizes offered, in percent of the 8 × 12 arrow; `pointer = auto` draws it at wm's scale.
pub const SIZES: [u16; 9] = [100, 150, 200, 250, 300, 400, 500, 600, 800];
/// The colours offered: a name, the fill and the edge (white's edge is black, black's white, the others' black).
pub const COLOURS: [(&str, u32, u32); 8] = [("white", 0xFF_FF_FF, 0x00_00_00), ("black", 0x00_00_00, 0xFF_FF_FF), ("yellow", 0xFF_E0_00, 0x00_00_00),
    ("orange", 0xFF_8C_1A, 0x00_00_00), ("red", 0xFF_30_30, 0x00_00_00), ("green", 0x30_E0_60, 0x00_00_00), ("cyan", 0x30_E0_F0, 0x00_00_00),
    ("magenta", 0xF0_50_F0, 0x00_00_00)];

/// The size the pointer is drawn at, in percent: the one chosen, or wm's scale for `auto` (None).
pub fn percent(chosen: Option<u16>, scale: u16) -> u16 { chosen.unwrap_or(scale) }

/// The arrow's pixels at `percent`.
pub fn size(percent: u16) -> (usize, usize) { (8 * percent as usize / 100, 12 * percent as usize / 100) }

/// Pixel (col, row) of the arrow `size` pixels large in colour `colour` (an index into `COLOURS`): each takes the 100%
/// arrow's pixel under it; None where the arrow is not.
pub fn pixel(size: (usize, usize), colour: usize, col: usize, row: usize) -> Option<u32> {
    let (_, fill, edge) = COLOURS[colour.min(COLOURS.len() - 1)];
    match ARROW.get(row * 12 / size.1.max(1))?.get(col * 8 / size.0.max(1))? { b'X' => Some(edge), b'W' => Some(fill), _ => None }
}
