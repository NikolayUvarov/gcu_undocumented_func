// What a Boot#### entry leads to (351-KRN-0195): its device path read node by node. The firmware's own network boot (PXE,
// HTTP boot) runs before our loader and outside every guarantee, so our hand never puts such an entry first or next.
// Plain, so the host tests build it alone.

/// What a boot entry's device path names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PathKind { Local, Network, Malformed }

// Messaging nodes (type 3) of a network: MAC address, IPv4, IPv6 and URI (UEFI 2.10, section 10.3.4).
const MESSAGING: u8 = 3;
const NETWORK: [u8; 4] = [0x0B, 0x0C, 0x0D, 0x18];
// The end node: of the whole path (0xFF), or of one instance with another after it (0x01).
const END: u8 = 0x7F;

/// A device path's kind. Each node is a type, a subtype and a length that counts its own 4 bytes, up to the end node; any
/// network node in any instance makes it Network, and a node that overruns the path or no end node makes it Malformed.
pub fn path_kind(path: &[u8]) -> PathKind {
    let (mut at, mut network) = (0, false);
    while let Some(node) = path.get(at..at + 4) {
        let length = u16::from_le_bytes([node[2], node[3]]) as usize;
        if length < 4 || at + length > path.len() { return PathKind::Malformed; }
        match (node[0], node[1]) {
            (END, 0xFF) => return if network { PathKind::Network } else { PathKind::Local },
            (MESSAGING, subtype) if NETWORK.contains(&subtype) => network = true,
            _ => {}
        }
        at += length;
    }
    PathKind::Malformed
}

/// A Boot#### variable's device path: after its attributes (u32), the path's length (u16) and its description (UTF-16
/// up to a NUL unit). None when the variable is cut short.
pub fn option_path(option: &[u8]) -> Option<&[u8]> {
    let length = u16::from_le_bytes([*option.get(4)?, *option.get(5)?]) as usize;
    let mut at = 6;
    while option.get(at..at + 2)? != [0, 0] { at += 2; }
    option.get(at + 2..at + 2 + length)
}

/// What a Boot#### variable leads to; one cut short is Malformed.
pub fn option_kind(option: &[u8]) -> PathKind { option_path(option).map_or(PathKind::Malformed, path_kind) }
