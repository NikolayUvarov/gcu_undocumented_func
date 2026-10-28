// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2018-2019 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw88.txt.
// Adapted from Linux v6.12 rtw88 fw.h (struct rtw_fw_hdr), mac.c (check_firmware_size, wlan_cpu_enable,
// download_firmware_reg_backup/restore, download_firmware_reset_platform, start_download_firmware,
// download_firmware_to_mem, iddma_download_firmware, check_fw_checksum, download_firmware_end_flow,
// download_firmware_validate, __rtw_download_firmware) and fw.c (rtw_fw_write_data_rsvd_page).
//! The firmware: its header, and its download into the chip's CPU through the beacon ring and the chip's own DMA.

use crate::dma::{self, Region, TX_DESC};
use crate::io::{Error, Regs};
use crate::{coex, power, txrx};

/// One part of the image: where it goes in the CPU's memory, where it is in the file, its bytes with their checksum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Part { pub addr: u32, pub at: usize, pub len: usize }

/// The image's header as rtw88 reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Image { pub signature: u16, pub version: u16, pub sub: u8, pub dmem: Part, pub imem: Part, pub emem: Option<Part> }

fn u32_at(f: &[u8], at: usize) -> u32 { u32::from_le_bytes(f[at..at + 4].try_into().unwrap()) }

/// The header read and the parts' sizes checked against the file's (check_firmware_size).
pub fn parse(f: &[u8]) -> Result<Image, Error> {
    if f.len() < 64 { return Err(Error::Firmware("shorter than its header")); }
    let (dmem, imem, emem) = (u32_at(f, 0x24) as usize, u32_at(f, 0x30) as usize, u32_at(f, 0x34) as usize);
    let has_emem = f[0x18] & 1 << 4 != 0;
    let total = 64usize.checked_add(dmem + 8).and_then(|t| t.checked_add(imem + 8)).and_then(|t| t.checked_add(if has_emem { emem + 8 } else { 0 }));
    if total != Some(f.len()) { return Err(Error::Firmware("its parts' sizes do not add up to the file's")); }
    let dmem_part = Part { addr: u32_at(f, 0x20) & !(1 << 31), at: 64, len: dmem + 8 };
    let imem_part = Part { addr: u32_at(f, 0x3c) & !(1 << 31), at: 64 + dmem + 8, len: imem + 8 };
    let emem_part = has_emem.then(|| Part { addr: u32_at(f, 0x38) & !(1 << 31), at: imem_part.at + imem + 8, len: emem + 8 });
    Ok(Image { signature: u16::from_le_bytes([f[0], f[1]]), version: u16::from_le_bytes([f[4], f[5]]), sub: f[6], dmem: dmem_part, imem: imem_part, emem: emem_part })
}

/// The chip's memory map: a destination below this is the instruction memory.
const OCP_DMEM: u32 = 0x0020_0000;
/// Where the beacon ring's TX buffer is, as the chip's DMA engine reads it.
const OCP_TXBUF: u32 = 0x1878_0000;
const CHUNK: usize = 0x1000;

/// `bytes` into TX FIFO page 0 through the beacon ring (rtw_fw_write_data_rsvd_page).
fn reserved_page<R: Regs>(r: &mut R, region: &mut Region, bytes: &[u8]) -> Result<(), Error> {
    r.w16(0x204, 1 << 15);
    let cr1 = r.r8(0x101);
    r.w8(0x101, cr1 | 1);
    let txq = r.r8(0x422);
    r.w8(0x422, txq & !(1 << 6));
    let buf = region.bytes(dma::BCN_BUF, TX_DESC + bytes.len());
    txrx::page_desc(&mut buf[..TX_DESC], bytes.len());
    buf[TX_DESC..].copy_from_slice(bytes);
    dma::post_beacon(r, region, TX_DESC + bytes.len());
    let done = r.poll32(0x204, 1 << 15, 1 << 15, 1000, 10, "a reserved page's DMA");
    // The boundary is 0 before the MAC's set-up.
    r.w16(0x204, 1 << 15);
    r.w8(0x422, txq);
    r.w8(0x101, cr1);
    done
}

/// One part copied into the CPU's memory, a chunk at a time, by the chip's DMA engine from TX FIFO page 0, with its
/// running checksum checked.
fn part<R: Regs>(r: &mut R, region: &mut Region, file: &[u8], p: &Part) -> Result<(), Error> {
    r.set32(0x1208, 1 << 25);
    for (i, chunk) in file[p.at..p.at + p.len].chunks(CHUNK).enumerate() {
        reserved_page(r, region, chunk)?;
        r.poll32(0x1208, 1 << 31, 0, 1000, 10, "the DMA engine")?;
        r.w32(0x1200, OCP_TXBUF + TX_DESC as u32);
        r.w32(0x1204, p.addr + (i * CHUNK) as u32);
        r.w32(0x1208, 1 << 31 | 1 << 29 | chunk.len() as u32 & 0x3ffff | if i == 0 { 0 } else { 1 << 24 });
        r.poll32(0x1208, 1 << 31, 0, 1000, 10, "the DMA engine")?;
    }
    let c = r.r8(0x80);
    let imem = p.addr < OCP_DMEM;
    if r.r32(0x1208) & 1 << 27 != 0 {
        r.w8(0x80, if imem { (c | 1 << 3) & !(1 << 4) } else { (c | 1 << 5) & !(1 << 6) });
        return Err(Error::Firmware("a part's checksum does not match"));
    }
    r.w8(0x80, c | if imem { 1 << 3 | 1 << 4 } else { 1 << 5 | 1 << 6 });
    Ok(())
}

/// The image downloaded and started (__rtw_download_firmware for an 11ac CPU); the rings must be programmed and the
/// MAC on. The caller programs the rings again after it.
pub fn download<R: Regs>(r: &mut R, region: &mut Region, file: &[u8], image: &Image) -> Result<(), Error> {
    let lte = coex::lte_read(r, 0x38)?;
    r.clr8(0x03, 1 << 2);
    r.clr8(0x1d, 1);
    let b0 = r.r8(0x10d); r.w8(0x10d, 0xc0);
    let b1 = r.r8(0x100); r.w8(0x100, 0x05);
    let b2 = 0x8000_0000; r.w32(0x1330, b2);
    let b3 = r.r16(0x230); r.w16(0x230, 0x200);
    let b4 = r.r32(0x22c) | 1 << 31; r.w32(0x22c, b4);
    let b5 = r.r8(0x550); r.w8(0x550, b5 & !(1 << 3) | 1 << 4);
    r.clr8(0x1082, 1);
    r.clr8(0x09, 1 << 6);
    r.set8(0x1082, 1);
    r.set8(0x09, 1 << 6);
    let v = r.r16(0x80);
    r.w16(0x80, v & 0x3800 | 1);
    let mut copied = part(r, region, file, &image.dmem).and_then(|_| part(r, region, file, &image.imem));
    if let (Ok(()), Some(emem)) = (copied, &image.emem) { copied = part(r, region, file, emem); }
    r.w8(0x10d, b0); r.w8(0x100, b1); r.w32(0x1330, b2); r.w16(0x230, b3); r.w32(0x22c, b4); r.w8(0x550, b5);
    let started = copied.and_then(|_| {
        r.w32(0x210, 1 << 2);
        let v = r.r16(0x80);
        if v & 0x50 == 0x50 { r.w16(0x80, (v | 1 << 14) & !1); }
        r.set8(0x1d, 1);
        r.set8(0x03, 1 << 2);
        coex::lte_write(r, 0x38, lte)?;
        // Linux waits 10 ms; this waits 100 ms.
        r.poll32(0x80, 0xffff, power::FW_READY as u32, 10_000, 10, "the firmware's ready pattern")
    });
    if let Err(e) = started {
        r.clr8(0x80, 1);
        r.set8(0x03, 1 << 2);
        if r.r32(0x10fc) & 0xffff_ff00 == 0xfaaa_aa00 { return Err(Error::Firmware("the chip refused the firmware's key")); }
        return Err(e);
    }
    Ok(())
}
