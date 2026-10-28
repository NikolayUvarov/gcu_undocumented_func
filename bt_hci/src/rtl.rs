//! A Realtek controller's patch (hwdocs/rtl8821c-bt.md): from the firmware file, in its "Realtech" format, the patch
//! for the controller's ROM version, its last four bytes set to the file's version, then the configuration file; sent
//! in vendor commands of 252 bytes each. Written from the files' layout, not from another driver's code.

/// Read the ROM version: one return byte after the status.
pub const READ_ROM_VERSION: u16 = 0xFC6D;
/// Download a fragment: an index byte, then the fragment.
pub const DOWNLOAD: u16 = 0xFC20;
pub const FRAGMENT: usize = 252;
pub const MANUFACTURER: u16 = 0x005D;

/// The controllers bt_hci patches: Read Local Version's LMP subversion and HCI revision as the ROM gives them, and
/// the project ID the firmware file must name.
pub const RTL8821C: Chip = Chip { lmp_subver: 0x8821, hci_rev: 0x000C, project: 10, name: "RTL8821C" };

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chip { pub lmp_subver: u16, pub hci_rev: u16, pub project: u8, pub name: &'static str }

const SIGNATURE: &[u8; 8] = b"Realtech";
const EXTENSION: [u8; 4] = [0x51, 0x04, 0xFD, 0x77];
const CONFIG_SIGNATURE: [u8; 4] = [0x55, 0xAB, 0x23, 0x87];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { Signature, Truncated, NoProject, Project(u8), NoPatch(u8), Config, TooLarge }

/// The firmware file's head: its version, its patches' chip IDs, and the project its extension names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Epatch { pub version: u32, pub patches: u16, pub project: u8 }

fn u16_at(b: &[u8], i: usize) -> Option<u16> { b.get(i..i + 2).map(|x| u16::from_le_bytes([x[0], x[1]])) }
fn u32_at(b: &[u8], i: usize) -> Option<u32> { b.get(i..i + 4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]])) }

/// The project ID in the extension at the file's end: entries read backwards from before its signature, each an
/// opcode, a length and that many bytes; opcode 0 is the project ID, 0xFF ends them.
fn project(fw: &[u8]) -> Result<u8, Error> {
    if fw.len() < 14 + 4 || fw[fw.len() - 4..] != EXTENSION { return Err(Error::NoProject); }
    let mut at = fw.len() - 4;
    while at >= 14 + 2 {
        let (opcode, len) = (fw[at - 1], fw[at - 2] as usize);
        if opcode == 0xFF { break; }
        if at < 2 + len { break; }
        if opcode == 0 && len >= 1 { return Ok(fw[at - 2 - len]); }
        at -= 2 + len;
    }
    Err(Error::NoProject)
}

pub fn parse(fw: &[u8]) -> Result<Epatch, Error> {
    if fw.len() < 14 || &fw[..8] != SIGNATURE { return Err(Error::Signature); }
    let version = u32_at(fw, 8).ok_or(Error::Truncated)?;
    let patches = u16_at(fw, 12).ok_or(Error::Truncated)?;
    if fw.len() < 14 + 8 * patches as usize { return Err(Error::Truncated); }
    Ok(Epatch { version, patches, project: project(fw)? })
}

/// The bytes to download into `out` for a controller whose ROM version is `rom`: the patch whose chip ID is `rom` + 1,
/// the file's version over its last four bytes, then `config`. Its length.
pub fn image(fw: &[u8], config: &[u8], chip: &Chip, rom: u8, out: &mut [u8]) -> Result<usize, Error> {
    let e = parse(fw)?;
    if e.project != chip.project { return Err(Error::Project(e.project)); }
    let n = e.patches as usize;
    let want = rom as u16 + 1;
    // The chip IDs, then the lengths, then the offsets: three arrays of `n` after the head.
    let i = (0..n).find(|&i| u16_at(fw, 14 + 2 * i) == Some(want)).ok_or(Error::NoPatch(rom))?;
    let len = u16_at(fw, 14 + 2 * n + 2 * i).ok_or(Error::Truncated)? as usize;
    let at = u32_at(fw, 14 + 4 * n + 4 * i).ok_or(Error::Truncated)? as usize;
    let patch = fw.get(at..at + len).ok_or(Error::Truncated)?;
    if len < 4 { return Err(Error::Truncated); }
    if config.len() < 6 || config[..4] != CONFIG_SIGNATURE { return Err(Error::Config); }
    // Past 255 fragments the index byte would repeat: no Realtek patch is that large.
    if len + config.len() > out.len() || fragments(len + config.len()) > 0xFF { return Err(Error::TooLarge); }
    out[..len].copy_from_slice(patch);
    out[len - 4..len].copy_from_slice(&e.version.to_le_bytes());
    out[len..len + config.len()].copy_from_slice(config);
    Ok(len + config.len())
}

/// The download's fragments: their count, and fragment `i`'s index byte and range. Indices run 0 to 0x7F, then on
/// from 1 again; the last has bit 7 set. The count is the whole fragments and one more, which may be empty.
pub fn fragments(len: usize) -> usize { len / FRAGMENT + 1 }

pub fn fragment(len: usize, i: usize) -> (u8, core::ops::Range<usize>) {
    let mut index = if i > 0x7F { (i & 0x7F) + 1 } else { i } as u8;
    let last = i + 1 == fragments(len);
    if last { index |= 0x80; }
    let start = i * FRAGMENT;
    (index, start..if last { len } else { start + FRAGMENT })
}
