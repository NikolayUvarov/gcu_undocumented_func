// Pin controllers in the DSDT and SSDTs (issue 206): without an AML interpreter, a bounded scan for a device's
// `Name (_HID, "<id>")` with a known pin controller ID, then the first memory window in the `Name (_CRS, Buffer)`
// that follows it in the same device. A `_CRS` that is a method (the Raspberry Pi 4's EDK2 firmware patches the base at
// run time) or anything else the scan cannot read gives the controller without a window. No allocation and no system
// state: tests/aml_host.rs.

/// The pin controllers the kernel knows by their ACPI hardware ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pins { Bcm2711, Pl061 }

const IDS: [(&[u8], Pins); 2] = [(b"BCM2845", Pins::Bcm2711), (b"ARMH0061", Pins::Pl061)];
const NAME_OP: u8 = 0x08; const STRING_PREFIX: u8 = 0x0D; const BUFFER_OP: u8 = 0x11; const EXT_OP: u8 = 0x5B; const DEVICE_OP: u8 = 0x82;
const SEARCH: usize = 1024; // how far after the _HID its _CRS may be

/// Each known pin controller in `aml` (a definition block without its header): its kind and, if a static `_CRS`
/// gives it, its register base and size.
pub fn pin_controllers(aml: &[u8], mut found: impl FnMut(Pins, Option<(u64, u64)>)) {
    let mut at = 0;
    while let Some(offset) = find(&aml[at..], &[NAME_OP, b'_', b'H', b'I', b'D', STRING_PREFIX]) {
        let start = at + offset + 6;
        at = start;
        let Some(&(_, kind)) = IDS.iter().find(|(id, _)| aml.get(start..start + id.len()) == Some(id) && aml.get(start + id.len()) == Some(&0)) else { continue };
        // The _CRS of the same device: before the next device starts.
        let end = (start + SEARCH).min(aml.len());
        let scope = &aml[start..end];
        let scope = &scope[..find(scope, &[EXT_OP, DEVICE_OP]).unwrap_or(scope.len())];
        let window = find(scope, &[NAME_OP, b'_', b'C', b'R', b'S', BUFFER_OP]).and_then(|crs| buffer(&scope[crs + 6..])).and_then(memory_window);
        found(kind, window);
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> { haystack.windows(needle.len()).position(|w| w == needle) }

// A Buffer's contents after its BufferOp: PkgLength, the size (an integer), the bytes.
fn buffer(bytes: &[u8]) -> Option<&[u8]> {
    let lead = *bytes.first()?;
    let follow = (lead >> 6) as usize;
    let length = if follow == 0 { (lead & 0x3F) as usize } else { (1..=follow).try_fold((lead & 0x0F) as usize, |l, i| Some(l | (*bytes.get(i)? as usize) << (4 + 8 * (i - 1))))? };
    let body = bytes.get(1 + follow..length)?;
    let (size, data) = match *body.first()? {
        0x0A => (*body.get(1)? as usize, body.get(2..)?),
        0x0B => (u16::from_le_bytes([*body.get(1)?, *body.get(2)?]) as usize, body.get(3..)?),
        0x0C => (u32::from_le_bytes(body.get(1..5)?.try_into().ok()?) as usize, body.get(5..)?),
        _ => return None,
    };
    data.get(..size.min(data.len()))
}

// The first memory range of a resource template: Memory32Fixed, DWord or QWord memory address space.
fn memory_window(mut resources: &[u8]) -> Option<(u64, u64)> {
    let u32_at = |b: &[u8], at: usize| -> Option<u64> { Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?) as u64) };
    let u64_at = |b: &[u8], at: usize| -> Option<u64> { Some(u64::from_le_bytes(b.get(at..at + 8)?.try_into().ok()?)) };
    while let Some(&tag) = resources.first() {
        if tag & 0x80 == 0 {
            if tag & 0x78 == 0x78 { return None; } // the end tag
            resources = resources.get(1 + (tag & 7) as usize..)?;
            continue;
        }
        let length = u16::from_le_bytes([*resources.get(1)?, *resources.get(2)?]) as usize;
        let item = resources.get(..3 + length)?;
        match tag {
            0x86 => return Some((u32_at(item, 4)?, u32_at(item, 8)?)),
            0x87 if item.get(3) == Some(&0) => return Some((u32_at(item, 10)?, u32_at(item, 22)?)),
            0x8A if item.get(3) == Some(&0) => return Some((u64_at(item, 14)?, u64_at(item, 38)?)),
            _ => {}
        }
        resources = &resources[3 + length..];
    }
    None
}
