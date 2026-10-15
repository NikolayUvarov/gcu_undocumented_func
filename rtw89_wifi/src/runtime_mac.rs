// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022, 2024 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! Bounded RTL8852B SCC MAC configuration with packet scheduling stopped; not a WLAN-ready state.

use crate::board::Board;
use crate::download;
use crate::fwcmd::{self, Bus, Queue};
use crate::io::{self, Fault, FaultKind, RegisterIo, TimedIo};
use crate::{mac, power};
use crate::registers::*;
use crate::runtime_mac_regs::*;

pub const BAR_BYTES: usize = 0x20000;
const PACKET_STOP: u32 = 0x000b_0f03;
const STAGE_US: u64 = 2_000_000;
const DLE: u32 = 0x0480_0000;
const HFC_ENABLE: u32 = 9;
const PLE_MIN: [u32; 11] = [147,0,16,20,17,13,89,0,32,14,8];
const PLE_MAX: [u32; 11] = [147,0,16,20,157,13,229,0,172,14,24];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage { Guard, PacketStop, Baseband, Functions, Memory, FlowControl, Station, MpduSecurity, Cmac, Verify }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Failure {
    pub stage: Stage, pub cause: Fault, pub packet_stop: Option<Fault>, pub radio_stop: Option<Fault>,
    pub command_stop: Option<download::Stop>,
}

/// Registers prepared with packet scheduling stopped; interrupts, packet C2H and final offload setup are not provided.
#[derive(Debug)]
pub struct Prepared { _private: () }

fn fault(at: usize, value: u32) -> Fault { Fault::new(FaultKind::PowerState,at,value) }
fn require(bus: &mut impl RegisterIo, at: usize, mask: u32, expected: u32) -> io::Result<()> {
    let raw=io::read32(bus,at)?;
    if raw&mask!=expected { return Err(fault(at,raw)); } Ok(())
}
fn write(bus: &mut impl RegisterIo, at: usize, value: u32) -> io::Result<()> {
    bus.write32(at,value)?; require(bus,at,u32::MAX,value)
}
fn modify(bus: &mut impl RegisterIo, at: usize, clear: u32, set: u32) -> io::Result<()> {
    io::modify32(bus,at,clear,set)?; require(bus,at,clear|set,set)
}
fn modify8(bus: &mut impl RegisterIo, at: usize, clear: u32, set: u32) -> io::Result<()> {
    let raw=bus.read8(at)?; if raw==u8::MAX { return Err(fault(at,raw as u32)); }
    bus.write8(at,(raw&!(clear as u8))|set as u8)?;
    let raw=bus.read8(at)?; if raw as u32&(clear|set)!=set { return Err(fault(at,raw as u32)); } Ok(())
}
fn modify16(bus: &mut impl RegisterIo, at: usize, clear: u32, set: u32) -> io::Result<()> {
    let raw=bus.read16(at)?;
    bus.write16(at,(raw&!(clear as u16))|set as u16)?;
    let raw=bus.read16(at)?; if raw as u32&(clear|set)!=set { return Err(fault(at,raw as u32)); } Ok(())
}
fn field(bus: &mut impl RegisterIo, at: usize, mask: u32, value: u32) -> io::Result<()> {
    modify(bus,at,mask,(value<<mask.trailing_zeros())&mask)
}

fn guard(bus: &mut impl Bus, queue: &Queue<'_>, board: &Board, bar_bytes: usize) -> io::Result<()> {
    if bar_bytes<BAR_BYTES || board.identity.cut!=1 || board.identity.rfe!=1 { return Err(fault(BAR_BYTES,bar_bytes.min(u32::MAX as usize) as u32)); }
    if board.maps.phycap[0x6c..0x6e]!=[0xff,0xff] { return Err(fault(0x5ec,0)); }
    let command=bus.pci_command()?;
    if command==u16::MAX || command&6!=6 { return Err(Fault::new(FaultKind::Access,4,command as u32)); }
    if queue.state()!=fwcmd::State::Running || queue.pending()!=0 { return Err(fault(fwcmd::INDEX,queue.pending() as u32)); }
    require(bus,SYS_CFG1,CHIP_VERSION_MASK,1<<12)?;
    require(bus,R_AX_IC_PWR_STATE,B_AX_WLMAC_PWR_STE_MASK,1<<8)?;
    require(bus,R_AX_PLATFORM_ENABLE,3,3)?;
    require(bus,R_AX_CMAC_FUNC_EN,1<<30,1<<30)?;
    require(bus,0xe000,1<<30,0)?;
    require(bus,download::FW_CONTROL,0xe0,0xe0)?;
    let bb=bus.read8(R_AX_SYS_FUNC_EN)?;
    if bb&3!=0 { return Err(fault(R_AX_SYS_FUNC_EN,bb as u32)); }
    require(bus,mac::WDE_CONFIG,0x1fff3f03,0)?;
    require(bus,mac::PLE_CONFIG,0x1fff3f03,0x00400801)?;
    require(bus,fwcmd::INIT_CONFIG,1<<11|1<<13,1<<11)?;
    require(bus,fwcmd::DMA_STOP,0x001f0f03,PACKET_STOP)?;
    require(bus,fwcmd::DMA_BUSY,fwcmd::BUSY_MASK,0)?;
    require(bus,fwcmd::BASE_LOW,u32::MAX,queue.physical() as u32)?;
    require(bus,fwcmd::BASE_HIGH,u32::MAX,0)?;
    require(bus,fwcmd::BDRAM,u32::MAX,28|4<<8|1<<16)?;
    let number=bus.read16(fwcmd::NUMBER)?;
    if number!=32 { return Err(fault(fwcmd::NUMBER,number as u32)); }
    require(bus,R_AX_WLAN_XTAL_SI_CTRL,B_AX_WL_XTAL_SI_CMD_POLL,0)
}

fn poll8(bus: &mut impl RegisterIo, at: usize, nonzero: bool, interval: u32, timeout: u64) -> io::Result<()> {
    let mut timed=TimedIo::new(bus,timeout);
    for _ in 0..=timeout/u64::from(interval) {
        let raw=timed.read8(at)?;
        if raw==u8::MAX { return Err(Fault::new(FaultKind::Unavailable,at,raw as u32)); }
        if (raw!=0)==nonzero { return timed.delay_us(0); }
        timed.delay_us(interval)?;
    }
    Err(Fault::new(FaultKind::Timeout,at,0))
}
fn packet_status(bus: &mut impl RegisterIo) -> io::Result<()> {
    require(bus,R_AX_CTN_TXEN,0xffff,0)?;
    io::poll32(bus,R_AX_PTCL_TX_CTN_SEL,B_AX_PTCL_TX_ON_STAT,0,8,80_000)?;
    bus.delay_us(0)
}
fn counter(bus: &mut impl RegisterIo, mask: u8) -> io::Result<()> {
    let at=R_AX_UDM1+1;
    let original=bus.read8(at)?;
    let shift=mask.trailing_zeros();
    let next=(((original&mask)>>shift).wrapping_add(1)<<shift)&mask;
    bus.write8(at,(original&!mask)|next)
}
fn packet_stop(bus: &mut impl RegisterIo) -> io::Result<()> {
    // Firmware owns scheduler updates after FW_RDY; do not overwrite an outstanding register exchange.
    poll8(bus,R_AX_H2CREG_CTRL,false,1000,5000)?;
    let pending=bus.read8(R_AX_C2HREG_CTRL)?;
    if pending!=0 { return Err(fault(R_AX_C2HREG_CTRL,pending as u32)); }
    for (i,value) in [0x205,0xffff,0,0].into_iter().enumerate() { bus.write32(R_AX_H2CREG_DATA0+i*4,value)?; }
    counter(bus,(B_AX_UDM1_HALMAC_H2C_DEQ_CNT_MASK>>8) as u8)?;
    bus.write8(R_AX_H2CREG_CTRL,1)?;
    poll8(bus,R_AX_C2HREG_CTRL,true,1,1_000_000)?;
    let mut response=[0u32;4];
    for (i,value) in response.iter_mut().enumerate() { *value=io::read32(bus,R_AX_C2HREG_DATA0+i*4)?; }
    bus.write8(R_AX_C2HREG_CTRL,0)?;
    counter(bus,(B_AX_UDM1_HALMAC_C2H_ENQ_CNT_MASK>>8) as u8)?;
    let words=(response[0]>>8)&15;
    if response[0]&127!=4 || !(1..=4).contains(&words) { return Err(fault(R_AX_C2HREG_DATA0,response[0])); }
    packet_status(bus)
}

/// Initial calibration pause/restore may only send an already-zero scheduler mask.
/// Caller exclusively owns the register mailbox; any error forbids retry or restore through this mailbox.
/// Firmware ACK must be followed by actual CTN/PTCL idle; no packet TX is enabled by this exchange.
pub fn scheduler_zero(bus: &mut impl RegisterIo, timeout_us: u32) -> io::Result<()> {
    if timeout_us==0 || timeout_us>1_000_000 { return Err(Fault::new(FaultKind::Access,R_AX_CTN_TXEN,timeout_us)); }
    let mut timed=TimedIo::new(bus,u64::from(timeout_us));
    require(&mut timed,download::FW_CONTROL,0xe0,0xe0)?;
    require(&mut timed,R_AX_CMAC_FUNC_EN,1<<30,1<<30)?;
    require(&mut timed,R_AX_CTN_TXEN,0xffff,0)?;
    packet_stop(&mut timed)?;
    timed.delay_us(0)
}

fn xtal(bus: &mut impl RegisterIo, offset: u8, value: u8) -> io::Result<()> {
    io::poll32(bus,R_AX_WLAN_XTAL_SI_CTRL,B_AX_WL_XTAL_SI_CMD_POLL,0,50,50_000)?;
    power::xtal_write(bus,offset,value,255)?;
    let actual=power::xtal_read(bus,offset)?;
    if actual!=value { return Err(fault(offset as usize,actual as u32)); } Ok(())
}
fn enable_radio(bus: &mut impl RegisterIo) -> io::Result<()> {
    modify8(bus,R_AX_SYS_FUNC_EN,0,3)?;
    field(bus,R_AX_SPS_DIG_ON_CTRL0,B_AX_REG_ZCDC_H_MASK,1)?;
    for set in [B_AX_AFC_AFEDIG,0,B_AX_AFC_AFEDIG] { modify(bus,R_AX_WLRF_CTRL,B_AX_AFC_AFEDIG,set)?; }
    xtal(bus,0x80,0xc7)?; xtal(bus,0x81,0xc7)?;
    bus.write8(0x8040,0xe)?;
    let actual=bus.read8(0x8040)?; if actual!=0xe { return Err(fault(0x8040,actual as u32)); } Ok(())
}
fn disable_radio(bus: &mut impl RegisterIo) -> io::Result<()> {
    let mut timed=TimedIo::new(bus,400_000);
    let mut first=None;
    let mut remember=|result:io::Result<()>| { if let Err(error)=result { if first.is_none() { first=Some(error); } } };
    remember((|| { modify(&mut timed,R_AX_WLRF_CTRL,B_AX_AFC_AFEDIG,0)?; timed.delay_us(0) })());
    remember((|| { modify8(&mut timed,R_AX_SYS_FUNC_EN,3,0)?; timed.delay_us(0) })());
    for offset in [0x80u8,0x81] {
        remember((|| {
            io::poll32(&mut timed,R_AX_WLAN_XTAL_SI_CTRL,B_AX_WL_XTAL_SI_CMD_POLL,0,50,50_000)?;
            let original=power::xtal_read(&mut timed,offset)?;
            power::xtal_write(&mut timed,offset,original&!7,255)?;
            let actual=power::xtal_read(&mut timed,offset)?;
            if actual!=original&!7 { return Err(fault(offset as usize,actual as u32)); }
            timed.delay_us(0)
        })());
    }
    first.map_or(Ok(()),Err)
}
fn functions(bus: &mut impl RegisterIo) -> io::Result<()> {
    write(bus,R_AX_DMAC_FUNC_EN,0xfb7d0000)?;
    write(bus,mac::DMAC_CLOCK,0x0b1f0000)?;
    modify(bus,0xc004,0,0x4000003f)?;
    modify(bus,R_AX_CMAC_FUNC_EN,0,0xf000003f)?;
    modify(bus,R_AX_SPS_DIG_ON_CTRL0,0,B_AX_OCP_L1_MASK)
}
fn memory(bus: &mut impl RegisterIo) -> io::Result<()> {
    modify(bus,R_AX_DMAC_FUNC_EN,DLE,0)?;
    modify(bus,mac::DMAC_CLOCK,0,DLE)?;
    modify(bus,mac::WDE_CONFIG,0x1fff3f03,510<<16)?;
    modify(bus,mac::PLE_CONFIG,0x1fff3f03,496<<16|4<<8|1)?;
    for (at,pages) in [(0x8c40,446),(0x8c44,48),(0x8c4c,0),(0x8c50,16)] { write(bus,at,pages|pages<<16)?; }
    for i in 0..11 { write(bus,0x9040+i*4,PLE_MIN[i]|PLE_MAX[i]<<16)?; }
    modify(bus,R_AX_DMAC_FUNC_EN,0,DLE)?;
    io::poll32(bus,mac::WDE_READY,3,3,1,2000)?;
    io::poll32(bus,mac::PLE_READY,3,3,1,2000)?; Ok(())
}
fn flow_control(bus: &mut impl RegisterIo) -> io::Result<()> {
    modify(bus,mac::HFC_CONTROL,HFC_ENABLE,0)?;
    for (channel,min,max) in [(0,5,341),(1,5,341),(2,4,342),(3,4,342),(8,4,342),(9,4,342)] {
        write(bus,0x8a10+channel*4,min|max<<16)?;
    }
    for (at,value) in [(0x8a90,446),(0x8aa4,0),(mac::CHANNEL_PAGE,2|40<<16),(0x8a94,446),(0x8aa0,0)] { write(bus,at,value)?; }
    modify(bus,mac::HFC_CONTROL,0xff6,1<<4)?;
    modify(bus,mac::HFC_CONTROL,0,HFC_ENABLE)?;
    bus.delay_us(10)?;
    for channel in [0,1,2,3,8,9] { io::read32(bus,0x8a50+channel*4)?; }
    let used=io::read32(bus,0x8a98)?;
    let available=io::read32(bus,0x8a9c)?&0x1fff;
    if (used&0x1fff)+((used>>16)&0x1fff)+available!=446 { return Err(fault(0x8a98,used)); }
    Ok(())
}
fn station(bus: &mut impl RegisterIo) -> io::Result<()> {
    modify8(bus,R_AX_SS_CTRL,0,B_AX_SS_EN)?;
    io::poll32(bus,R_AX_SS_CTRL,B_AX_SS_INIT_DONE_1,B_AX_SS_INIT_DONE_1,1,2000)?;
    modify(bus,R_AX_SS_CTRL,0,B_AX_SS_WARM_INIT_FLG)?;
    modify(bus,R_AX_SS_CTRL,B_AX_SS_NONEMPTY_SS2FINFO_EN,0)
}
fn mpdu_security(bus: &mut impl RegisterIo) -> io::Result<()> {
    write(bus,R_AX_ACTION_FWD0,0x02a95a95)?;
    write(bus,R_AX_TF_FWD,0xaa55)?;
    modify(bus,R_AX_MPDU_PROC,0,B_AX_APPEND_FCS|B_AX_A_ICV_ERR)?;
    write(bus,R_AX_CUT_AMSDU_CTRL,0x010e05f0)?;
    modify(bus,R_AX_SEC_ENG_CTRL,B_AX_TX_PARTIAL_MODE,B_AX_CLK_EN_CGCMP|B_AX_CLK_EN_WAPI|B_AX_CLK_EN_WEP_TKIP|B_AX_SEC_TX_ENC|B_AX_SEC_RX_DEC|B_AX_MC_DEC|B_AX_BC_DEC)?;
    modify(bus,R_AX_SEC_MPDU_PROC,0,B_AX_APPEND_ICV|B_AX_APPEND_MIC)
}

fn cmac(bus: &mut impl RegisterIo) -> io::Result<()> {
    field(bus,R_AX_PREBKF_CFG_1,B_AX_SIFS_MACTXEN_T1_MASK,0x47)?;
    modify(bus,R_AX_SCH_EXT_CTRL,0,B_AX_PORT_RST_TSF_ADV)?;
    modify(bus,R_AX_CCA_CFG_0,B_AX_BTCCA_EN,0)?;
    field(bus,R_AX_PREBKF_CFG_0,B_AX_PREBKF_TIME_MASK,0x18)?;
    let cam=io::read32(bus,R_AX_ADDR_CAM_CTRL)?;
    bus.write32(R_AX_ADDR_CAM_CTRL,cam|0x7f<<B_AX_ADDR_CAM_RANGE_MASK.trailing_zeros()|B_AX_ADDR_CAM_CLR|B_AX_ADDR_CAM_EN)?;
    io::poll32(bus,R_AX_ADDR_CAM_CTRL,B_AX_ADDR_CAM_CLR,0,1,2000)?;
    for at in [R_AX_MGNT_FLTR,R_AX_CTRL_FLTR,R_AX_DATA_FLTR] { write(bus,at,0x55555555)?; }
    let filter=B_AX_A_A1_MATCH|B_AX_A_BC|B_AX_A_MC|B_AX_A_UC_CAM_MATCH|B_AX_A_BC_CAM_MATCH|B_AX_A_PWR_MGNT|B_AX_A_FTM_REQ|3<<B_AX_UID_FILTER_MASK.trailing_zeros()|B_AX_A_BCN_CHK_EN;
    write(bus,R_AX_RX_FLTR_OPT,filter)?;
    let plcp=B_AX_CCK_CRC_CHK|B_AX_CCK_SIG_CHK|B_AX_LSIG_PARITY_CHK_EN|B_AX_SIGA_CRC_CHK|B_AX_VHT_SU_SIGB_CRC_CHK|B_AX_VHT_MU_SIGB_CRC_CHK|B_AX_HE_SIGB_CRC_CHK;
    bus.write16(R_AX_PLCP_HDR_FLTR,plcp as u16)?;
    let clear=B_AX_TB_CHK_TX_NAV|B_AX_TB_CHK_CCA_S80|B_AX_TB_CHK_CCA_S40|B_AX_TB_CHK_CCA_S20|B_AX_SIFS_CHK_CCA_S80|B_AX_SIFS_CHK_CCA_S40|B_AX_SIFS_CHK_CCA_S20|B_AX_CTN_CHK_TXNAV|B_AX_SIFS_CHK_EDCCA;
    let set=B_AX_TB_CHK_BASIC_NAV|B_AX_TB_CHK_BTCCA|B_AX_TB_CHK_EDCCA|B_AX_TB_CHK_CCA_P20|B_AX_SIFS_CHK_BTCCA|B_AX_SIFS_CHK_CCA_P20|B_AX_CTN_CHK_INTRA_NAV|B_AX_CTN_CHK_BASIC_NAV|B_AX_CTN_CHK_BTCCA|B_AX_CTN_CHK_EDCCA|B_AX_CTN_CHK_CCA_S80|B_AX_CTN_CHK_CCA_S40|B_AX_CTN_CHK_CCA_S20|B_AX_CTN_CHK_CCA_P20;
    modify(bus,R_AX_CCA_CONTROL,clear,set)?;
    modify(bus,R_AX_RSP_CHK_SIG,B_AX_RSP_CHK_TXNAV|B_AX_RSP_CHK_INTRA_NAV|B_AX_RSP_CHK_BASIC_NAV,0)?;
    modify(bus,R_AX_TRXPTCL_RESP_0,B_AX_RSP_CHK_SEC_CCA_80|B_AX_RSP_CHK_SEC_CCA_40|B_AX_RSP_CHK_SEC_CCA_20|B_AX_RSP_CHK_BTCCA|B_AX_RSP_CHK_EDCCA|B_AX_RSP_CHK_CCA,0)?;
    modify(bus,R_AX_WMAC_NAV_CTL,0,B_AX_WMAC_PLCP_UP_NAV_EN|B_AX_WMAC_TF_UP_NAV_EN|B_AX_WMAC_NAV_UPPER_EN)?;
    field(bus,R_AX_WMAC_NAV_CTL,B_AX_WMAC_NAV_UPPER_MASK,0xc4)?;
    modify8(bus,R_AX_RX_SR_CTRL,B_AX_SR_EN,0)?;
    modify8(bus,R_AX_BSSID_SRC_CTRL,0,B_AX_PLCP_SRC_EN)?;
    modify(bus,R_AX_MAC_LOOPBACK,B_AX_MACLBK_EN,0)?;
    field(bus,R_AX_TCR0,B_AX_TCR_UDF_THSD_MASK,6)?;
    field(bus,R_AX_TXD_FIFO_CTRL,B_AX_TXDFIFO_HIGH_MCS_THRE_MASK,7)?;
    field(bus,R_AX_TXD_FIFO_CTRL,B_AX_TXDFIFO_LOW_MCS_THRE_MASK,7)?;
    field(bus,R_AX_TRXPTCL_RESP_0,B_AX_WMAC_SPEC_SIFS_CCK_MASK,10)?;
    field(bus,R_AX_TRXPTCL_RESP_0,B_AX_WMAC_SPEC_SIFS_OFDM_MASK,17)?;
    modify(bus,R_AX_RXTRIG_TEST_USER_2,0,B_AX_RXTRIG_FCSCHK_EN)?;
    field(bus,R_AX_TRXPTCL_RRSR_CTL_0,B_AX_WMAC_RESP_REF_RATE_SEL,0)?;
    field(bus,R_AX_TRXPTCL_RRSR_CTL_0,B_AX_WMAC_RESP_RSC_MASK,2)?;
    let ba=io::read32(bus,R_AX_RESPBA_CAM_CTRL)?;
    bus.write32(R_AX_RESPBA_CAM_CTRL,(ba&!B_AX_BACAM_RST_MASK)|(S_AX_BACAM_RST_ALL<<B_AX_BACAM_RST_MASK.trailing_zeros()))?;
    io::poll32(bus,R_AX_RESPBA_CAM_CTRL,B_AX_BACAM_RST_MASK,0,1,1000)?;
    modify8(bus,R_AX_RESPBA_CAM_CTRL,0,B_AX_SSN_SEL)?;
    modify16(bus,R_AX_DLK_PROTECT_CTL,B_AX_RX_DLK_DATA_TIME_MASK|B_AX_RX_DLK_CCA_TIME_MASK,15<<B_AX_RX_DLK_DATA_TIME_MASK.trailing_zeros()|32<<B_AX_RX_DLK_CCA_TIME_MASK.trailing_zeros())?;
    modify8(bus,R_AX_RCR,B_AX_CH_EN_MASK,1<<B_AX_CH_EN_MASK.trailing_zeros())?;
    // The source limits MPDU length by the minimum RX quota: floor(89*128/512)=22 units.
    field(bus,R_AX_RX_FLTR_OPT,B_AX_RX_MPDU_MAX_LEN_MASK,22)?;
    modify8(bus,R_AX_PLCP_HDR_FLTR,B_AX_VHT_SU_SIGB_CRC_CHK,0)?;
    modify(bus,R_AX_TX_SUB_CARRIER_VALUE,B_AX_TXSC_20M_MASK|B_AX_TXSC_40M_MASK|B_AX_TXSC_80M_MASK,0)?;
    field(bus,R_AX_PTCL_RRSR1,B_AX_RRSR_RATE_EN_MASK,3)?;
    field(bus,R_AX_SIFS_SETTING,B_AX_HW_CTS2SELF_PKT_LEN_TH_MASK,S_AX_CTS2S_TH_1K)?;
    field(bus,R_AX_SIFS_SETTING,B_AX_HW_CTS2SELF_PKT_LEN_TH_TWW_MASK,S_AX_CTS2S_TH_SEC_256B)?;
    modify(bus,R_AX_SIFS_SETTING,0,B_AX_HW_CTS2SELF_EN)?;
    field(bus,R_AX_PTCL_FSM_MON,B_AX_PTCL_TX_ARB_TO_THR_MASK,S_AX_PTCL_TO_2MS)?;
    modify(bus,R_AX_PTCL_FSM_MON,B_AX_PTCL_TX_ARB_TO_MODE,0)?;
    modify8(bus,R_AX_PTCL_COMMON_SETTING_0,0,B_AX_CMAC_TX_MODE_0|B_AX_CMAC_TX_MODE_1)?;
    modify8(bus,R_AX_PTCL_COMMON_SETTING_0,B_AX_PTCL_TRIGGER_SS_EN_0|B_AX_PTCL_TRIGGER_SS_EN_1|B_AX_PTCL_TRIGGER_SS_EN_UL,0)?;
    modify8(bus,R_AX_PTCLRPT_FULL_HDL,B_AX_SPE_RPT_PATH_MASK,1<<B_AX_SPE_RPT_PATH_MASK.trailing_zeros())?;
    field(bus,R_AX_AGG_LEN_VHT_0,B_AX_AMPDU_MAX_LEN_VHT_MASK,0x3ff80)?;
    modify8(bus,R_AX_RXDMA_CTRL_0,B_AX_RU0_PTR_FULL_MODE|B_AX_RU1_PTR_FULL_MODE|B_AX_RU2_PTR_FULL_MODE|B_AX_RU3_PTR_FULL_MODE|B_AX_CSI_PTR_FULL_MODE|B_AX_RXSTS_PTR_FULL_MODE,0)
}

/// The same device must have completed the verified downloader; no other MMIO or firmware command writer may run.
/// Ordinary host DMA stays stopped; the sole register-mailbox writer must own its mirrored diagnostic counters.
/// This does not enable interrupts, create RX rings or configure a usable channel.
pub fn prepare(bus: &mut impl Bus, queue: &mut Queue<'_>, board: &Board, bar_bytes: usize, _firmware: &download::Ready) -> core::result::Result<Prepared,Failure> {
    prepare_inner(bus,queue,board,bar_bytes)
}
fn prepare_inner(bus: &mut impl Bus, queue: &mut Queue<'_>, board: &Board, bar_bytes: usize) -> core::result::Result<Prepared,Failure> {
    let fail=|cause| Failure{stage:Stage::Guard,cause,packet_stop:None,radio_stop:None,command_stop:None};
    guard(bus,queue,board,bar_bytes).map_err(fail)?;
    let mut stage=Stage::PacketStop;
    let result=(|| {
        let mut timed=TimedIo::new(bus,STAGE_US);
        packet_stop(&mut timed)?;
        stage=Stage::Baseband; enable_radio(&mut timed)?;
        stage=Stage::Functions; functions(&mut timed)?;
        stage=Stage::Memory; memory(&mut timed)?;
        stage=Stage::FlowControl; flow_control(&mut timed)?;
        stage=Stage::Station; station(&mut timed)?;
        stage=Stage::MpduSecurity; mpdu_security(&mut timed)?;
        stage=Stage::Cmac; cmac(&mut timed)?;
        stage=Stage::Verify;
        require(&mut timed,download::FW_CONTROL,0xe0,0xe0)?;
        require(&mut timed,R_AX_DMAC_FUNC_EN,0xfffd0000,0xfffd0000)?;
        require(&mut timed,mac::DMAC_CLOCK,0x0f9f0000,0x0f9f0000)?;
        require(&mut timed,fwcmd::DMA_STOP,0x001f0f03,PACKET_STOP)?;
        require(&mut timed,fwcmd::INIT_CONFIG,1<<11|1<<13,1<<11)?;
        require(&mut timed,R_AX_CTN_TXEN,0xffff,0)?;
        require(&mut timed,R_AX_PTCL_TX_CTN_SEL,B_AX_PTCL_TX_ON_STAT,0)?;
        timed.delay_us(0)
    })();
    if let Err(cause)=result {
        let packet_stop=packet_status(&mut TimedIo::new(bus,80_000)).err();
        let radio_stop=disable_radio(bus).err();
        let command_stop=Some(download::stop(bus,queue));
        return Err(Failure{stage,cause,packet_stop,radio_stop,command_stop});
    }
    Ok(Prepared{_private:()})
}

#[cfg(test)]
#[path = "../../tests/rtw89_runtime_mac_cases.rs"]
mod tests;
