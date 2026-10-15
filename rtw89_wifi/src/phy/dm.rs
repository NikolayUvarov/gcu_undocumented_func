// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022, 2024 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! Initial single-band DM prefix from pinned phy.c and the RTL8852B common-chip dispatch.

use super::{Phy, PhyIo, State, RegisterPhase, PHY_BASE, guard, rf_read, rf_write_masked, disable_bb_rf};
use crate::{dack::Band, io::{self, Fault, FaultKind, RegisterIo, TimedIo}, power, registers::*};

const BUDGET_US:u64=300_000;

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) enum Stage{Guard,ThermalA,ThermalB,Baseband,Environment,PhyStatus,Dig,Crystal,Dcfo,Edcca,UlTb}
#[derive(Debug,Eq,PartialEq)]
pub(crate) struct Failure{pub stage:Stage,pub cause:Fault,pub radio_stop:Option<Fault>}

#[derive(Debug,Eq,PartialEq)]
pub(crate) struct Dig{
    pub gaincode:[u8;3],pub force_gaincode:[u8;3],pub force_gaincode_index:bool,
    pub rssi_thresholds:[u8;5],pub false_alarm_thresholds:[u16;4],pub igi_rssi:u8,
    pub dynamic_igi:[u8;2],pub dynamic_pd:bool,pub dynamic_pd_max:u8,pub pd_low_offset:u8,
}
#[derive(Debug,Eq,PartialEq)]
pub(crate) struct Cfo{
    pub crystal:u8,pub default_crystal:u8,pub bounds:[u8;2],pub offset:i8,
    pub timer_ms:u16,pub timer_enabled:bool,pub average_mode:u8,pub ofdma_accumulate:bool,
}
/// Initial observations/settings, not periodic tracking, calibrated PHY state or permission to transmit.
#[derive(Debug,Eq,PartialEq)]
pub(crate) struct Prefix{
    pub thermal:[u8;2],pub thermal_ewma_fixed4:[u16;2],pub last_thermal_max:u8,
    pub gain_offset:i8,pub gain_rssi:i8,pub ccx_unit:u8,pub ifs_low:[u16;4],pub ifs_high:[u16;4],
    pub dig:Dig,pub cfo:Cfo,pub edcca_backup:[u8;4],pub dynamic_tb:bool,pub bandedge:u8,
}

fn field(bus:&mut impl RegisterIo,at:usize,mask:u32,value:u32)->io::Result<()>{
    io::modify32(bus,at,mask,(value<<mask.trailing_zeros())&mask)
}
fn bb(bus:&mut impl RegisterIo,at:usize,mask:u32,value:u32)->io::Result<()>{field(bus,PHY_BASE+at,mask,value)}
fn read_bb(bus:&mut impl RegisterIo,at:usize,mask:u32)->io::Result<u32>{
    Ok((io::read32(bus,PHY_BASE+at)?&mask)>>mask.trailing_zeros())
}
fn remaining(bus:&mut impl RegisterIo,start:u64)->io::Result<u64>{
    let elapsed=bus.now_us().checked_sub(start).ok_or(Fault::new(FaultKind::Clock,0,0))?;
    BUDGET_US.checked_sub(elapsed).filter(|left|*left>0).ok_or(Fault::new(FaultKind::Timeout,0,0))
}
fn thermal(bus:&mut impl RegisterIo,path:u8)->io::Result<u8>{
    for value in [1,0,1]{rf_write_masked(bus,path,0x42,1<<19,value)?;}
    bus.delay_us(200)?;
    rf_read(bus,path,0x42,0x7e).map(|value|value as u8)
}
fn baseband(bus:&mut impl RegisterIo)->io::Result<(i8,i8)>{
    bb(bus,0xd7c,2,0)?;bb(bus,0x2d7c,2,0)?;
    // CMAC0 TX-power registers use MAC addresses, not the MCU-offset PHY window.
    for at in (0xd36c..=0xd568).step_by(4){bus.write32(at,0)?;}
    Ok((read_bb(bus,0x49b0,255)?as u8 as i8,read_bb(bus,0x4a00,255)?as u8 as i8))
}
fn environment(bus:&mut impl RegisterIo)->io::Result<([u16;4],[u16;4])>{
    for (mask,value) in [(1,1),(2,1),(4,1),(0x70,0)]{bb(bus,0xc00,mask,value)?;}
    // Source 64 us first bin, factor four, measured in 4 us << RTW89_CCX_32_US (3).
    let mut low=[0u16;4];let mut high=[0u16;4];let mut us=64;
    for i in 0..4{high[i]=(us>>5)as u16;if i>0{low[i]=high[i-1]+1;}us*=4;}
    for (i,&value) in low.iter().enumerate(){bb(bus,0xc2c+4*i,0x7fff,value as u32)?;}
    for (i,&value) in high.iter().enumerate(){bb(bus,0xc2c+4*i,0xffff0000,value as u32)?;}
    bb(bus,0xc28,1<<12,1)?;
    for i in 0..4{bb(bus,0xc2c+4*i,1<<15,1)?;}
    Ok((low,high))
}
fn physts(bus:&mut impl RegisterIo)->io::Result<()>{
    bb(bus,0x738,1<<3,1)?;bb(bus,0x738,1<<2,1)?;
    for page in 0..16{
        if page==9{continue;}
        let index=if page>9{page-1}else{page};let at=PHY_BASE+0x73c+index*4;
        let mut value=io::read32(bus,at)?;
        match page{
            6|7=>value|=1<<13,
            10=>value|=1<<13|1<<1,
            11..=15=>{value&=!0xf0;if page==11{value|=2;}else if page>=13{value|=1<<20;}},
            _=>{},
        }
        bus.write32(at,value)?;
    }
    Ok(())
}
fn dig(bus:&mut impl RegisterIo,band:Band)->io::Result<Dig>{
    // Initial zeroed IGI RSSI makes CCK max(0-(16+7+bandwidth_offset), -18)-110 equal -128.
    bb(bus,0x4860,0x7c0,0)?;bb(bus,0x4860,1<<30,0)?;
    bb(bus,0x4b74,1<<30,0)?;bb(bus,0x4b64,0xff000000,0x80)?;
    for at in [0x46e8,0x46ec,0x47a8,0x47ac]{bb(bus,at,1<<5,0)?;}
    Ok(Dig{gaincode:[6,1,31],force_gaincode:[6,1,31],force_gaincode_index:band==Band::Ghz5,
        rssi_thresholds:[68,84,90,98,104],false_alarm_thresholds:[196,352,440,528],igi_rssi:0,
        dynamic_igi:[0x20,0x5a],dynamic_pd:true,dynamic_pd_max:0,pd_low_offset:16})
}
fn idle_xtal(bus:&mut impl RegisterIo)->io::Result<()>{
    io::poll32(bus,R_AX_WLAN_XTAL_SI_CTRL,B_AX_WL_XTAL_SI_CMD_POLL,0,50,50_000).map(|_|())
}
fn crystal(bus:&mut impl RegisterIo,raw:u8)->io::Result<Cfo>{
    let value=raw&0x7f;
    idle_xtal(bus)?;
    power::xtal_write(bus,5,value,255)?;power::xtal_write(bus,4,value,255)?;
    let xo=power::xtal_read(bus,5)?;let xi=power::xtal_read(bus,4)?;
    if xo!=value||xi!=value{return Err(Fault::new(FaultKind::Xtal,4,(xo as u32)<<8|xi as u32));}
    Ok(Cfo{crystal:xi,default_crystal:value,bounds:[value.saturating_sub(64).max(1),value.saturating_add(64).min(127)],
        offset:0,timer_ms:2000,timer_enabled:false,average_mode:2,ofdma_accumulate:true})
}

/// Private engine only: integration must supply real BTC ntfy_init readiness before exposing this operation.
/// This is the initial TSSI-off, unassociated, PHY0-only prefix; periodic DM tracking is separate.
pub(crate) fn run<B:PhyIo>(phy:&mut Phy<'_,B>,band:Band)->Result<Prefix,Failure>{
    let fail=|cause|Failure{stage:Stage::Guard,cause,radio_stop:None};
    if phy.state!=State::RadioLoaded{return Err(fail(Fault::new(FaultKind::PowerState,0,phy.state as u32)));}
    if phy.identity.xtal==255{return Err(fail(Fault::new(FaultKind::Xtal,crate::efuse::XTAL_AT,255)));}
    let start=phy.bus.now_us();
    let budget=guard(phy.bus,&phy.identity,RegisterPhase::BeforeDack).and_then(|()|remaining(phy.bus,start));
    let budget=budget.map_err(|cause|{phy.mark_failed();fail(cause)})?;
    let mut stage=Stage::ThermalA;
    let result=(||{
        let mut timed=TimedIo::new(phy.bus,budget);
        let a=thermal(&mut timed,0)?;stage=Stage::ThermalB;let b=thermal(&mut timed,1)?;
        stage=Stage::Baseband;let (gain_offset,gain_rssi)=baseband(&mut timed)?;
        stage=Stage::Environment;let (ifs_low,ifs_high)=environment(&mut timed)?;
        stage=Stage::PhyStatus;physts(&mut timed)?;
        stage=Stage::Dig;let dig=dig(&mut timed,band)?;
        stage=Stage::Crystal;let cfo=crystal(&mut timed,phy.identity.xtal)?;
        stage=Stage::Dcfo;bb(&mut timed,0x4494,1<<29,1)?;bb(&mut timed,0x4490,0x0f000000,8)?;
        field(&mut timed,0xd248,7,6)?;
        stage=Stage::Edcca;bb(&mut timed,0xc70,0x03f00000,0x29)?;
        stage=Stage::UlTb;let bandedge=read_bb(&mut timed,0x4498,1<<30)?as u8;
        timed.delay_us(0)?;
        Ok(Prefix{thermal:[a,b],thermal_ewma_fixed4:[u16::from(a)<<4,u16::from(b)<<4],last_thermal_max:a.max(b),
            gain_offset,gain_rssi,ccx_unit:3,ifs_low,ifs_high,dig,cfo,edcca_backup:[0;4],dynamic_tb:true,bandedge})
    })().and_then(|value|remaining(phy.bus,start).map(|_|value));
    result.map_err(|cause|{phy.mark_failed();Failure{stage,cause,radio_stop:disable_bb_rf(phy.bus).err()}})
}
