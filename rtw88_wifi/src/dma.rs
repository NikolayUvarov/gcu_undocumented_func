// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2018-2019 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw88.txt.
// Adapted from Linux v6.12 rtw88 pci.c (rtw_pci_reset_buf_desc, rtw_pci_dma_reset, rtw_pci_tx_write_data,
// rtw_pci_write_data_rsvd_page, rtw_pci_rx_napi) and pci.h (the ring registers and buffer descriptors).
//! The DMA region: the rings' buffer descriptors and buffers, all below 4 GiB (rtw88's DMA is 32-bit), and how the chip
//! is told where they are, given frames, and asked for what it received.

use crate::io::{Error, Regs};
use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{fence, Ordering};

/// The region init grants (550-DRV-0042: 1 MiB, asked of KRN).
pub const BYTES: usize = 0x10_0000;
/// TX slots of each data ring, and the bytes of each: a 48-byte TX descriptor, then a frame.
pub const TX_SLOTS: usize = 16;
pub const TX_SLOT_BYTES: usize = 4096;
/// RX buffers: a 24-byte RX descriptor, 32 bytes of PHY status, the frame and its FCS.
pub const RX_SLOTS: usize = 128;
pub const RX_BUF_BYTES: usize = 4096;
/// The H2C ring's slots: a 48-byte TX descriptor and a 32-byte packet.
const H2C_SLOT_BYTES: usize = 128;
pub const TX_DESC: usize = 48;

// Where things are in the region. The beacon ring's buffer holds a TX descriptor and a 4 KiB firmware chunk.
const BCN_BD: usize = 0x000;
const MGMT_BD: usize = 0x100;
const BE_BD: usize = 0x200;
const H2C_BD: usize = 0x300;
const SPARE_BD: [usize; 4] = [0x400, 0x420, 0x440, 0x460]; // BK, VI, VO, HI0: programmed, never used
const RX_BD: usize = 0x800;
pub const BCN_BUF: usize = 0x1000;
pub const BCN_BUF_BYTES: usize = 0x2000;
const MGMT_BUF: usize = 0x3000;
const BE_BUF: usize = MGMT_BUF + TX_SLOTS * TX_SLOT_BYTES;
const H2C_BUF: usize = BE_BUF + TX_SLOTS * TX_SLOT_BYTES;
const RX_BUF: usize = H2C_BUF + 0x1000;
pub const END: usize = RX_BUF + RX_SLOTS * RX_BUF_BYTES;
const _: () = assert!(TX_SLOTS * H2C_SLOT_BYTES <= 0x1000 && END <= BYTES && RX_SLOTS * 8 <= BCN_BUF - RX_BD);

/// The region as mapped: its address here and on the bus, which must be contiguous and below 4 GiB.
pub struct Region { base: *mut u8, phys: u32 }

impl Region {
    /// # Safety
    /// `base` must point to at least END writable bytes, mapped coherently at bus address `phys` onwards, kept alive
    /// and untouched by anything else for as long as the chip may use them.
    pub unsafe fn new(base: *mut u8, phys: u64, len: usize) -> Result<Self, Error> {
        if len < END || base.is_null() { return Err(Error::Dma("the region is too small")); }
        if phys == 0 || phys % 4096 != 0 || phys + END as u64 > 1 << 32 { return Err(Error::Dma("the region is not below 4 GiB")); }
        core::ptr::write_bytes(base, 0, END);
        Ok(Self { base, phys: phys as u32 })
    }
    pub fn phys(&self, at: usize) -> u32 { self.phys + at as u32 }
    pub fn bytes(&mut self, at: usize, n: usize) -> &mut [u8] {
        assert!(at + n <= END);
        unsafe { core::slice::from_raw_parts_mut(self.base.add(at), n) }
    }
    fn w16(&mut self, at: usize, v: u16) { assert!(at + 2 <= END); unsafe { write_volatile(self.base.add(at) as *mut u16, v.to_le()) } }
    fn w32(&mut self, at: usize, v: u32) { assert!(at + 4 <= END); unsafe { write_volatile(self.base.add(at) as *mut u32, v.to_le()) } }
    fn r16(&self, at: usize) -> u16 { assert!(at + 2 <= END); u16::from_le(unsafe { read_volatile(self.base.add(at) as *const u16) }) }
}

/// A TX ring: its buffer descriptors, buffers and registers, and the host's write and read indices.
#[derive(Clone, Copy)]
pub struct Tx { bd: usize, buf: usize, slot: usize, len: u16, desa: u32, num: u32, idx: u32, wp: u16, rp: u16 }

pub const MGMT: Tx = Tx { bd: MGMT_BD, buf: MGMT_BUF, slot: TX_SLOT_BYTES, len: TX_SLOTS as u16, desa: 0x310, num: 0x380, idx: 0x3b0, wp: 0, rp: 0 };
pub const BE: Tx = Tx { bd: BE_BD, buf: BE_BUF, slot: TX_SLOT_BYTES, len: TX_SLOTS as u16, desa: 0x328, num: 0x388, idx: 0x3a8, wp: 0, rp: 0 };
pub const H2C: Tx = Tx { bd: H2C_BD, buf: H2C_BUF, slot: H2C_SLOT_BYTES, len: TX_SLOTS as u16, desa: 0x1320, num: 0x1328, idx: 0x132c, wp: 0, rp: 0 };
// BK, VI, VO, HI0: two entries each, as Linux programs every ring.
const SPARE: [(u32, u32); 4] = [(0x330, 0x38a), (0x320, 0x386), (0x318, 0x384), (0x340, 0x38c)];

impl Tx {
    fn program<R: Regs>(&mut self, r: &mut R, region: &Region) {
        r.w16(self.num, self.len);
        r.w32(self.desa, region.phys(self.bd));
        (self.wp, self.rp) = (0, 0);
    }

    /// The bytes of the next slot: a TX descriptor's then a frame's.
    pub fn slot<'a>(&self, region: &'a mut Region) -> &'a mut [u8] { region.bytes(self.buf + self.wp as usize * self.slot, self.slot) }
    pub fn slot_bytes(&self) -> usize { self.slot }

    /// Whether a slot is free, by the chip's read index.
    pub fn room<R: Regs>(&mut self, r: &mut R) -> bool {
        self.rp = (r.r32(self.idx) >> 16 & 0xfff) as u16 % self.len;
        (self.wp + 1) % self.len != self.rp
    }

    /// The slot filled with a descriptor and `frame` bytes after it handed to the chip (rtw_pci_tx_write_data).
    pub fn post<R: Regs>(&mut self, r: &mut R, region: &mut Region, frame: usize) {
        let at = self.bd + self.wp as usize * 16;
        let dma = region.phys(self.buf + self.wp as usize * self.slot);
        for i in 0..4 { region.w32(at + 4 * i, 0); }
        region.w16(at, TX_DESC as u16);
        region.w16(at + 2, (TX_DESC + frame).div_ceil(128) as u16);
        region.w32(at + 4, dma);
        region.w16(at + 8, frame as u16);
        region.w32(at + 12, dma + TX_DESC as u32);
        fence(Ordering::SeqCst);
        self.wp = (self.wp + 1) % self.len;
        r.w16(self.idx, self.wp);
    }
}

/// The rings programmed and every index cleared (rtw_pci_reset_buf_desc then rtw_pci_dma_reset); the RX buffers armed.
pub fn program<R: Regs>(r: &mut R, region: &mut Region, rings: &mut [&mut Tx; 3]) {
    let v = r.r8(0x303);
    r.w8(0x303, v | 0xf7);
    r.w32(0x308, region.phys(BCN_BD));
    for ring in rings.iter_mut() { ring.program(r, region); }
    for (i, (desa, num)) in SPARE.iter().enumerate() {
        r.w16(*num, 2);
        r.w32(*desa, region.phys(SPARE_BD[i]));
    }
    for i in 0..RX_SLOTS { arm(region, i); }
    r.w16(0x382, RX_SLOTS as u16);
    r.w32(0x338, region.phys(RX_BD));
    fence(Ordering::SeqCst);
    r.w32(0x39c, 0xffff_ffff);
    r.set32(0x1330, 1 << 16 | 1 << 8);
    r.set32(0x300, 1 << 20 | 1 << 15);
}

/// RX buffer descriptor `i` given its buffer again (rtw_pci_reset_rx_desc).
fn arm(region: &mut Region, i: usize) {
    let at = RX_BD + i * 8;
    region.w32(at, 0);
    region.w16(at, RX_BUF_BYTES as u16);
    region.w32(at + 4, region.phys(RX_BUF + i * RX_BUF_BYTES));
}

/// The beacon ring's one descriptor given `bytes` (a TX descriptor and a firmware chunk, already in BCN_BUF) with its
/// owner bit, and the ring kicked (rtw_pci_write_data_rsvd_page).
pub fn post_beacon<R: Regs>(r: &mut R, region: &mut Region, bytes: usize) {
    for i in 0..4 { region.w32(BCN_BD + 4 * i, 0); }
    region.w16(BCN_BD, TX_DESC as u16);
    region.w16(BCN_BD + 2, bytes.div_ceil(128) as u16 | 1 << 15);
    region.w32(BCN_BD + 4, region.phys(BCN_BUF));
    fence(Ordering::SeqCst);
    let v = r.r8(0x383);
    r.w8(0x383, v | 1 << 4);
}

/// The host's place in the RX ring.
pub struct Rx { pub next: usize }

impl Rx {
    /// How many buffers the chip has filled since `next` (by its write index).
    pub fn ready<R: Regs>(&self, r: &mut R) -> usize {
        let hw = (r.r32(0x3b4) >> 16 & 0xfff) as usize % RX_SLOTS;
        (hw + RX_SLOTS - self.next) % RX_SLOTS
    }
    /// The next filled buffer.
    pub fn buffer<'a>(&self, region: &'a mut Region) -> &'a mut [u8] { region.bytes(RX_BUF + self.next * RX_BUF_BYTES, RX_BUF_BYTES) }
    /// The buffer given back to the chip, which is told so.
    pub fn done<R: Regs>(&mut self, r: &mut R, region: &mut Region) {
        arm(region, self.next);
        fence(Ordering::SeqCst);
        self.next = (self.next + 1) % RX_SLOTS;
        r.w16(0x3b4, self.next as u16);
    }
    /// What the chip wrote into the buffer's descriptor: its running tag, for a check.
    pub fn tag(&self, region: &Region) -> u16 { region.r16(RX_BD + self.next * 8 + 2) }
}
