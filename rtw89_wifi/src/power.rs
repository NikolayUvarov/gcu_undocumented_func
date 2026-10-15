// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! Initial board-read power lifecycle, adapted from pinned Linux rtw8852b.c and mac.c; not radio startup.

use crate::io::{modify8, modify32, poll32, read32, Fault, FaultKind, RegisterIo, Result};
use crate::registers::*;

pub const SUPPORTED_CUT: u8 = 1;

pub fn identify(io: &mut impl RegisterIo) -> Result<u8> {
    let value = read32(io, SYS_CFG1)?;
    let cut = chip_cut(value).ok_or(Fault::new(FaultKind::Unavailable, SYS_CFG1, value))?;
    if value == 0 || cut != SUPPORTED_CUT { return Err(Fault::new(FaultKind::UnsupportedCut, SYS_CFG1, value)); }
    Ok(cut)
}

pub fn xtal_write(io: &mut impl RegisterIo, offset: u8, value: u8, mask: u8) -> Result<()> {
    poll32(io, R_AX_WLAN_XTAL_SI_CTRL, B_AX_WL_XTAL_SI_CMD_POLL, 0, 50, 50_000)?;
    let command = B_AX_WL_XTAL_SI_CMD_POLL | (mask as u32) << 16 | (value as u32) << 8 | offset as u32;
    io.write32(R_AX_WLAN_XTAL_SI_CTRL, command)?;
    poll32(io, R_AX_WLAN_XTAL_SI_CTRL, B_AX_WL_XTAL_SI_CMD_POLL, 0, 50, 50_000).map(|_| ())
}

pub fn xtal_read(io: &mut impl RegisterIo, offset: u8) -> Result<u8> {
    poll32(io, R_AX_WLAN_XTAL_SI_CTRL, B_AX_WL_XTAL_SI_CMD_POLL, 0, 50, 50_000)?;
    let command = B_AX_WL_XTAL_SI_CMD_POLL | (XTAL_SI_NORMAL_READ << 24) | offset as u32;
    io.write32(R_AX_WLAN_XTAL_SI_CTRL, command)?;
    poll32(io, R_AX_WLAN_XTAL_SI_CTRL, B_AX_WL_XTAL_SI_CMD_POLL, 0, 50, 50_000)?;
    io.read8(R_AX_WLAN_XTAL_SI_CTRL + 1)
}

fn rfe_supply(io: &mut impl RegisterIo, rfe: Option<u8>) -> Result<()> {
    if rfe == Some(5) { io.write16(R_AX_SPS_ANA_ON_CTRL2, RTL8852B_RFE_05_SPS_ANA as u16)?; }
    Ok(())
}

/// First power-on before EFUSE is known; later calibrated radio startup needs the separate power-trim path.
pub fn on_for_board_read(io: &mut impl RegisterIo) -> Result<()> {
    modify32(io, R_AX_SYS_PW_CTRL, B_AX_AFSM_WLSUS_EN | B_AX_AFSM_PCIE_SUS_EN, 0)?;
    modify32(io, R_AX_SYS_PW_CTRL, 0, B_AX_DIS_WLBT_PDNSUSEN_SOPC)?;
    modify32(io, R_AX_WLLPS_CTRL, 0, B_AX_DIS_WLBT_LPSEN_LOPC)?;
    modify32(io, R_AX_SYS_PW_CTRL, B_AX_APDM_HPDN, 0)?;
    modify32(io, R_AX_SYS_PW_CTRL, B_AX_APFM_SWLPS, 0)?;
    poll32(io, R_AX_SYS_PW_CTRL, B_AX_RDY_SYSPWR, B_AX_RDY_SYSPWR, 1_000, 20_000)?;
    modify32(io, R_AX_AFE_LDO_CTRL, 0, B_AX_AON_OFF_PC_EN)?;
    poll32(io, R_AX_AFE_LDO_CTRL, B_AX_AON_OFF_PC_EN, B_AX_AON_OFF_PC_EN, 1_000, 20_000)?;
    modify32(io, R_AX_SPS_DIG_OFF_CTRL0, B_AX_C1_L1_MASK, 1)?;
    modify32(io, R_AX_SPS_DIG_OFF_CTRL0, B_AX_C3_L1_MASK, 3 << 4)?;
    modify32(io, R_AX_SYS_PW_CTRL, 0, B_AX_EN_WLON)?;
    modify32(io, R_AX_SYS_PW_CTRL, 0, B_AX_APFN_ONMAC)?;
    poll32(io, R_AX_SYS_PW_CTRL, B_AX_APFN_ONMAC, 0, 1_000, 20_000)?;
    for enabled in [true, false, true, false, true] {
        modify8(io, R_AX_PLATFORM_ENABLE, B_AX_PLATFORM_EN as u8, if enabled { B_AX_PLATFORM_EN as u8 } else { 0 })?;
    }
    modify32(io, R_AX_SYS_SDIO_CTRL, B_AX_PCIE_CALIB_EN_V1, 0)?;
    modify32(io, R_AX_SYS_ADIE_PAD_PWR_CTRL, 0, B_AX_SYM_PADPDN_WL_PTA_1P3)?;
    xtal_write(io, XTAL_SI_ANAPAR_WL as u8, XTAL_SI_GND_SHDN_WL as u8, XTAL_SI_GND_SHDN_WL as u8)?;
    modify32(io, R_AX_SYS_ADIE_PAD_PWR_CTRL, 0, B_AX_SYM_PADPDN_WL_RFC_1P3)?;
    for (value, mask) in [
        (XTAL_SI_SHDN_WL, XTAL_SI_SHDN_WL), (XTAL_SI_OFF_WEI, XTAL_SI_OFF_WEI),
        (XTAL_SI_OFF_EI, XTAL_SI_OFF_EI), (0, XTAL_SI_RFC2RF),
        (XTAL_SI_PON_WEI, XTAL_SI_PON_WEI), (XTAL_SI_PON_EI, XTAL_SI_PON_EI), (0, XTAL_SI_SRAM2RFC),
    ] { xtal_write(io, XTAL_SI_ANAPAR_WL as u8, value as u8, mask as u8)?; }
    xtal_write(io, XTAL_SI_SRAM_CTRL as u8, 0, XTAL_SI_SRAM_DIS as u8)?;
    xtal_write(io, XTAL_SI_XTAL_XMD_2 as u8, 0, XTAL_SI_LDO_LPS as u8)?;
    xtal_write(io, XTAL_SI_XTAL_XMD_4 as u8, 0, XTAL_SI_LPS_CAP as u8)?;
    modify32(io, R_AX_PMC_DBG_CTRL2, 0, B_AX_SYSON_DIS_PMCR_AX_WRMSK)?;
    modify32(io, R_AX_SYS_ISO_CTRL, 0, B_AX_ISO_EB2CORE)?;
    modify32(io, R_AX_SYS_ISO_CTRL, B_AX_PWC_EV2EF_B15, 0)?;
    io.delay_us(1_000)?;
    modify32(io, R_AX_SYS_ISO_CTRL, B_AX_PWC_EV2EF_B14, 0)?;
    modify32(io, R_AX_PMC_DBG_CTRL2, B_AX_SYSON_DIS_PMCR_AX_WRMSK, 0)?;
    modify32(io, R_AX_DMAC_FUNC_EN, 0,
        B_AX_MAC_FUNC_EN | B_AX_DMAC_FUNC_EN | B_AX_MPDU_PROC_EN | B_AX_WD_RLS_EN | B_AX_DLE_WDE_EN |
        B_AX_TXPKT_CTRL_EN | B_AX_STA_SCH_EN | B_AX_DLE_PLE_EN | B_AX_PKT_BUF_EN | B_AX_DMAC_TBL_EN |
        B_AX_PKT_IN_EN | B_AX_DLE_CPUIO_EN | B_AX_DISPATCHER_EN | B_AX_BBRPT_EN | B_AX_MAC_SEC_EN | B_AX_DMACREG_GCKEN)?;
    modify32(io, R_AX_CMAC_FUNC_EN, 0,
        B_AX_CMAC_EN | B_AX_CMAC_TXEN | B_AX_CMAC_RXEN | B_AX_FORCE_CMACREG_GCKEN | B_AX_PHYINTF_EN |
        B_AX_CMAC_DMA_EN | B_AX_PTCLTOP_EN | B_AX_SCHEDULER_EN | B_AX_TMAC_EN | B_AX_RMAC_EN)?;
    modify32(io, R_AX_EECS_EESK_FUNC_SEL, B_AX_PINMUX_EESK_FUNC_SEL_MASK,
        PINMUX_EESK_FUNC_SEL_BT_LOG << B_AX_PINMUX_EESK_FUNC_SEL_MASK.trailing_zeros())?;
    io.write8(R_AX_SCOREBOARD + 3, MAC_AX_NOTIFY_TP_MAJOR as u8)
}

pub fn off(io: &mut impl RegisterIo, rfe: Option<u8>) -> Result<()> {
    identify(io)?;
    rfe_supply(io, rfe)?;
    for (value, mask) in [(XTAL_SI_RFC2RF, XTAL_SI_RFC2RF), (0, XTAL_SI_OFF_EI), (0, XTAL_SI_OFF_WEI)] {
        xtal_write(io, XTAL_SI_ANAPAR_WL as u8, value as u8, mask as u8)?;
    }
    xtal_write(io, XTAL_SI_WL_RFC_S0 as u8, 0, XTAL_SI_RF00 as u8)?;
    xtal_write(io, XTAL_SI_WL_RFC_S1 as u8, 0, XTAL_SI_RF10 as u8)?;
    for (value, mask) in [(XTAL_SI_SRAM2RFC, XTAL_SI_SRAM2RFC), (0, XTAL_SI_PON_EI), (0, XTAL_SI_PON_WEI)] {
        xtal_write(io, XTAL_SI_ANAPAR_WL as u8, value as u8, mask as u8)?;
    }
    modify32(io, R_AX_SYS_PW_CTRL, 0, B_AX_EN_WLON)?;
    modify32(io, R_AX_WLRF_CTRL, B_AX_AFC_AFEDIG, 0)?;
    modify8(io, R_AX_SYS_FUNC_EN, (B_AX_FEN_BB_GLB_RSTN | B_AX_FEN_BBRSTB) as u8, 0)?;
    modify32(io, R_AX_SYS_ADIE_PAD_PWR_CTRL, B_AX_SYM_PADPDN_WL_RFC_1P3, 0)?;
    xtal_write(io, XTAL_SI_ANAPAR_WL as u8, 0, XTAL_SI_SHDN_WL as u8)?;
    modify32(io, R_AX_SYS_ADIE_PAD_PWR_CTRL, B_AX_SYM_PADPDN_WL_PTA_1P3, 0)?;
    xtal_write(io, XTAL_SI_ANAPAR_WL as u8, 0, XTAL_SI_GND_SHDN_WL as u8)?;
    modify32(io, R_AX_SYS_PW_CTRL, 0, B_AX_APFM_OFFMAC)?;
    poll32(io, R_AX_SYS_PW_CTRL, B_AX_APFM_OFFMAC, 0, 1_000, 20_000)?;
    io.write32(R_AX_WLLPS_CTRL, SW_LPS_OPTION)?;
    modify32(io, R_AX_SYS_SWR_CTRL1, 0, B_AX_SYM_CTRL_SPS_PWMFREQ)?;
    modify32(io, R_AX_SPS_DIG_ON_CTRL0, B_AX_REG_ZCDC_H_MASK, 3 << B_AX_REG_ZCDC_H_MASK.trailing_zeros())?;
    modify32(io, R_AX_SYS_PW_CTRL, 0, B_AX_APFM_SWLPS)?;
    io.write8(R_AX_SCOREBOARD + 3, MAC_AX_NOTIFY_PWR_MAJOR as u8)?;
    let state = read32(io, R_AX_IC_PWR_STATE)?;
    if state & B_AX_WLMAC_PWR_STE_MASK != 0 {
        return Err(Fault::new(FaultKind::PowerState, R_AX_IC_PWR_STATE, state));
    }
    let bb = io.read8(R_AX_SYS_FUNC_EN)?;
    if bb & (B_AX_FEN_BB_GLB_RSTN | B_AX_FEN_BBRSTB) as u8 != 0 {
        return Err(Fault::new(FaultKind::PowerState, R_AX_SYS_FUNC_EN, bb as u32));
    }
    Ok(())
}
