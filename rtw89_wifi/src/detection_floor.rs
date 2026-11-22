//! 213-DRV-0102: the receiver's detection floor as Linux's DIG and EDCCA would hold it for the joined access point's RSSI
//! (phy.c, phy.h, reg.h and rtw8852b.c at 05f7e89), written once and not tracked: a static floor, not a DIG.

/// False (as the driver was): the floor stays as the initial PHY left it. True: written once the joined access point's
/// RSSI is measured, on a 2.4 GHz 20 MHz channel only, and again after a recovery or a new link.
pub(crate) const DETECTION_FLOOR_AS_LINUX: bool = false;
/// A bench mode, false: with it and DETECTION_FLOOR_AS_LINUX true, the floor is Linux's for one minute and the values
/// found before the first write for the next, turn and turn about from the first whole minute after the beacons are
/// measured (off first), so that adjacent minutes of one boot compare the two.
pub(crate) const DETECTION_FLOOR_ALTERNATES: bool = false;

// phy.c: pd_low_th_offset 16, plus PD_TH_SB_FLTR_CMP_VAL 7 on the AX generation and PD_TH_BW20_CMP_VAL 0 (phy.h).
const UNDER_REGION: i16 = 16 + 7;
const PD_TH_MIN_RSSI: i16 = 8;
const PD_TH_MAX_RSSI: i16 = 70;
const CCKPD_TH_MIN_RSSI: i16 = -18;
const IGI_RSSI_MAX: i16 = 110;
const EDCCA_TH_L2H_LB: i16 = 66;

/// The registers' values: 0x4860[10:6], 0x4B64[31:24], and 0x4884's [7:0], [15:8] and [31:24].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Floor { pub ofdm_pd: u8, pub cck_pd: u8, pub edcca: u8 }

/// `rssi_min >> 1` for a signal of `dbm`: RTW89_RSSI_RAW_TO_DBM's half-dB units above -110 dBm, halved (core.h).
pub(crate) fn igi_rssi(dbm: i8) -> u8 { (i16::from(dbm) + 110).clamp(0, 255) as u8 }

/// `__rtw89_phy_dig_dyn_pd_th`: the OFDM packet-detection lower bound, 2 dB a step above -102 dBm.
pub(crate) fn ofdm_pd(igi: u8) -> u8 {
    let th = i16::from(igi).clamp(PD_TH_MIN_RSSI + UNDER_REGION, PD_TH_MAX_RSSI + UNDER_REGION);
    ((th - UNDER_REGION - PD_TH_MIN_RSSI) >> 1) as u8
}

/// `rtw89_phy_dig_dyn_pd_th`'s CCK part: max(igi - under region, -18) - 110 as a signed byte.
pub(crate) fn cck_pd(igi: u8) -> u8 {
    let th = ((i16::from(igi) - UNDER_REGION) as i8).max(CCKPD_TH_MIN_RSSI as i8);
    (i16::from(th) - IGI_RSSI_MAX) as i8 as u8
}

/// `rtw89_phy_edcca_get_thre_by_rssi`, linked: igi - 110 + 128 - 3, at least EDCCA_TH_L2H_LB (66); dBm + 128.
pub(crate) fn edcca(igi: u8) -> u8 { (i16::from(igi) - 110 + 128 - 3).max(EDCCA_TH_L2H_LB).min(255) as u8 }

/// The floor for a joined access point heard at `dbm`.
pub(crate) fn for_rssi(dbm: i8) -> Floor {
    let igi = igi_rssi(dbm);
    Floor { ofdm_pd: ofdm_pd(igi), cck_pd: cck_pd(igi), edcca: edcca(igi) }
}

/// BB registers (PHY base 0x10000): what the floor writes and the probe reads.
pub(crate) const PD: usize = 0x4860;
pub(crate) const CCK_PD: usize = 0x4b64;
pub(crate) const CCK_LIMIT: usize = 0x4b74;
pub(crate) const EDCCA: usize = 0x4884;
pub(crate) const REGISTERS: [usize; 4] = [PD, CCK_PD, CCK_LIMIT, EDCCA];

/// The same fields with the values `found` in the four registers (REGISTERS' order) before the first write.
pub(crate) fn restore(floor: &Floor, found: &[u32; 4]) -> [(usize, u32, u32); 5] {
    fields(floor).map(|(at, mask, _)| (at, mask, found[REGISTERS.iter().position(|r| *r == at).unwrap_or(0)] & mask))
}

/// The fields written, as `rtw89_phy_dig_dyn_pd_th` and `rtw89_phy_edcca_thre_calc` write them: (register, mask, value).
pub(crate) fn fields(floor: &Floor) -> [(usize, u32, u32); 5] {
    [(PD, 0x7c0, u32::from(floor.ofdm_pd) << 6), (PD, 1 << 30, 1 << 30), (CCK_PD, 0xff00_0000, u32::from(floor.cck_pd) << 24),
     (CCK_LIMIT, 1 << 30, 1 << 30), (EDCCA, 0xff00_ffff, u32::from(floor.edcca) * 0x0100_0101)]
}
