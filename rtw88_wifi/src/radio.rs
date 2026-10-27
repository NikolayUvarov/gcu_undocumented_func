// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2018-2019 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw88.txt.
// The bring-up order is rtw88's (main.c rtw_power_on, rtw_chip_efuse_info_setup; pci.c rtw_pci_phy_cfg) for the
// 8821C on PCIe; the rest is this driver's own: a radio of raw frames for idl/wlanhw.wit.
//! The radio: the chip brought up in rtw88's order, received frames polled from the RX ring and queued, a frame sent
//! at a fixed rate and its report awaited.

use crate::cond::Chip;
use crate::dma::{self, Region, Rx, Tx, RX_BUF_BYTES, TX_DESC};
use crate::efuse::{self, Board};
use crate::h2c::{self, H2c};
use crate::io::{Error, Regs};
use crate::phy::CckFilter;
use crate::txrx::{self, Report};
use crate::{coex, fw, mac, phy, power};

pub const FRAME_MAX: usize = 2346;
pub const RADIO_HEADER: usize = 8;
pub const QUEUE_SLOT: usize = RADIO_HEADER + FRAME_MAX;
/// RX buffers of 4 KiB: REG_RX_PKT_LIMIT of 7 × 512 bytes leaves room for the descriptor, PHY status and FCS.
const RX_LIMIT: u8 = 7;
/// How long a frame's report is awaited (rtw88's 500 ms).
const REPORT_US: u32 = 500_000;

/// Why a request failed, as idl/wlanhw.wit says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fail { NoDevice, Busy, Empty, Invalid, Failed }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sent { pub acked: bool, pub attempts: u8 }

/// What happened to received buffers and sent frames, for the log.
#[derive(Clone, Copy, Default, Debug)]
pub struct Counts { pub frames: u32, pub kept: u32, pub crc: u32, pub c2h: u32, pub reports: u32, pub other_c2h: u32, pub dropped: u32, pub sent: u32, pub acked: u32, pub no_report: u32 }

/// The bring-up's stages, for the log of where it stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage { Chip, Rings, Master, Power, Firmware, Efuse, Board, Mac, Phy, Commands, Coex, Channel, Iqk }

pub struct Radio<'q, R: Regs> {
    pub r: R,
    region: Region,
    mgmt: Tx, be: Tx, h2c_ring: Tx, rx: Rx, h2c: H2c,
    pub board: Board, pub chip: Chip, pub cut: u8, cck: CckFilter,
    pub mac: [u8; 6], channel: u8,
    all_beacons: bool, bssid: [u8; 6], own: [u8; 6],
    queue: &'q mut [u8], head: usize, count: usize,
    cookie: u8, report: Option<Report>,
    pub counts: Counts, pub iqk: Option<u32>,
}

/// Brings the chip up as rtw_power_on does after probe. `master` turns PCI bus mastering on, once the rings are
/// programmed; `file` is the firmware, `queue` room for received frames (QUEUE_SLOT bytes each).
pub fn up<'q, R: Regs>(mut r: R, mut region: Region, file: &[u8], queue: &'q mut [u8], master: impl FnOnce() -> bool,
                       say: &mut dyn FnMut(core::fmt::Arguments)) -> Result<Radio<'q, R>, (Stage, Error)> {
    let cfg = r.r32(0xf0);
    let cut = (cfg >> 12 & 0xf) as u8;
    say(format_args!("SYS_CFG1 {:08X}: CUT {}, {}, {} PATH", cfg, cut, if cfg & 1 << 23 == 0 { "MP" } else { "TEST CHIP" }, if cfg & 1 << 27 != 0 { 2 } else { 1 }));
    if cfg & 1 << 27 != 0 { return Err((Stage::Chip, Error::Chip("two RF paths: not an RTL8821C"))); }
    let image = fw::parse(file).map_err(|e| (Stage::Firmware, e))?;
    // PCIe Gen1 PHY parameter 0x09 = 0x6380 through MDIO (rtw8821c's pcie_gen1_param).
    r.w16(0x3f4, 0x6380);
    r.w8(0x3f8, 0x09);
    r.w8(0x3fb, 0);
    r.mask32(0x3f8, 1 << 5, 1);
    r.poll32(0x3f8, 1 << 5, 0, 20, 10, "a PCIe PHY MDIO write").map_err(|e| (Stage::Chip, e))?;
    let (mut mgmt, mut be, mut h2c_ring) = (dma::MGMT, dma::BE, dma::H2C);
    dma::program(&mut r, &mut region, &mut [&mut mgmt, &mut be, &mut h2c_ring]);
    if !master() { return Err((Stage::Master, Error::Dma("bus mastering was not turned on"))); }
    let cycled = power::on(&mut r).map_err(|e| (Stage::Power, e))?;
    say(format_args!("MAC ON{}; FIRMWARE {:04X} {}.{}, DMEM {} B AT {:08X}, IMEM {} B AT {:08X}{}", if cycled { " (IT WAS ON: CYCLED)" } else { "" },
                     image.signature, image.version, image.sub, image.dmem.len, image.dmem.addr, image.imem.len, image.imem.addr,
                     if image.emem.is_some() { ", EMEM" } else { "" }));
    fw::download(&mut r, &mut region, file, &image).map_err(|e| (Stage::Firmware, e))?;
    dma::program(&mut r, &mut region, &mut [&mut mgmt, &mut be, &mut h2c_ring]);
    let mut h2c = H2c::new();
    h2c.command(&mut r, &h2c::RECOVER_BT_DEV).map_err(|e| (Stage::Firmware, e))?;
    say(format_args!("FIRMWARE RUNNING: MCUFW_CTRL {:04X}", r.r16(0x80)));
    let mut phy_map = [0u8; efuse::PHYSICAL];
    let mut log = [0u8; efuse::LOGICAL];
    efuse::read(&mut r, &mut phy_map).map_err(|e| (Stage::Efuse, e))?;
    efuse::decode(&phy_map, &mut log).map_err(|e| (Stage::Efuse, e))?;
    let board = Board::from(&log);
    let b = &board;
    say(format_args!("EFUSE: RFE {} PACKAGE {}, CRYSTAL {:02X}, BT COEX {} SHARED ANTENNA {}, COUNTRY {}{}, MAC {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                     b.rfe, b.pkg, b.crystal, b.btcoex, b.shared, b.country[0] as char, b.country[1] as char,
                     b.mac[0], b.mac[1], b.mac[2], b.mac[3], b.mac[4], b.mac[5]));
    say(format_args!("EFUSE 2.4 GHZ POWER: CCK {:02X?}, BW40 {:02X?}, OFDM {:+}, BW20 {:+}", b.power.cck, b.power.bw40, b.power.ofdm_diff, b.power.bw20_diff));
    // Linux drives RFE types 0, 2, 4 and 6 alone (rtw8821c_rfe_defs).
    if !matches!(b.rfe, 0 | 2 | 4 | 6) { return Err((Stage::Board, Error::Chip("an RFE type rtw88 does not drive"))); }
    if !b.mac_valid() { return Err((Stage::Board, Error::Chip("no valid MAC address in the eFuse"))); }
    let chip = Chip::pcie(cut, b.pkg, b.rfe);
    mac::init(&mut r, RX_LIMIT).map_err(|e| (Stage::Mac, e))?;
    let cck = phy::init(&mut r, &chip, b).map_err(|e| (Stage::Phy, e))?;
    h2c.packet(&mut r, &mut region, &mut h2c_ring, 0x0d, &h2c::general_info()).map_err(|e| (Stage::Commands, e))?;
    h2c.packet(&mut r, &mut region, &mut h2c_ring, 0x11, &h2c::phydm_info(b.rfe, cut)).map_err(|e| (Stage::Commands, e))?;
    let coex_board = coex::Board { rfe: b.rfe, shared: b.shared };
    coex::power_on(&mut r, &coex_board);
    h2c.command(&mut r, &coex::H2C_BT_WIFI_CONTROL).map_err(|e| (Stage::Coex, e))?;
    coex::wifi_only(&mut r, &coex_board).map_err(|e| (Stage::Coex, e))?;
    mac::rcr(&mut r, false);
    mac::station(&mut r, &b.mac);
    let mut radio = Radio {
        r, region, mgmt, be, h2c_ring, rx: Rx { next: 0 }, h2c, board, chip, cut, cck, mac: board.mac, channel: 0,
        all_beacons: true, bssid: [0; 6], own: board.mac, queue, head: 0, count: 0, cookie: 0, report: None,
        counts: Counts::default(), iqk: None,
    };
    radio.set_channel(1).map_err(|_| (Stage::Channel, Error::Chip("channel 1 was refused")))?;
    radio.h2c.packet(&mut radio.r, &mut radio.region, &mut radio.h2c_ring, 0x0e, &h2c::iqk()).map_err(|e| (Stage::Iqk, e))?;
    match phy::iqk_wait(&mut radio.r) {
        Ok(status) => { radio.iqk = Some(status); say(format_args!("IQK DONE: 0x1BF0 {:08X}", status)); }
        Err(e) => say(format_args!("IQK NOT REPORTED DONE ({:?}): GOING ON WITHOUT IT", e)),
    }
    Ok(radio)
}


impl<R: Regs> Radio<'_, R> {
    pub fn channel(&self) -> u8 { self.channel }
    pub fn waiting(&self) -> usize { self.count }
    fn slots(&self) -> usize { self.queue.len() / QUEUE_SLOT }

    pub fn set_channel(&mut self, ch: u8) -> Result<(), Fail> {
        if !(1..=13).contains(&ch) { return Err(Fail::Invalid); }
        phy::channel(&mut self.r, ch, &self.board, &self.cck).map_err(|_| Fail::Failed)?;
        self.channel = ch;
        Ok(())
    }

    /// The receive filter: the hardware's by BSSID for beacons when a BSSID is set and no scan runs; the rest in
    /// software (txrx::wanted).
    pub fn set_filter(&mut self, all_beacons: bool, bssid: [u8; 6], own: [u8; 6]) {
        self.all_beacons = all_beacons;
        self.bssid = bssid;
        self.own = if own == [0; 6] { self.mac } else { own };
        mac::set_mac(&mut self.r, &self.own);
        mac::set_bssid(&mut self.r, &bssid);
        mac::rcr(&mut self.r, !all_beacons && bssid != [0; 6]);
    }

    /// The RX ring drained: firmware reports kept, wanted frames queued; a frame finding the queue full is dropped, so
    /// that the reports behind it still come.
    pub fn poll(&mut self) {
        let mut ready = self.rx.ready(&mut self.r);
        while ready > 0 {
            ready -= 1;
            self.counts.frames += 1;
            let rfe0 = self.board.rfe == 0;
            let buf = self.rx.buffer(&mut self.region);
            let d = txrx::Rx::parse(buf);
            match d {
                Some(d) if d.start() + d.len <= RX_BUF_BYTES => {
                    let start = d.start();
                    if d.c2h {
                        self.counts.c2h += 1;
                        match txrx::report(&buf[start..start + d.len]) {
                            Some(rep) => { self.counts.reports += 1; self.report = Some(rep); }
                            None => self.counts.other_c2h += 1,
                        }
                    } else if d.crc {
                        self.counts.crc += 1;
                    } else if d.len < 14 || d.len - 4 > FRAME_MAX {
                        self.counts.dropped += 1;
                    } else {
                        let frame = &buf[start..start + d.len - 4];
                        let slots = self.queue.len() / QUEUE_SLOT;
                        if !txrx::wanted(frame, self.all_beacons, &self.own, &self.bssid) {
                        } else if self.count == slots {
                            self.counts.dropped += 1;
                        } else {
                            let dbm = if d.physt { txrx::signal(&buf[24 + d.shift..24 + d.shift + 32], rfe0) } else { None };
                            let weakness = dbm.map_or(0, |v| (-v).clamp(1, 255) as u8);
                            let len = frame.len();
                            let header = [len as u8, (len >> 8) as u8, weakness, self.channel, txrx::rate_500k(d.rate), self.all_beacons as u8, 0, 0];
                            let at = (self.head + self.count) % slots * QUEUE_SLOT;
                            self.queue[at..at + RADIO_HEADER].copy_from_slice(&header);
                            self.queue[at + RADIO_HEADER..at + RADIO_HEADER + len].copy_from_slice(frame);
                            self.count += 1;
                            self.counts.kept += 1;
                        }
                    }
                }
                _ => self.counts.dropped += 1,
            }
            self.rx.done(&mut self.r, &mut self.region);
        }
    }

    /// The oldest queued frame, led by its 8-byte header.
    pub fn take(&mut self, out: &mut [u8]) -> Result<usize, Fail> {
        if self.count == 0 { return Err(Fail::Empty); }
        let at = self.head * QUEUE_SLOT;
        let len = RADIO_HEADER + u16::from_le_bytes([self.queue[at], self.queue[at + 1]]) as usize;
        out[..len].copy_from_slice(&self.queue[at..at + len]);
        self.head = (self.head + 1) % self.slots();
        self.count -= 1;
        Ok(len)
    }

    /// One frame sent at `rate` (500 kb/s units), at most `attempts` times; answered when the firmware reports it.
    pub fn send(&mut self, frame: &[u8], rate: u8, attempts: u8) -> Result<Sent, Fail> {
        if frame.len() < 10 || frame.len() > FRAME_MAX || !(1..=15).contains(&attempts) { return Err(Fail::Invalid); }
        let desc_rate = txrx::desc_rate(rate).ok_or(Fail::Invalid)?;
        if self.channel == 0 { return Err(Fail::Failed); }
        let data = frame[0] >> 2 & 3 == 2;
        let group = frame[4] & 1 != 0;
        let seq = if frame.len() >= 24 { u16::from_le_bytes([frame[22], frame[23]]) >> 4 } else { 0 };
        self.cookie = (self.cookie + 1) & 0x3f;
        let ring = if data { &mut self.be } else { &mut self.mgmt };
        if !ring.room(&mut self.r) { return Err(Fail::Busy); }
        let slot = ring.slot(&mut self.region);
        txrx::tx_desc(&mut slot[..TX_DESC], &txrx::Frame { len: frame.len(), qsel: if data { txrx::QSEL_BE } else { txrx::QSEL_MGMT },
            rate: desc_rate, tries: attempts, group, seq, cookie: self.cookie });
        slot[TX_DESC..TX_DESC + frame.len()].copy_from_slice(frame);
        self.report = None;
        ring.post(&mut self.r, &mut self.region, frame.len());
        self.counts.sent += 1;
        let mut waited = 0;
        while waited < REPORT_US {
            self.poll();
            if let Some(rep) = self.report.take() {
                if rep.cookie == self.cookie {
                    let acked = rep.acked && !group;
                    if acked { self.counts.acked += 1; }
                    return Ok(Sent { acked, attempts: rep.tries.unwrap_or(1).clamp(1, attempts) });
                }
            }
            self.r.udelay(100);
            waited += 100;
        }
        self.counts.no_report += 1;
        Ok(Sent { acked: false, attempts })
    }
}
