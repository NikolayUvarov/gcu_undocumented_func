// What link.conf's words, which init passes as link's arguments, still set (801-LNK-0044): the line's rate and this
// machine's address on a shared line. No word limits what the cable may do: init's grants decide that, and with no
// link.conf at all link serves at DEFAULT_BAUD with no address. Host-tested in tests/link_conf_host.rs.

/// The rate with no `baud=` word: the bench's, as scripts/build_and_write.sh --link writes it and effector485 runs.
pub const DEFAULT_BAUD: u32 = 460_800;
/// Tried next where the line refuses that: a UART that keeps the rate its firmware left (docs/link/mindlink.md).
pub const FALLBACK_BAUD: u32 = 115_200;
/// The highest address on a shared line (linkproto::frame::ADDRESS_MAX).
pub const ADDRESS_MAX: u8 = 247;

/// The rates to try in turn and how many: `baud=<rate>` first where it names one, then DEFAULT_BAUD and FALLBACK_BAUD,
/// each once. A rate the converter refuses is not the end of the cable.
pub fn rates(words: &str) -> ([u32; 3], usize) {
    let named = words.split_ascii_whitespace().find_map(|w| w.strip_prefix("baud=")?.parse::<u32>().ok()).filter(|&r| r > 0);
    let mut out = [0u32; 3];
    let mut n = 0;
    for rate in named.into_iter().chain([DEFAULT_BAUD, FALLBACK_BAUD]) {
        if !out[..n].contains(&rate) { out[n] = rate; n += 1; }
    }
    (out, n)
}

/// This machine's address on a line shared by several (800-LNK-0026): `address=<1..247>`; Ok(0) without the word, a
/// line with one slave; Err with the word's value when it is out of range, which also serves a line of one.
pub fn address(words: &str) -> Result<u8, &str> {
    let Some(word) = words.split_ascii_whitespace().find_map(|w| w.strip_prefix("address=")) else { return Ok(0) };
    match word.parse::<u8>() {
        Ok(a) if (1..=ADDRESS_MAX).contains(&a) => Ok(a),
        _ => Err(word),
    }
}
