//! The firmware's UEFI variables (351-KRN-0027): BootNext, BootOrder, Boot#### and others, through the kernel's
//! FIRMWARE_VARIABLE call with the privilege in SLOT_FIRMWARE (the shell lends it for REQUEST_FIRMWARE once the user
//! agreed). Names are ASCII here; the kernel passes them to the firmware as UTF-16.
use crate::abi::*;
use crate::mem::Pages;
use crate::sys::{check, syscall, Error, Result};

/// EFI_GLOBAL_VARIABLE: 8BE4DF61-93CA-11D2-AA0D-00E098032B8C, as the firmware stores it.
pub const GLOBAL: [u8; 16] = [0x61, 0xDF, 0xE4, 0x8B, 0xCA, 0x93, 0xD2, 0x11, 0xAA, 0x0D, 0x00, 0xE0, 0x98, 0x03, 0x2B, 0x8C];
pub const NON_VOLATILE: u32 = 1;
pub const BOOTSERVICE_ACCESS: u32 = 2;
pub const RUNTIME_ACCESS: u32 = 4;
/// What boot variables carry.
pub const BOOT_VARIABLE: u32 = NON_VOLATILE | BOOTSERVICE_ACCESS | RUNTIME_ACCESS;

// The request in `page`: GUID, attributes, the name's length in UTF-16 units, the data's length, the name, the data.
fn request(page: &mut [u8], name: &str, guid: &[u8; 16], attributes: u32, data: &[u8]) -> Result<usize> {
    let units = name.len();
    let end = FIRMWARE_HEADER + units * 2;
    if units == 0 || !name.is_ascii() || end + data.len() > page.len() { return Err(Error::Invalid); }
    page[..16].copy_from_slice(guid);
    page[16..20].copy_from_slice(&attributes.to_le_bytes());
    page[20..22].copy_from_slice(&(units as u16).to_le_bytes());
    page[22..26].copy_from_slice(&(data.len() as u32).to_le_bytes());
    for (i, b) in name.bytes().enumerate() { page[FIRMWARE_HEADER + 2 * i] = b; page[FIRMWARE_HEADER + 2 * i + 1] = 0; }
    page[end..end + data.len()].copy_from_slice(data);
    Ok(end)
}

/// Reads variable `name` of `guid` into `out`: its attributes and length. NotFound when it is not set, or the firmware
/// has no runtime variable services.
pub fn get(name: &str, guid: &[u8; 16], out: &mut [u8]) -> Result<(u32, usize)> {
    let mut pages = Pages::new(FIRMWARE_BUFFER).ok_or(Error::NoMemory)?;
    let page = pages.as_mut_slice();
    let end = request(page, name, guid, 0, &[])?;
    let length = check(syscall(SYSCALL_FIRMWARE_VARIABLE, SLOT_FIRMWARE, FIRMWARE_GET, [page.as_ptr() as usize, FIRMWARE_BUFFER, 0, 0]).result)?;
    let attributes = u32::from_le_bytes(page[16..20].try_into().unwrap());
    let n = length.min(out.len());
    out[..n].copy_from_slice(&page[end..end + n]);
    Ok((attributes, length))
}

/// Writes variable `name` of `guid` with `attributes`; empty `data` deletes it.
pub fn set(name: &str, guid: &[u8; 16], attributes: u32, data: &[u8]) -> Result<()> {
    let mut pages = Pages::new(FIRMWARE_BUFFER).ok_or(Error::NoMemory)?;
    let page = pages.as_mut_slice();
    let end = request(page, name, guid, attributes, data)?;
    check(syscall(SYSCALL_FIRMWARE_VARIABLE, SLOT_FIRMWARE, FIRMWARE_SET, [page.as_ptr() as usize, end + data.len(), 0, 0]).result).map(|_| ())
}
