// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2020 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
// Format definitions adapted from Linux v6.19 rtw89/fw.h, fw.c and core.h.

//! Structural validation only: this module does not authenticate or download firmware.

pub const MAX_FILE_SIZE: usize = 4 * 1024 * 1024;
pub const MAX_IMAGES: usize = 32;
pub const MAX_SECTIONS: usize = 10;
pub const NORMAL_CE: u8 = 5;
pub const DOWNLOAD_PART_SIZE: usize = 2020;
const MFW_HEADER: usize = 16;
const MFW_ENTRY: usize = 16;
const FW_HEADER: usize = 32;
const SECTION_HEADER: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    TooLarge,
    Truncated,
    ContainerSignature,
    ImageCount,
    ImageRange,
    OverlappingImages,
    AmbiguousImage,
    NoCompatibleImage,
    HeaderVersion,
    SectionCount,
    DynamicHeader,
    UnsupportedSecurity,
    SectionRange,
    AddressRange,
    TrailingData,
}

#[derive(Clone, Copy, Debug)]
pub struct SelectedImage<'a> {
    pub bytes: &'a [u8],
    pub cut: u8,
    pub kind: u8,
    pub container_offset: usize,
}

/// Structural directory metadata only; arbitrary input cannot establish pinned-file provenance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContainerLayout { count: u8, last_end: usize, maximum_end: usize, elements_at: usize }
impl ContainerLayout {
    pub fn image_count(&self) -> u8 { self.count }
    pub fn last_image_end(&self) -> usize { self.last_end }
    pub fn maximum_image_end(&self) -> usize { self.maximum_end }
    pub fn elements_offset(&self) -> usize { self.elements_at }
}

#[derive(Clone, Copy, Debug)]
pub struct Section<'a> {
    pub kind: u8,
    pub download_address: u32,
    pub bytes: &'a [u8],
    pub checksum_present: bool,
    pub redownload: bool,
}

const EMPTY_SECTION: Section<'static> = Section {
    kind: 0,
    download_address: 0,
    bytes: &[],
    checksum_present: false,
    redownload: false,
};

#[derive(Debug)]
pub struct Image<'a> {
    pub version: [u8; 4],
    pub commit_id: u32,
    pub header_len: usize,
    pub idmem_share_mode: u8,
    pub dynamic_header: &'a [u8],
    sections: [Section<'a>; MAX_SECTIONS],
    section_count: usize,
}

impl<'a> Image<'a> {
    pub fn sections(&self) -> &[Section<'a>] {
        &self.sections[..self.section_count]
    }
}

fn range(bytes: &[u8], start: usize, len: usize) -> Result<&[u8], Error> {
    let end = start.checked_add(len).ok_or(Error::Truncated)?;
    bytes.get(start..end).ok_or(Error::Truncated)
}

fn word(bytes: &[u8], at: usize) -> Result<u32, Error> {
    let b = range(bytes, at, 4)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn entry_range(bytes: &[u8], at: usize, table_end: usize) -> Result<(usize, usize), Error> {
    let start = word(bytes, at + 4)? as usize;
    let len = word(bytes, at + 8)? as usize;
    let end = start.checked_add(len).ok_or(Error::ImageRange)?;
    if len == 0 || start < table_end || end > bytes.len() {
        return Err(Error::ImageRange);
    }
    Ok((start, end))
}

/// Validate every image range; upstream locates elements after the aligned final directory entry, not the maximum.
pub fn container_layout(bytes: &[u8]) -> Result<ContainerLayout, Error> {
    if bytes.len() > MAX_FILE_SIZE {
        return Err(Error::TooLarge);
    }
    range(bytes, 0, MFW_HEADER)?;
    if bytes[0] != 0xff {
        return Err(Error::ContainerSignature);
    }
    let count = bytes[1] as usize;
    if count == 0 || count > MAX_IMAGES {
        return Err(Error::ImageCount);
    }
    let table_end = MFW_HEADER + count * MFW_ENTRY;
    range(bytes, 0, table_end)?;
    let mut last_end = 0;
    let mut maximum_end = 0;
    for i in 0..count {
        let at = MFW_HEADER + i * MFW_ENTRY;
        let (start, end) = entry_range(bytes, at, table_end)?;
        last_end = end;
        maximum_end = maximum_end.max(end);
        for j in 0..i {
            let prev = MFW_HEADER + j * MFW_ENTRY;
            let (prev_start, prev_end) = entry_range(bytes, prev, table_end)?;
            if start < prev_end && prev_start < end {
                return Err(Error::OverlappingImages);
            }
            if bytes[at..at + 3] == bytes[prev..prev + 3] {
                return Err(Error::AmbiguousImage);
            }
        }
    }
    let elements_at = last_end.checked_add(15).ok_or(Error::ImageRange)? & !15;
    Ok(ContainerLayout { count: count as u8, last_end, maximum_end, elements_at })
}

/// Select the newest non-manufacturing image compatible with the measured chip cut.
pub fn select(bytes: &[u8], chip_cut: u8, kind: u8) -> Result<SelectedImage<'_>, Error> {
    let layout = container_layout(bytes)?;
    let count = layout.count as usize;
    let table_end = MFW_HEADER + count * MFW_ENTRY;
    let mut chosen: Option<SelectedImage<'_>> = None;
    for i in 0..count {
        let at = MFW_HEADER + i * MFW_ENTRY;
        let (start, end) = entry_range(bytes, at, table_end)?;
        let cut = bytes[at];
        if bytes[at + 1] != kind || bytes[at + 2] != 0 || cut > chip_cut {
            continue;
        }
        if chosen.as_ref().map_or(true, |old| cut > old.cut) {
            chosen = Some(SelectedImage {
                bytes: &bytes[start..end],
                cut,
                kind,
                container_offset: start,
            });
        }
    }
    chosen.ok_or(Error::NoCompatibleImage)
}

/// Parse AX header version 0; secure-download policy and dynamic contents remain opaque.
pub fn parse(bytes: &[u8]) -> Result<Image<'_>, Error> {
    if bytes.len() > MAX_FILE_SIZE {
        return Err(Error::TooLarge);
    }
    range(bytes, 0, FW_HEADER)?;
    let w3 = word(bytes, 12)?;
    if w3 >> 24 != 0 {
        return Err(Error::HeaderVersion);
    }
    let count = ((word(bytes, 24)? >> 8) & 0xff) as usize;
    if count == 0 || count > MAX_SECTIONS {
        return Err(Error::SectionCount);
    }
    let base_header = FW_HEADER + count * SECTION_HEADER;
    range(bytes, 0, base_header)?;
    let w7 = word(bytes, 28)?;
    let header_len = if w7 & (1 << 16) != 0 {
        let len = ((w3 >> 16) & 0xff) as usize;
        let dynamic_len = len.checked_sub(base_header).ok_or(Error::DynamicHeader)?;
        if dynamic_len < 8 {
            return Err(Error::DynamicHeader);
        }
        range(bytes, 0, len)?;
        if word(bytes, base_header)? as usize != dynamic_len {
            return Err(Error::DynamicHeader);
        }
        len
    } else {
        base_header
    };
    let mut image = Image {
        version: word(bytes, 4)?.to_le_bytes(),
        commit_id: word(bytes, 8)?,
        header_len,
        idmem_share_mode: ((w7 >> 18) & 0xf) as u8,
        dynamic_header: &bytes[base_header..header_len],
        sections: [EMPTY_SECTION; MAX_SECTIONS],
        section_count: count,
    };
    let mut offset = header_len;
    for i in 0..count {
        let at = FW_HEADER + i * SECTION_HEADER;
        let metadata = word(bytes, at + 4)?;
        let kind = ((metadata >> 24) & 0xf) as u8;
        let checksum_present = metadata & (1 << 28) != 0;
        let payload_len = (metadata & 0x00ff_ffff) as usize;
        let len = payload_len + if checksum_present { 8 } else { 0 };
        if kind == 9 && word(bytes, at + 8)? != 0 {
            return Err(Error::UnsupportedSecurity);
        }
        let download_address = word(bytes, at)? & 0x1fff_ffff;
        if download_address as usize + len > 0x2000_0000 {
            return Err(Error::AddressRange);
        }
        let data = range(bytes, offset, len).map_err(|_| Error::SectionRange)?;
        image.sections[i] = Section {
            kind,
            download_address,
            bytes: data,
            checksum_present,
            redownload: metadata & (1 << 29) != 0,
        };
        offset += len;
    }
    if offset != bytes.len() {
        return Err(Error::TrailingData);
    }
    Ok(image)
}
