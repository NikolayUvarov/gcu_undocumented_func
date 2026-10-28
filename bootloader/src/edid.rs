// The monitor's preferred mode from its EDID, and the graphics mode the loader picks with it (211-PRT-0011); kept apart
// from the loader for the host tests (tests/edid_host.rs).

/// The largest screen the services are sized for: the compositor's and the window broker's memory (211-KRN-0018).
pub const PIXELS_MAX: usize = 2880 * 1800;

const HEADER: [u8; 8] = [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00];

/// Why an EDID gave no preferred mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused { Short, Header, Checksum, NoTiming }

/// The preferred timing's width and height: the first detailed timing descriptor of the base block (bytes 54–71), which
/// EDID 1.3 and later make the preferred one. The base block must have its header and a zero byte sum.
pub fn preferred(edid: &[u8]) -> Result<(usize, usize), Refused> {
    let block = edid.get(..128).ok_or(Refused::Short)?;
    if block[..8] != HEADER { return Err(Refused::Header); }
    if block.iter().fold(0u8, |sum, &b| sum.wrapping_add(b)) != 0 { return Err(Refused::Checksum); }
    let d = &block[54..72];
    // A pixel clock of zero marks a display descriptor (a name, limits), not a timing.
    if d[0] == 0 && d[1] == 0 { return Err(Refused::NoTiming); }
    let width = d[2] as usize | (d[4] as usize & 0xF0) << 4;
    let height = d[5] as usize | (d[7] as usize & 0xF0) << 4;
    if width == 0 || height == 0 { return Err(Refused::NoTiming); }
    Ok((width, height))
}

/// What the loader does with the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    /// Mode `n` is the monitor's preferred one: set it (or keep it, if current).
    Preferred(usize),
    /// The firmware's mode stays: no preferred mode, or it is not listed with a linear framebuffer, or it is larger than
    /// the services are sized for.
    Keep,
    /// The firmware's mode has no linear framebuffer: the largest drawable mode up to 1920x1200, mode `n`.
    Largest(usize),
    /// Nothing drawable is listed.
    Nothing,
}

/// The mode to use among `modes` (width, height, drawable), with the firmware's current one drawable or not and the
/// monitor's preferred size if its EDID gave one. Without an EDID the firmware's mode is kept: a mode the monitor did not
/// say it shows may leave the screen dark.
pub fn choose(modes: &[(usize, usize, bool)], current_drawable: bool, preferred: Option<(usize, usize)>) -> Choice {
    if let Some((w, h)) = preferred.filter(|&(w, h)| w * h <= PIXELS_MAX) {
        if let Some(n) = modes.iter().position(|&(mw, mh, ok)| ok && mw == w && mh == h) { return Choice::Preferred(n); }
    }
    if current_drawable { return Choice::Keep; }
    let area = |&(w, h, _): &(usize, usize, bool)| if w <= 1920 && h <= 1200 { w * h } else { 0 };
    modes.iter().enumerate().filter(|(_, m)| m.2).max_by_key(|(_, m)| area(m)).map_or(Choice::Nothing, |(n, _)| Choice::Largest(n))
}
