//! What a Boot#### entry leads to (libmind/src/firmware/boot_path.rs, 351-KRN-0195): efivar marks and refuses the
//! firmware's network boot, whose device path holds a MAC, IPv4, IPv6 or URI messaging node.
#[path = "../libmind/src/firmware/boot_path.rs"]
mod boot_path;
use boot_path::{option_kind, option_path, path_kind, PathKind};

// A device path node: type, subtype, its length with these 4 bytes, the body.
fn node(kind: u8, subtype: u8, body: &[u8]) -> Vec<u8> {
    let mut n = vec![kind, subtype];
    n.extend_from_slice(&((body.len() + 4) as u16).to_le_bytes());
    n.extend_from_slice(body);
    n
}

fn path(nodes: &[Vec<u8>]) -> Vec<u8> { nodes.concat() }

fn root() -> Vec<u8> { node(2, 1, &[0xD0, 0x41, 0x03, 0x0A, 0, 0, 0, 0]) } // PciRoot(0x0)
fn pci() -> Vec<u8> { node(1, 1, &[0, 3]) } // Pci(0x3,0x0)
fn disk() -> Vec<u8> { node(4, 1, &[0; 38]) } // HD(...), a partition
fn file() -> Vec<u8> { node(4, 4, &"\\EFI\\BOOT\\BOOTX64.EFI\0".encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<_>>()) }
fn mac() -> Vec<u8> { node(3, 0x0B, &[0x52, 0x54, 0, 0x12, 0x34, 0x56, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]) }
fn ipv4() -> Vec<u8> { node(3, 0x0C, &[0; 23]) }
fn ipv6() -> Vec<u8> { node(3, 0x0D, &[0; 56]) }
fn uri() -> Vec<u8> { node(3, 0x18, b"http://192.0.2.1/boot.efi") }
fn end() -> Vec<u8> { node(0x7F, 0xFF, &[]) }
fn end_instance() -> Vec<u8> { node(0x7F, 0x01, &[]) }

// A Boot#### variable: active, the path's length, its description in UTF-16 with a NUL, the path, optional data.
fn option(description: &str, path: &[u8], optional: &[u8]) -> Vec<u8> {
    let mut o = 1u32.to_le_bytes().to_vec();
    o.extend_from_slice(&(path.len() as u16).to_le_bytes());
    o.extend(description.encode_utf16().chain([0]).flat_map(u16::to_le_bytes));
    o.extend_from_slice(path);
    o.extend_from_slice(optional);
    o
}

#[test]
fn a_disk_path_is_local() {
    assert_eq!(path_kind(&path(&[root(), pci(), disk(), file(), end()])), PathKind::Local);
}

#[test]
fn a_mac_and_ipv4_path_is_network() {
    assert_eq!(path_kind(&path(&[root(), pci(), mac(), ipv4(), end()])), PathKind::Network);
    assert_eq!(path_kind(&path(&[root(), pci(), mac(), end()])), PathKind::Network);
}

#[test]
fn an_ipv6_path_is_network() {
    assert_eq!(path_kind(&path(&[root(), pci(), mac(), ipv6(), end()])), PathKind::Network);
}

#[test]
fn a_uri_path_is_network() {
    assert_eq!(path_kind(&path(&[root(), pci(), mac(), ipv4(), uri(), end()])), PathKind::Network);
    assert_eq!(path_kind(&path(&[uri(), end()])), PathKind::Network);
}

#[test]
fn a_network_instance_after_a_disk_one_is_network() {
    assert_eq!(path_kind(&path(&[root(), pci(), disk(), end_instance(), root(), pci(), mac(), end()])), PathKind::Network);
    assert_eq!(path_kind(&path(&[root(), pci(), disk(), end_instance(), root(), pci(), disk(), end()])), PathKind::Local);
}

#[test]
fn a_malformed_path_is_not_read_as_local() {
    let good = path(&[root(), pci(), disk(), end()]);
    assert_eq!(path_kind(&[]), PathKind::Malformed);
    assert_eq!(path_kind(&good[..good.len() - 4]), PathKind::Malformed); // no end node
    assert_eq!(path_kind(&good[..good.len() - 2]), PathKind::Malformed); // the end node cut
    let mut short = good.clone();
    short[2] = 2; // a node shorter than its own header
    assert_eq!(path_kind(&short), PathKind::Malformed);
    let mut long = good.clone();
    long[2] = 0xFF; // a node past the path's end
    assert_eq!(path_kind(&long), PathKind::Malformed);
    let mut zero = good;
    zero[2] = 0; zero[3] = 0; // a length of 0 would never move on
    assert_eq!(path_kind(&zero), PathKind::Malformed);
}

#[test]
fn a_boot_option_is_read_past_its_description_to_its_path() {
    let network = path(&[root(), pci(), mac(), ipv4(), end()]);
    let pxe = option("UEFI PXEv4 (MAC:525400123456)", &network, &[0xAA; 16]);
    assert_eq!(option_path(&pxe), Some(&network[..]));
    assert_eq!(option_kind(&pxe), PathKind::Network);
    let local = path(&[root(), pci(), disk(), file(), end()]);
    assert_eq!(option_kind(&option("MIND Core", &local, &[])), PathKind::Local);
}

#[test]
fn a_boot_option_cut_short_is_malformed() {
    let local = path(&[root(), pci(), disk(), end()]);
    let full = option("MIND Core", &local, &[]);
    assert_eq!(option_kind(&full[..full.len() - 1]), PathKind::Malformed); // the path cut
    assert_eq!(option_kind(&full[..10]), PathKind::Malformed); // inside the description, no NUL
    assert_eq!(option_kind(&full[..5]), PathKind::Malformed); // no path length
    assert_eq!(option_kind(&[]), PathKind::Malformed);
}
