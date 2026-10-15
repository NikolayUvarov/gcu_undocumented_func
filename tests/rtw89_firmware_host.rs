#[path = "../rtw89_wifi/src/firmware.rs"]
mod firmware;
#[path = "../rtw89_wifi/src/registers.rs"]
mod registers;

use firmware::{parse, select, Error, NORMAL_CE};

fn put(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn image() -> Vec<u8> {
    let mut bytes = vec![0; 56];
    put(&mut bytes, 4, 0x0f1d1d00);
    put(&mut bytes, 8, 0x12345678);
    put(&mut bytes, 24, 1 << 8);
    put(&mut bytes, 32, 0xb8970000);
    put(&mut bytes, 36, 0x22000008);
    bytes[48..].copy_from_slice(b"testcode");
    bytes
}

fn container(entries: &[(u8, u8, u8)]) -> Vec<u8> {
    let inner = image();
    let table_end = 16 + entries.len() * 16;
    let mut bytes = vec![0; table_end];
    bytes[0] = 0xff;
    bytes[1] = entries.len() as u8;
    for (i, &(cut, kind, mp)) in entries.iter().enumerate() {
        let at = 16 + i * 16;
        bytes[at..at + 3].copy_from_slice(&[cut, kind, mp]);
        put(&mut bytes, at + 4, (table_end + i * inner.len()) as u32);
        put(&mut bytes, at + 8, inner.len() as u32);
        bytes.extend_from_slice(&inner);
    }
    bytes
}

#[test]
fn selects_compatible_cut_and_ignores_manufacturing_and_wowlan() {
    let bytes = container(&[
        (2, NORMAL_CE, 0),
        (1, NORMAL_CE, 0),
        (3, NORMAL_CE, 1),
        (3, 3, 0),
    ]);
    assert_eq!(
        select(&bytes, 0, NORMAL_CE).unwrap_err(),
        Error::NoCompatibleImage
    );
    assert_eq!(select(&bytes, 1, NORMAL_CE).unwrap().cut, 1);
    let selected = select(&bytes, 3, NORMAL_CE).unwrap();
    assert_eq!(selected.cut, 2);
    assert_eq!(selected.kind, NORMAL_CE);
    assert_eq!(selected.container_offset, 80);
    assert_eq!(select(&bytes, 3, 1).unwrap_err(), Error::NoCompatibleImage);
}

#[test]
fn validates_all_entries_even_when_selected_image_is_earlier() {
    let mut bytes = container(&[(1, NORMAL_CE, 0), (2, 3, 0)]);
    put(&mut bytes, 36, u32::MAX);
    assert_eq!(select(&bytes, 1, NORMAL_CE).unwrap_err(), Error::ImageRange);
}

#[test]
fn rejects_overlap_table_aliases_and_duplicate_selection_keys() {
    let mut bytes = container(&[(1, NORMAL_CE, 0), (2, NORMAL_CE, 0)]);
    put(&mut bytes, 36, 48);
    assert_eq!(
        select(&bytes, 2, NORMAL_CE).unwrap_err(),
        Error::OverlappingImages
    );
    put(&mut bytes, 20, 16);
    assert_eq!(select(&bytes, 2, NORMAL_CE).unwrap_err(), Error::ImageRange);
    let bytes = container(&[(1, NORMAL_CE, 0), (1, NORMAL_CE, 0)]);
    assert_eq!(
        select(&bytes, 1, NORMAL_CE).unwrap_err(),
        Error::AmbiguousImage
    );
}

#[test]
fn limits_container_size_count_and_signature() {
    let mut bytes = container(&[(1, NORMAL_CE, 0)]);
    bytes[0] = 0;
    assert_eq!(
        select(&bytes, 1, NORMAL_CE).unwrap_err(),
        Error::ContainerSignature
    );
    bytes[0] = 0xff;
    for n in [0, 33, 255] {
        bytes[1] = n;
        assert_eq!(select(&bytes, 1, NORMAL_CE).unwrap_err(), Error::ImageCount);
    }
    bytes.resize(firmware::MAX_FILE_SIZE + 1, 0);
    assert_eq!(select(&bytes, 1, NORMAL_CE).unwrap_err(), Error::TooLarge);
    assert_eq!(parse(&bytes).unwrap_err(), Error::TooLarge);
}

#[test]
fn rejects_every_truncated_prefix_without_panicking() {
    let outer = container(&[(1, NORMAL_CE, 0)]);
    for end in 0..outer.len() {
        assert!(
            select(&outer[..end], 1, NORMAL_CE).is_err(),
            "container prefix {end}"
        );
    }
    let inner = image();
    for end in 0..inner.len() {
        assert!(parse(&inner[..end]).is_err(), "image prefix {end}");
    }
}

#[test]
fn preserves_section_metadata_and_ax_address_mapping() {
    let bytes = image();
    let parsed = parse(&bytes).unwrap();
    assert_eq!(parsed.version, [0, 29, 29, 15]);
    assert_eq!(parsed.commit_id, 0x12345678);
    assert_eq!(parsed.header_len, 48);
    assert_eq!(parsed.idmem_share_mode, 0);
    assert_eq!(parsed.sections()[0].kind, 2);
    assert_eq!(parsed.sections()[0].download_address, 0x18970000);
    assert!(parsed.sections()[0].redownload);
    assert!(!parsed.sections()[0].checksum_present);
    assert_eq!(parsed.sections()[0].bytes, b"testcode");
    assert_eq!(firmware::DOWNLOAD_PART_SIZE, 2020);
}

#[test]
fn checksum_trailer_is_part_of_section_bounds() {
    let mut bytes = image();
    put(&mut bytes, 36, 0x12000008);
    assert_eq!(parse(&bytes).unwrap_err(), Error::SectionRange);
    bytes.extend_from_slice(&[0x55; 8]);
    let parsed = parse(&bytes).unwrap();
    assert!(parsed.sections()[0].checksum_present);
    assert_eq!(parsed.sections()[0].bytes.len(), 16);
}

#[test]
fn checks_dynamic_header_lengths_without_interpreting_extensions() {
    let mut bytes = image();
    bytes.splice(48..48, [8, 0, 0, 0, 0, 0, 0, 0]);
    put(&mut bytes, 12, 56 << 16);
    put(&mut bytes, 28, 1 << 16);
    assert_eq!(parse(&bytes).unwrap().dynamic_header.len(), 8);
    put(&mut bytes, 12, 47 << 16);
    assert_eq!(parse(&bytes).unwrap_err(), Error::DynamicHeader);
    put(&mut bytes, 12, 52 << 16);
    assert_eq!(parse(&bytes).unwrap_err(), Error::DynamicHeader);
    put(&mut bytes, 12, 56 << 16);
    put(&mut bytes, 48, u32::MAX);
    assert_eq!(parse(&bytes).unwrap_err(), Error::DynamicHeader);
}

#[test]
fn rejects_new_headers_extra_bytes_and_large_sections() {
    let mut bytes = image();
    put(&mut bytes, 12, 1 << 24);
    assert_eq!(parse(&bytes).unwrap_err(), Error::HeaderVersion);
    put(&mut bytes, 12, 0);
    put(&mut bytes, 24, 11 << 8);
    assert_eq!(parse(&bytes).unwrap_err(), Error::SectionCount);
    put(&mut bytes, 24, 1 << 8);
    bytes.push(0);
    assert_eq!(parse(&bytes).unwrap_err(), Error::TrailingData);
    put(&mut bytes, 36, 0x02ffffff);
    assert_eq!(parse(&bytes).unwrap_err(), Error::SectionRange);
    put(&mut bytes, 32, 0xffffffff);
    assert_eq!(parse(&bytes).unwrap_err(), Error::AddressRange);
}

#[test]
fn does_not_treat_unimplemented_security_signature_pools_as_payload() {
    let mut bytes = image();
    put(&mut bytes, 36, 0x09000008);
    assert!(parse(&bytes).is_ok());
    for mssc in [1, 255, u32::MAX] {
        put(&mut bytes, 40, mssc);
        assert_eq!(parse(&bytes).unwrap_err(), Error::UnsupportedSecurity);
    }
}

#[test]
fn single_byte_mutations_stay_bounded() {
    let original = container(&[(1, NORMAL_CE, 0), (2, NORMAL_CE, 0)]);
    for at in 0..original.len() {
        for value in [0, 1, 0x7f, 0xff] {
            let mut bytes = original.clone();
            bytes[at] = value;
            if let Ok(selected) = select(&bytes, 2, NORMAL_CE) {
                let _ = parse(selected.bytes);
            }
        }
    }
}

#[test]
fn chip_cut_uses_only_sys_cfg1_bits_and_rejects_absent_device() {
    assert_eq!(registers::chip_cut(0x12342001), Some(2));
    assert_eq!(registers::chip_cut(0), Some(0));
    assert_eq!(registers::chip_cut(u32::MAX), None);
    assert_eq!(
        (registers::VENDOR_ID, registers::DEVICE_ID),
        (0x10ec, 0xb852)
    );
    assert_eq!((registers::MMIO_BAR, registers::SYS_CFG1), (2, 0xf0));
}

#[test]
#[ignore = "requires separately fetched proprietary firmware; not a CI fixture"]
fn parses_pinned_realtek_firmware() {
    let path =
        std::env::var("RTW89_FIRMWARE").expect("set RTW89_FIRMWARE to the verified fw-1 file");
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(bytes.len(), 1_245_944);
    assert_eq!(
        select(&bytes, 0, NORMAL_CE).unwrap_err(),
        Error::NoCompatibleImage
    );
    for (cut, size) in [(1, 326616), (2, 326536)] {
        let selected = select(&bytes, cut, NORMAL_CE).unwrap();
        assert_eq!(selected.bytes.len(), size);
        let parsed = parse(selected.bytes).unwrap();
        assert_eq!(parsed.version, [0, 29, 29, 15]);
        assert_eq!(parsed.commit_id, 0x6fb3ec41);
        assert_eq!(parsed.header_len, 144);
        assert_eq!(parsed.sections().len(), 3);
    }
}
