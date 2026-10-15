// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! RTL8852B MAC tail and installed RX-channel activation; ordinary TX and host interrupts remain disabled.

use core::sync::atomic::{fence,Ordering};
use crate::{board::Board,download,fwcmd::{self,Bus,Queue},io::{self,Fault,FaultKind,RegisterIo,TimedIo},mac,runtime_mac,rx};
use crate::post_mac_regs::*;

const STOPPED:u32=0x000b0f03;
const RECEIVING:u32=STOPPED&!3;
const STOP_MASK:u32=0x001f0f03;
const HCI:u32=1<<11|1<<13;
const TIMEOUT_US:u64=10_000;
const HOST_MASKS:[usize;3]=[0x1a0,0x10b0,0x13b0];

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub enum Stage{Guard,MacMasks,HostReports,Pcie,Receive,Verify}
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub enum Error{Register(Fault),Command(fwcmd::Error),Receive(rx::Error)}
impl From<Fault> for Error{fn from(e:Fault)->Self{Self::Register(e)}}
impl From<fwcmd::Error> for Error{fn from(e:fwcmd::Error)->Self{Self::Command(e)}}
impl From<rx::Error> for Error{fn from(e:rx::Error)->Self{Self::Receive(e)}}
#[derive(Debug)]
pub struct Failure{pub stage:Stage,pub cause:Error,pub receive_stop:Option<rx::Stop>,pub radio_stop:Option<Fault>,pub command_stop:Option<download::Stop>}
#[derive(Debug)]
pub struct Receiving{_private:()}

fn require(bus:&mut impl RegisterIo,at:usize,mask:u32,expected:u32)->io::Result<()>{
    let value=bus.read32(at)?;
    if value&mask!=expected{return Err(Fault::new(FaultKind::PowerState,at,value));}Ok(())
}
fn modify(bus:&mut impl RegisterIo,at:usize,clear:u32,set:u32,status:u32)->io::Result<()>{
    let old=bus.read32(at)?;
    bus.write32(at,((old&!clear)|set)&!status)?;
    require(bus,at,clear|set,set)
}
fn write(bus:&mut impl RegisterIo,at:usize,value:u32)->io::Result<()>{bus.write32(at,value)?;require(bus,at,u32::MAX,value)}

// Known combined-register status/clear fields are never echoed while setting interrupt masks.
const MASKS:&[(usize,u32,u32,u32)]=&[
    (R_AX_WDRLS_ERR_IMR,B_AX_WDRLS_IMR_EN_CLR,B_AX_WDRLS_IMR_SET,0),
    (R_AX_SEC_DEBUG,0,B_AX_IMR_ERROR,0),
    (R_AX_MPDU_TX_ERR_IMR,B_AX_TX_GET_ERRPKTID_INT_EN|B_AX_TX_NXT_ERRPKTID_INT_EN|B_AX_TX_MPDU_SIZE_ZERO_INT_EN|B_AX_TX_OFFSET_ERR_INT_EN|B_AX_TX_HDR3_SIZE_ERR_INT_EN,0,0),
    (R_AX_MPDU_RX_ERR_IMR,B_AX_GETPKTID_ERR_INT_EN|B_AX_MHDRLEN_ERR_INT_EN|B_AX_RPT_ERR_INT_EN,0,0),
    (R_AX_STA_SCHEDULER_ERR_IMR,B_AX_SEARCH_HANG_TIMEOUT_INT_EN|B_AX_RPT_HANG_TIMEOUT_INT_EN|B_AX_PLE_B_PKTID_ERR_INT_EN,B_AX_STA_SCHEDULER_IMR_SET,0),
    (R_AX_TXPKTCTL_ERR_IMR_ISR,B_AX_TXPKTCTL_IMR_B0_CLR,B_AX_TXPKTCTL_IMR_B0_SET,0x030f0000),
    (R_AX_TXPKTCTL_ERR_IMR_ISR_B1,B_AX_TXPKTCTL_IMR_B1_CLR,B_AX_TXPKTCTL_IMR_B1_SET,0x030f0000),
    (R_AX_WDE_ERR_IMR,B_AX_WDE_IMR_CLR,B_AX_WDE_IMR_SET,0),
    (R_AX_PLE_ERR_IMR,B_AX_PLE_IMR_CLR,B_AX_PLE_IMR_SET,0),
    (R_AX_PKTIN_ERR_IMR,0,B_AX_PKTIN_GETPKTID_ERR_INT_EN,0),
    (R_AX_HOST_DISPATCHER_ERR_IMR,B_AX_HOST_DISP_IMR_CLR,B_AX_HOST_DISP_IMR_SET,0),
    (R_AX_CPU_DISPATCHER_ERR_IMR,B_AX_CPU_DISP_IMR_CLR,B_AX_CPU_DISP_IMR_SET,0),
    (R_AX_OTHER_DISPATCHER_ERR_IMR,B_AX_OTHER_DISP_IMR_CLR,0,0),
    (R_AX_CPUIO_ERR_IMR,B_AX_CPUIO_IMR_CLR,B_AX_CPUIO_IMR_SET,0),
    (R_AX_BBRPT_COM_ERR_IMR_ISR,0,B_AX_BBRPT_COM_NULL_PLPKTID_ERR_INT_EN,1<<16),
    (R_AX_BBRPT_CHINFO_ERR_IMR_ISR,B_AX_BBRPT_CHINFO_IMR_CLR,0,0xff0000),
    (R_AX_BBRPT_DFS_ERR_IMR_ISR,0,B_AX_BBRPT_DFS_TO_ERR_INT_EN,1<<16),
    (R_AX_LA_ERRFLAG,0,B_AX_LA_IMR_DATA_LOSS_ERR,1<<16),
    (R_AX_SCHEDULE_ERR_IMR,B_AX_SORT_NON_IDLE_ERR_INT_EN|B_AX_FSM_TIMEOUT_ERR_INT_EN,B_AX_FSM_TIMEOUT_ERR_INT_EN,0),
    (R_AX_PTCL_IMR0,u32::MAX,B_AX_PTCL_IMR_SET,0),
    (R_AX_DLE_CTRL,B_AX_DLE_IMR_CLR,B_AX_DLE_IMR_SET,0),
    (R_AX_RMAC_ERR_ISR,B_AX_RMAC_IMR_CLR,B_AX_RMAC_IMR_SET,255),
    (R_AX_TMAC_ERR_IMR_ISR,B_AX_TMAC_IMR_CLR,B_AX_TMAC_IMR_SET,0xf0000),
];

fn guard(bus:&mut impl Bus,queue:&mut Queue<'_>,rings:&rx::Rings<'_>,board:&Board,bar_bytes:usize)->Result<(),Error>{
    if bar_bytes<runtime_mac::BAR_BYTES||board.identity.cut!=1||board.identity.rfe!=1||board.maps.phycap[0x6c..0x6e]!=[0xff,0xff]{
        return Err(Fault::new(FaultKind::PowerState,0,0).into());
    }
    let command=bus.pci_command()?;
    if command==u16::MAX||command&6!=6{return Err(Fault::new(FaultKind::Access,4,command as u32).into());}
    require(bus,download::FW_CONTROL,0xe0,0xe0)?;
    require(bus,0xf0,0xf000,1<<12)?;require(bus,0x3f0,0x300,1<<8)?;
    require(bus,0x88,3,3)?;require(bus,0x8400,0xfffd0000,0xfffd0000)?;
    require(bus,0x8404,0x0f9f0000,0x0f9f0000)?;require(bus,0xc000,0xf000003f,0xf000003f)?;
    require(bus,0xc004,0x4000003f,0x4000003f)?;require(bus,0xe000,1<<30,0)?;
    require(bus,0xc348,0xffff,0)?;require(bus,0xc6ec,1<<7,0)?;
    require(bus,mac::WDE_CONFIG,0x1fff3f03,510<<16)?;
    require(bus,mac::PLE_CONFIG,0x1fff3f03,496<<16|4<<8|1)?;
    require(bus,mac::HFC_CONTROL,0xfff,0x19)?;
    require(bus,fwcmd::INIT_CONFIG,HCI|1<<18,1<<11)?;
    require(bus,fwcmd::DMA_STOP,STOP_MASK,STOPPED)?;
    require(bus,fwcmd::DMA_BUSY,fwcmd::BUSY_MASK,0)?;
    for at in HOST_MASKS{require(bus,at,u32::MAX,0)?;}
    require(bus,fwcmd::BASE_LOW,u32::MAX,queue.physical() as u32)?;require(bus,fwcmd::BASE_HIGH,u32::MAX,0)?;
    require(bus,fwcmd::BDRAM,u32::MAX,28|4<<8|1<<16)?;
    let count=bus.read16(fwcmd::NUMBER)?;
    if count!=32{return Err(Fault::new(FaultKind::PowerState,fwcmd::NUMBER,count as u32).into());}
    queue.reclaim(bus)?;
    if queue.pending()!=0{return Err(fwcmd::Error::State.into());}
    rings.installed(bus)?;
    Ok(())
}
fn host_reports(bus:&mut impl RegisterIo)->io::Result<()>{
    modify(bus,R_AX_WDRLS_CFG,B_AX_WDRLS_MODE_MASK,0,0)?;
    modify(bus,R_AX_RLSRPT0_CFG0,0,B_AX_RLSRPT0_FLTR_MAP_MASK,0)?;
    modify(bus,R_AX_RLSRPT0_CFG1,B_AX_RLSRPT0_AGGNUM_MASK,30,0)?;
    modify(bus,R_AX_RLSRPT0_CFG1,B_AX_RLSRPT0_TO_MASK,255<<16,0)
}
fn pcie(bus:&mut impl RegisterIo)->io::Result<()>{
    for at in [0x8410,0x8414,0x8418,0x841c]{
        let value=io::read32(bus,at)?;
        if value==u32::MAX||value==0xeaeaeaea{return Err(Fault::new(FaultKind::Access,at,value));}
    }
    modify(bus,0x8410,0x3700,0x2743,0)?;
    modify(bus,0x8414,0x0fff0fff,0x00280028,0)?;
    write(bus,0x8418,0x90039003)?;write(bus,0x841c,0x880b880b)?;
    modify(bus,0x8810,0,1,0)?;modify(bus,0x9a00,2,0,0)
}

/// Activate only installed receive rings after runtime preparation; keep host IRQs and all data TX stopped.
/// Caller retains both disjoint DMA allocations until externally verified PCI quiescence, including errors.
/// This returns receive-DMA readiness, not normal H2C/BTC/PHY completion or WLAN readiness.
pub fn activate(bus:&mut impl Bus,queue:&mut Queue<'_>,rings:&mut rx::Rings<'_>,board:&Board,bar_bytes:usize,_prepared:&runtime_mac::Prepared)->Result<Receiving,Failure>{
    activate_inner(bus,queue,rings,board,bar_bytes)
}
fn activate_inner(bus:&mut impl Bus,queue:&mut Queue<'_>,rings:&mut rx::Rings<'_>,board:&Board,bar_bytes:usize)->Result<Receiving,Failure>{
    let fail=|cause|Failure{stage:Stage::Guard,cause,receive_stop:None,radio_stop:None,command_stop:None};
    let started=bus.now_us();
    guard(bus,queue,rings,board,bar_bytes).map_err(fail)?;
    let budget=remaining(bus,started).map_err(|cause|fail(cause.into()))?;
    let mut stage=Stage::MacMasks;
    let result=(||->Result<(),Error>{
        let mut timed=TimedIo::new(bus,budget);
        for &(at,clear,set,status) in MASKS{modify(&mut timed,at,clear,set,status)?;}
        write(&mut timed,R_AX_DMAC_ERR_IMR,u32::MAX)?;write(&mut timed,R_AX_CMAC_ERR_IMR,u32::MAX)?;
        stage=Stage::HostReports;host_reports(&mut timed)?;
        // RTL8852B uses BACAM_V0, so the source mac_feat_init sends no BA-CAM H2C.
        stage=Stage::Pcie;pcie(&mut timed)?;
        rings.installed(&mut timed)?;
        stage=Stage::Receive;fence(Ordering::Release);
        modify(&mut timed,fwcmd::INIT_CONFIG,0,1<<13,0)?;
        modify(&mut timed,fwcmd::DMA_STOP,3,0,0)?;
        stage=Stage::Verify;
        require(&mut timed,fwcmd::DMA_STOP,STOP_MASK,RECEIVING)?;
        require(&mut timed,fwcmd::INIT_CONFIG,HCI|1<<18,HCI)?;
        require(&mut timed,0xc348,0xffff,0)?;require(&mut timed,0xc6ec,1<<7,0)?;
        for at in HOST_MASKS{require(&mut timed,at,u32::MAX,0)?;}
        timed.delay_us(0)?;
        Ok(())
    })();
    let result=result.and_then(|()|unsafe{rings.start(bus)}.map_err(Error::Receive)).and_then(|()|remaining(bus,started).map(|_|()).map_err(Error::Register));
    if let Err(cause)=result{
        let receive_stop=Some(rings.stop(bus));
        let radio_stop=runtime_mac::disable_radio(bus).err();
        let command_stop=Some(download::stop(bus,queue));
        return Err(Failure{stage,cause,receive_stop,radio_stop,command_stop});
    }
    Ok(Receiving{_private:()})
}

fn remaining(bus:&mut impl RegisterIo,started:u64)->io::Result<u64>{
    let elapsed=bus.now_us().checked_sub(started).ok_or(Fault::new(FaultKind::Clock,0,0))?;
    TIMEOUT_US.checked_sub(elapsed).filter(|v|*v>0).ok_or(Fault::new(FaultKind::Timeout,0,0))
}

#[cfg(test)]
#[path="../../tests/rtw89_post_mac_cases.rs"]mod tests;
