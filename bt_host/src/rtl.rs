//! A Realtek controller's patch: from the firmware file, the patch for the controller's ROM version, sent in vendor
//! commands of 252 bytes each.
//! - **The "Realtech" format** (the RTL8821C, hwdocs/rtl8821c-bt.md): the patch, its last four bytes set to the file's
//!   version, then the configuration file.
//! - **The "RTBTCore" format** (the RTL8852B, hwdocs/rtl8852b-bt.md, 700-BLT-0004): the patch sections' parts for the
//!   ROM version, in rising priority; no configuration file.
//!
//! Written from the files' layout, not from another driver's code.

/// Read the ROM version: one return byte after the status.
pub const READ_ROM_VERSION: u16 = 0xFC6D;
/// Download a fragment: an index byte, then the fragment.
pub const DOWNLOAD: u16 = 0xFC20;
pub const FRAGMENT: usize = 252;
pub const MANUFACTURER: u16 = 0x005D;

/// The controllers bt_hci patches: Read Local Version's LMP subversion and HCI revision as the ROM gives them, and
/// the project ID the firmware file must name.
pub const RTL8821C: Chip = Chip { lmp_subver: 0x8821, hci_rev: 0x000C, project: 10, name: "RTL8821C" };
/// The Bluetooth half of the Ryzen's RTL8852BE, on USB as 0BDA:B85B (700-BLT-0004).
pub const RTL8852B: Chip = Chip { lmp_subver: 0x8852, hci_rev: 0x000B, project: 20, name: "RTL8852B" };

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chip { pub lmp_subver: u16, pub hci_rev: u16, pub project: u8, pub name: &'static str }

const SIGNATURE: &[u8; 8] = b"Realtech";
const SIGNATURE_CORE: &[u8; 8] = b"RTBTCore";
/// An "RTBTCore" file's head: the signature, an 8-byte version, the section count.
const CORE_HEAD: usize = 20;
/// The sections whose parts are patch bytes; a security section asks for a key this loader does not read.
const SECTION_PATCH: u32 = 1;
const SECTION_DUMMY: u32 = 2;
const SECTION_SECURITY: u32 = 3;
const EXTENSION: [u8; 4] = [0x51, 0x04, 0xFD, 0x77];
const CONFIG_SIGNATURE: [u8; 4] = [0x55, 0xAB, 0x23, 0x87];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { Signature, Truncated, NoProject, Project(u8), NoPatch(u8), Config, TooLarge, Security, Priority(u8) }

/// The firmware file's head: its version, its patches' chip IDs, and the project its extension names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Epatch { pub version: u32, pub patches: u16, pub project: u8 }

/// An "RTBTCore" file's head: its version, its sections as counted and as found whole before the extension, and the
/// project its extension names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Core { pub version: u64, pub counted: u32, pub found: u32, pub project: u8 }

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
    if len + config.len() > out.len() { return Err(Error::TooLarge); }
    out[..len].copy_from_slice(patch);
    out[len - 4..len].copy_from_slice(&e.version.to_le_bytes());
    out[len..len + config.len()].copy_from_slice(config);
    Ok(len + config.len())
}

/// The download's fragments: their count, and fragment `i`'s index byte and range. Indices run 0 to 0x7F, then on
/// from 1 again; the last has bit 7 set. The count is the whole fragments and one more, which may be empty.
pub fn fragments(len: usize) -> usize { len / FRAGMENT + 1 }

pub fn fragment(len: usize, i: usize) -> (u8, core::ops::Range<usize>) {
    // 0, then 1 to 0x7F over and over: the 256th fragment is 1 again, not 0x80, the last one's mark (700-BLT-0004).
    let mut index = if i == 0 { 0 } else { ((i - 1) % 0x7F + 1) as u8 };
    let last = i + 1 == fragments(len);
    if last { index |= 0x80; }
    let start = i * FRAGMENT;
    (index, start..if last { len } else { start + FRAGMENT })
}

/// Whether `fw` is in the "RTBTCore" format.
pub fn is_core(fw: &[u8]) -> bool { fw.len() >= 8 && &fw[..8] == SIGNATURE_CORE }

// Each whole section before the file's last 7 bytes (the end of its extension): its opcode and its body, as many as the
// head counts and fit. The pinned RTL8852B file holds the patch section (opcode 1) and an empty one (opcode 8).
fn sections(fw: &[u8]) -> impl Iterator<Item = (u32, &[u8])> {
    let counted = u32_at(fw, 16).unwrap_or(0);
    let end = fw.len().saturating_sub(EXTENSION.len() + 3);
    let mut at = CORE_HEAD;
    (0..counted).map_while(move |_| {
        let (opcode, len) = (u32_at(fw, at)?, u32_at(fw, at + 4)? as usize);
        let body = fw.get(at + 8..(at + 8).checked_add(len)?).filter(|_| at + 8 + len <= end)?;
        at += 8 + len;
        Some((opcode, body))
    })
}

// A section's parts: each its ECO (the ROM version plus one), its priority and its bytes, after a 4-byte head that
// counts them.
fn parts(body: &[u8]) -> impl Iterator<Item = (u8, u8, &[u8])> {
    let counted = u16_at(body, 0).unwrap_or(0);
    let mut at = 4;
    (0..counted).map_while(move |_| {
        let (eco, priority, len) = (*body.get(at)?, *body.get(at + 1)?, u32_at(body, at + 4)? as usize);
        let bytes = body.get(at + 8..(at + 8).checked_add(len)?)?;
        at += 8 + len;
        Some((eco, priority, bytes))
    })
}

pub fn parse_core(fw: &[u8]) -> Result<Core, Error> {
    if fw.len() < CORE_HEAD || !is_core(fw) { return Err(Error::Signature); }
    let version = u64::from_le_bytes(fw[8..16].try_into().unwrap());
    let counted = u32_at(fw, 16).ok_or(Error::Truncated)?;
    Ok(Core { version, counted, found: sections(fw).count() as u32, project: project(fw)? })
}

/// The bytes to download into `out` for a controller whose ROM version is `rom`, from an "RTBTCore" file: the parts
/// whose ECO is `rom` + 1 of its patch and dummy sections, in rising priority. Two of one priority are refused, as the
/// file does not say their order, and so is a security section. Its length.
pub fn image_core(fw: &[u8], chip: &Chip, rom: u8, out: &mut [u8]) -> Result<usize, Error> {
    let e = parse_core(fw)?;
    if e.project != chip.project { return Err(Error::Project(e.project)); }
    let eco = rom.wrapping_add(1);
    if sections(fw).any(|(opcode, _)| opcode == SECTION_SECURITY) { return Err(Error::Security); }
    let chosen = || sections(fw).filter(|&(op, _)| op == SECTION_PATCH || op == SECTION_DUMMY).flat_map(|(_, body)| parts(body)).filter(move |p| p.0 == eco);
    let mut len = 0;
    for priority in 0..=u8::MAX {
        let mut of = chosen().filter(|p| p.1 == priority);
        let Some((_, _, bytes)) = of.next() else { continue };
        if of.next().is_some() { return Err(Error::Priority(priority)); }
        out.get_mut(len..len + bytes.len()).ok_or(Error::TooLarge)?.copy_from_slice(bytes);
        len += bytes.len();
    }
    if len == 0 { return Err(Error::NoPatch(rom)); }
    Ok(len)
}
