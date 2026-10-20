// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2024 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! Private post-RXDCK initial power reference, board trim and receive-path configuration.

use super::{Phy,PhyIo,State,RegisterPhase,PHY_BASE,guard,rf_read,rf_write_masked,disable_bb_rf,dm};
use crate::{board::Board,calibration::{self,Calibration},dack::Band,io::{self,Fault,FaultKind,RegisterIo,TimedIo},phycap::Capabilities};

const BUDGET_US:u64=100_000;

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) enum Subband{Ghz2,Ghz5Low,Ghz5Middle,Ghz5High}
impl Subband{
    fn band(self)->Band{if self==Self::Ghz2{Band::Ghz2}else{Band::Ghz5}}
    fn comp(self)->usize{match self{Self::Ghz2=>0,Self::Ghz5Low=>1,Self::Ghz5Middle=>3,Self::Ghz5High=>4}}
    fn gain(self)->usize{match self{Self::Ghz2=>1,Self::Ghz5Low=>2,Self::Ghz5Middle=>3,Self::Ghz5High=>4}}
}
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) enum Stage{Guard,PowerReference,ThermalTrim,PaTrim,ReceivePath,GainOffset,Btg,PowerReset,RadioMode,Streams,MacMode,Verify}
#[derive(Debug)]
pub(crate) struct Failure{pub stage:Stage,pub cause:Fault,pub radio_stop:Option<Fault>}
#[derive(Debug)]
pub(crate) struct Configured{pub attempt:u64,pub session:u64,pub rx_path:u8,pub rx_nss:u8}

fn fault(at:usize,value:u32)->Fault{Fault::new(FaultKind::PowerState,at,value)}
fn remaining(bus:&mut impl RegisterIo,start:u64)->io::Result<u64>{
    let elapsed=bus.now_us().checked_sub(start).ok_or(Fault::new(FaultKind::Clock,0,0))?;
    BUDGET_US.checked_sub(elapsed).filter(|n|*n>0).ok_or(Fault::new(FaultKind::Timeout,0,0))
}
fn field(bus:&mut impl RegisterIo,at:usize,mask:u32,value:u32)->io::Result<()>{
    let bits=(value<<mask.trailing_zeros())&mask;
    io::modify32(bus,at,mask,bits)?;
    let actual=io::read32(bus,at)?;
    if actual&mask!=bits{return Err(fault(at,actual));}Ok(())
}
fn bb(bus:&mut impl RegisterIo,at:usize,mask:u32,value:u32)->io::Result<()>{field(bus,PHY_BASE+at,mask,value)}
fn rf(bus:&mut impl RegisterIo,path:u8,at:u32,mask:u32,value:u32)->io::Result<()>{
    rf_write_masked(bus,path,at,mask,value)?;
    let actual=rf_read(bus,path,at,mask)?;
    if actual!=value{return Err(fault(at as usize,actual));}Ok(())
}
fn signed(value:i32)->u32{value.clamp(-128,127)as i8 as u8 as u32}
fn power_reference(bus:&mut impl RegisterIo)->io::Result<()>{
    // Source startup reference is zero with equal path power; this is not a regulatory power limit.
    field(bus,0xd200,0x0ffffc00,0)?;
    let value=(0xacu32<<18)|(0x138<<9);
    for at in [0x5804,0x7804,0x5808,0x7808]{bb(bus,at,0x07ffffff,value)?;}Ok(())
}
fn thermal(bus:&mut impl RegisterIo,cal:&Calibration)->io::Result<()>{
    if let Some(values)=cal.thermal_setting{for(path,value)in values.into_iter().enumerate(){rf(bus,path as u8,0x43,0xf0000,u32::from(value))?;}}Ok(())
}
fn pa_bias(bus:&mut impl RegisterIo,cal:&Calibration)->io::Result<()>{
    if let Some(values)=cal.pa_bias_setting{for(path,values)in values.into_iter().enumerate(){
        rf(bus,path as u8,0x60,0xf000,u32::from(values[0]))?;rf(bus,path as u8,0x60,0xf0000,u32::from(values[1]))?;
    }}Ok(())
}
fn stream_limits(bus:&mut impl RegisterIo,nss:u8,with_user:bool)->io::Result<()>{
    let encoded=u32::from(nss-1);
    bb(bus,0xd18,0x300,encoded)?;bb(bus,0xd18,0x00600000,encoded)?;
    if with_user{bb(bus,0xd80,0x3fc0,4)?;}
    bb(bus,0xd80,0x1c000,encoded)?;bb(bus,0xd80,0x03800000,encoded)
}
pub(super) fn receive_path(bus:&mut impl RegisterIo,path:u8)->io::Result<()>{
    bb(bus,0x49c4,15,u32::from(path))?;bb(bus,0x49c0,0x0003c000,u32::from(path))?;
    bb(bus,0x49c0,0x003c0000,u32::from(path))?;
    stream_limits(bus,if path==3{2}else{1},true)
}
pub(super) fn gain_offset(bus:&mut impl RegisterIo,cal:&Calibration,prefix:&dm::Prefix,subband:Subband,path:u8)->io::Result<()>{
    if cal.gain_comp_valid{for(p,at)in [0x4acc,0x4ad8].into_iter().enumerate(){bb(bus,at,255,signed(i32::from(cal.gain_comp[p][subband.comp()])*4))?;}}
    if !cal.gain_offset_valid{return Ok(());}
    let base=i32::from(prefix.gain_offset);let rssi=i32::from(prefix.gain_rssi);
    for(p,at)in [0x4694,0x4778].into_iter().enumerate(){
        let offset=-i32::from(cal.gain_offset[p][subband.gain()]);bb(bus,at,0x00ff0000,signed(-(offset*4+(base>>2))))?;
    }
    let selected=usize::from(path==2);let ofdm=-i32::from(cal.gain_offset[selected][subband.gain()]);
    bb(bus,0x49b0,255,signed(ofdm*16+base))?;bb(bus,0x4a00,255,signed(ofdm*16+rssi))?;
    if subband==Subband::Ghz2{
        let cck=-i32::from(cal.gain_offset[selected][0]);let value=(cck*8+(base>>1)).clamp(-64,63);
        bb(bus,0x23ac,127,value as i8 as u8 as u32)?;
    }Ok(())
}
pub(super) fn btg(bus:&mut impl RegisterIo,enabled:bool)->io::Result<()>{
    let on=u32::from(enabled);
    for(at,mask,value)in [(0x4738,1<<19,on),(0x4738,1<<22,0),
        (0x476c,0xff000000,if enabled{0x20}else{0x1a}),(0x4778,255,if enabled{0x30}else{0x2a}),
        (0x4aa4,1<<19,on),(0x4aa4,1<<22,on),(0x980,0x001e0000,if enabled{0}else{12}),
        (0x49c4,1<<14,on),(0x49c0,0x03c00000,if enabled{2}else{0}),(0x4420,1<<31,1),(0xc6c,1<<21,on)]{
        bb(bus,at,mask,value)?;
    }Ok(())
}
fn radio_modes(bus:&mut impl RegisterIo,path:u8)->io::Result<()>{
    for(p,(mode,ftm))in [(0x12ac,0x12b0),(0x32ac,0x32b0)].into_iter().enumerate(){
        let active=path&(1<<p)!=0;bb(bus,mode,0xfffffff0,if active{0x1233312}else{0x1111111})?;
        bb(bus,ftm,0xfff,if active{0x333}else{0x111})?;
    }Ok(())
}

/// Private initial DM tail only; actual BTC initialization and all source RFK interleaves must already be complete.
/// Neither a successful tail nor its subband selection authorizes channel tuning, packet TX or association.
pub(crate) fn run<B:PhyIo>(phy:&mut Phy<'_,B>,board:&Board,prefix:&dm::Prefix,caps:&Capabilities,subband:Subband)->Result<Configured,Failure>{
    run_inner(phy,board,prefix,caps.session(),caps.effective_rx_path(),caps.rx_nss(),subband)
}
fn run_inner<B:PhyIo>(phy:&mut Phy<'_,B>,board:&Board,prefix:&dm::Prefix,session:u64,path:u8,nss:u8,subband:Subband)->Result<Configured,Failure>{
    let fail=|cause|Failure{stage:Stage::Guard,cause,radio_stop:None};
    let attempt=phy.attempt();
    if phy.state!=State::RxdckDone||attempt==0||prefix.attempt()!=attempt||prefix.band!=subband.band()
        ||board.identity!=phy.identity||session==0||phy.bus.command_session()!=session||!(1..=3).contains(&path)||!(1..=2).contains(&nss){
        return Err(fail(fault(0,phy.state as u32)));
    }
    let cal=calibration::decode(board).map_err(fail)?;
    let start=phy.bus.now_us();
    let budget=guard(phy.bus,&phy.identity,RegisterPhase::AfterDack).and_then(|()|remaining(phy.bus,start));
    let budget=budget.map_err(|cause|{phy.mark_failed();fail(cause)})?;
    let nss=if path==3{nss}else{1};let mut stage=Stage::PowerReference;
    let result=(||{
        let mut timed=TimedIo::new(phy.bus,budget);power_reference(&mut timed)?;
        stage=Stage::ThermalTrim;thermal(&mut timed,&cal)?;stage=Stage::PaTrim;pa_bias(&mut timed,&cal)?;
        stage=Stage::ReceivePath;receive_path(&mut timed,path)?;stage=Stage::GainOffset;gain_offset(&mut timed,&cal,prefix,subband,path)?;
        stage=Stage::Btg;btg(&mut timed,subband==Subband::Ghz2&&path!=1)?;
        stage=Stage::PowerReset;let reset=if path==1{0x58dc}else{0x78dc};
        bb(&mut timed,reset,0xc0000000,1)?;bb(&mut timed,reset,0xc0000000,3)?;
        stage=Stage::RadioMode;radio_modes(&mut timed,path)?;stage=Stage::Streams;stream_limits(&mut timed,nss,false)?;
        stage=Stage::MacMode;bb(&mut timed,0x9a4,0x1c,0)?;stage=Stage::Verify;timed.delay_us(0)?;
        Ok(())
    })().and_then(|()|remaining(phy.bus,start).map(|_|()));
    result.map_err(|cause|{phy.mark_failed();Failure{stage,cause,radio_stop:disable_bb_rf(phy.bus).err()}})?;
    phy.state=State::PathsConfigured;
    Ok(Configured{attempt,session,rx_path:path,rx_nss:nss})
}
