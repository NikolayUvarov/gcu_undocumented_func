// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2020 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
// Register definitions adapted from Linux v6.19 rtw89/reg.h and core.c.

pub const VENDOR_ID: u16 = 0x10ec;
pub const DEVICE_ID: u16 = 0xb852;
pub const MMIO_BAR: usize = 2;
pub const SYS_CFG1: usize = 0x00f0;
pub const CHIP_VERSION_MASK: u32 = 0x0000_f000;

/// Extract the digital cut without performing the indirect XTAL_SI transaction.
pub fn chip_cut(sys_cfg1: u32) -> Option<u8> {
    if sys_cfg1 == u32::MAX {
        None
    } else {
        Some(((sys_cfg1 & CHIP_VERSION_MASK) >> 12) as u8)
    }
}

// Initial board access definitions from the same pinned reg.h and mac.h.
pub const B_AX_AFC_AFEDIG: u32 = 0x00020000;
pub const B_AX_AFSM_PCIE_SUS_EN: u32 = 0x00001000;
pub const B_AX_AFSM_WLSUS_EN: u32 = 0x00000800;
pub const B_AX_AON_OFF_PC_EN: u32 = 0x00800000;
pub const B_AX_APDM_HPDN: u32 = 0x00008000;
pub const B_AX_APFM_OFFMAC: u32 = 0x00000200;
pub const B_AX_APFM_SWLPS: u32 = 0x00000400;
pub const B_AX_APFN_ONMAC: u32 = 0x00000100;
pub const B_AX_AUTOLOAD_SUS: u32 = 0x00000020;
pub const B_AX_BBRPT_EN: u32 = 0x00020000;
pub const B_AX_C1_L1_MASK: u32 = 0x00000003;
pub const B_AX_C3_L1_MASK: u32 = 0x00000030;
pub const B_AX_CMAC_DMA_EN: u32 = 0x00000010;
pub const B_AX_CMAC_EN: u32 = 0x40000000;
pub const B_AX_CMAC_RXEN: u32 = 0x10000000;
pub const B_AX_CMAC_TXEN: u32 = 0x20000000;
pub const B_AX_DISPATCHER_EN: u32 = 0x00040000;
pub const B_AX_DIS_WLBT_LPSEN_LOPC: u32 = 0x00000002;
pub const B_AX_DIS_WLBT_PDNSUSEN_SOPC: u32 = 0x00040000;
pub const B_AX_DLE_CPUIO_EN: u32 = 0x00080000;
pub const B_AX_DLE_PLE_EN: u32 = 0x00800000;
pub const B_AX_DLE_WDE_EN: u32 = 0x04000000;
pub const B_AX_DMACREG_GCKEN: u32 = 0x00008000;
pub const B_AX_DMAC_FUNC_EN: u32 = 0x20000000;
pub const B_AX_DMAC_TBL_EN: u32 = 0x00200000;
pub const B_AX_EF_ADDR_MASK: u32 = 0x07ff0000;
pub const B_AX_EF_RDY: u32 = 0x20000000;
pub const B_AX_EN_WLON: u32 = 0x00010000;
pub const B_AX_FEN_BBRSTB: u32 = 0x00000001;
pub const B_AX_FEN_BB_GLB_RSTN: u32 = 0x00000002;
pub const B_AX_FORCE_CMACREG_GCKEN: u32 = 0x00008000;
pub const B_AX_ISO_EB2CORE: u32 = 0x00000100;
pub const B_AX_MAC_FUNC_EN: u32 = 0x40000000;
pub const B_AX_MAC_SEC_EN: u32 = 0x00010000;
pub const B_AX_MPDU_PROC_EN: u32 = 0x10000000;
pub const B_AX_PCIE_CALIB_EN_V1: u32 = 0x00001000;
pub const B_AX_PHYINTF_EN: u32 = 0x00000020;
pub const B_AX_PINMUX_EESK_FUNC_SEL_MASK: u32 = 0x000000f0;
pub const B_AX_PKT_BUF_EN: u32 = 0x00400000;
pub const B_AX_PKT_IN_EN: u32 = 0x00100000;
pub const B_AX_PLATFORM_EN: u32 = 0x00000001;
pub const B_AX_PTCLTOP_EN: u32 = 0x00000008;
pub const B_AX_PWC_EV2EF_B14: u32 = 0x00004000;
pub const B_AX_PWC_EV2EF_B15: u32 = 0x00008000;
pub const B_AX_RDY_SYSPWR: u32 = 0x00020000;
pub const B_AX_REG_ZCDC_H_MASK: u32 = 0x00060000;
pub const B_AX_RMAC_EN: u32 = 0x00000001;
pub const B_AX_SCHEDULER_EN: u32 = 0x00000004;
pub const B_AX_STA_SCH_EN: u32 = 0x01000000;
pub const B_AX_SYM_CTRL_SPS_PWMFREQ: u32 = 0x00000400;
pub const B_AX_SYM_PADPDN_WL_PTA_1P3: u32 = 0x00000040;
pub const B_AX_SYM_PADPDN_WL_RFC_1P3: u32 = 0x00000020;
pub const B_AX_SYSON_DIS_PMCR_AX_WRMSK: u32 = 0x00000004;
pub const B_AX_TMAC_EN: u32 = 0x00000002;
pub const B_AX_TXPKT_CTRL_EN: u32 = 0x02000000;
pub const B_AX_WD_RLS_EN: u32 = 0x08000000;
pub const B_AX_WLMAC_PWR_STE_MASK: u32 = 0x00000300;
pub const B_AX_WL_XTAL_SI_CMD_POLL: u32 = 0x80000000;
pub const MAC_AX_NOTIFY_PWR_MAJOR: u32 = 0x00000080;
pub const MAC_AX_NOTIFY_TP_MAJOR: u32 = 0x00000081;
pub const PINMUX_EESK_FUNC_SEL_BT_LOG: u32 = 0x00000001;
pub const RTL8852B_RFE_05_SPS_ANA: u32 = 0x00004a82;
pub const R_AX_AFE_LDO_CTRL: usize = 0x00000020;
pub const R_AX_CMAC_FUNC_EN: usize = 0x0000c000;
pub const R_AX_DMAC_FUNC_EN: usize = 0x00008400;
pub const R_AX_EECS_EESK_FUNC_SEL: usize = 0x000002d8;
pub const R_AX_EFUSE_CTRL: usize = 0x00000030;
pub const R_AX_IC_PWR_STATE: usize = 0x000003f0;
pub const R_AX_PLATFORM_ENABLE: usize = 0x00000088;
pub const R_AX_PMC_DBG_CTRL2: usize = 0x000000cc;
pub const R_AX_SCOREBOARD: usize = 0x000000ac;
pub const R_AX_SPS_ANA_ON_CTRL2: usize = 0x00000228;
pub const R_AX_SPS_DIG_OFF_CTRL0: usize = 0x00000400;
pub const R_AX_SPS_DIG_ON_CTRL0: usize = 0x00000200;
pub const R_AX_SYS_ADIE_PAD_PWR_CTRL: usize = 0x00000018;
pub const R_AX_SYS_FUNC_EN: usize = 0x00000002;
pub const R_AX_SYS_ISO_CTRL: usize = 0x00000000;
pub const R_AX_SYS_PW_CTRL: usize = 0x00000004;
pub const R_AX_SYS_SDIO_CTRL: usize = 0x00000070;
pub const R_AX_SYS_SWR_CTRL1: usize = 0x00000010;
pub const R_AX_SYS_WL_EFUSE_CTRL: usize = 0x0000000a;
pub const R_AX_WLAN_XTAL_SI_CTRL: usize = 0x00000270;
pub const R_AX_WLLPS_CTRL: usize = 0x00000090;
pub const R_AX_WLRF_CTRL: usize = 0x000002f0;
pub const SW_LPS_OPTION: u32 = 0x0001a0b2;
pub const XTAL_SI_ANAPAR_WL: u32 = 0x00000090;
pub const XTAL_SI_CTRL: u32 = 0x00000063;
pub const XTAL_SI_GND_SHDN_WL: u32 = 0x00000040;
pub const XTAL_SI_HIGH_ADDR_MASK: u32 = 0x00000007;
pub const XTAL_SI_LDO_LPS: u32 = 0x00000070;
pub const XTAL_SI_LOW_ADDR: u32 = 0x00000062;
pub const XTAL_SI_LOW_ADDR_MASK: u32 = 0x000000ff;
pub const XTAL_SI_LPS_CAP: u32 = 0x0000000f;
pub const XTAL_SI_MODE_SEL_MASK: u32 = 0x000000c0;
pub const XTAL_SI_NORMAL_READ: u32 = 0x00000001;
pub const XTAL_SI_OFF_EI: u32 = 0x00000008;
pub const XTAL_SI_OFF_WEI: u32 = 0x00000004;
pub const XTAL_SI_PON_EI: u32 = 0x00000002;
pub const XTAL_SI_PON_WEI: u32 = 0x00000001;
pub const XTAL_SI_RDY: u32 = 0x00000020;
pub const XTAL_SI_READ_VAL: u32 = 0x0000007a;
pub const XTAL_SI_RF00: u32 = 0x00000001;
pub const XTAL_SI_RF10: u32 = 0x00000001;
pub const XTAL_SI_RFC2RF: u32 = 0x00000010;
pub const XTAL_SI_SHDN_WL: u32 = 0x00000020;
pub const XTAL_SI_SRAM2RFC: u32 = 0x00000080;
pub const XTAL_SI_SRAM_CTRL: u32 = 0x000000a1;
pub const XTAL_SI_SRAM_DIS: u32 = 0x00000002;
pub const XTAL_SI_WL_RFC_S0: u32 = 0x00000080;
pub const XTAL_SI_WL_RFC_S1: u32 = 0x00000081;
pub const XTAL_SI_XTAL_XMD_2: u32 = 0x00000024;
pub const XTAL_SI_XTAL_XMD_4: u32 = 0x00000026;
