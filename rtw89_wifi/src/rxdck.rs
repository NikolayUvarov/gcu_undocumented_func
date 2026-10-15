// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! Initial single-band RTL8852B RXDCK; channel-change/TSSI calibration and packet transmission remain separate.

use crate::dack::{self,Band,BtcNotice,Calibration,DackIo,RfkState};
use crate::io::{self,Fault,FaultKind,RegisterIo,TimedIo};
use crate::phy;

const TOTAL_US:u64=2_500_000;
const RF_MASK:u32=0xfffff;
const MODE_MASK:u32=0xf0000;

/// Delegate both phases to runtime_mac::scheduler_zero using the same exclusively owned register mailbox.
/// Success requires firmware acknowledgement and actual zero CTN/PTCL; any error forbids another exchange.
/// Initial startup retains both source is_tssi_mode flags false; no other firmware or RF calibration may run.
pub trait RxdckIo:DackIo { fn scheduler_zero(&mut self,timeout_us:u32)->io::Result<()>; }

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub enum Stage { Guard,BtcStart,SchedulerPause,WaitA,WaitB,PathA,PathB,SchedulerRestore,BtcStop }
#[derive(Debug)]
pub struct Failure {
    pub stage:Stage,pub cause:Fault,pub scheduler_restore:Option<Fault>,pub scheduler_status:Option<Fault>,
    pub rf_restore:[Option<Fault>;2],pub radio_stop:Option<Fault>,pub btc_stop:Option<Fault>,
}
impl Failure {
    pub(crate) fn guard(cause:Fault)->Self {Self{stage:Stage::Guard,cause,scheduler_restore:None,scheduler_status:None,rf_restore:[None;2],radio_stop:None,btc_stop:None}}
}
/// Source trigger/delay and restoration completed; this is not a measured RX calibration quality guarantee.
#[derive(Debug,Eq,PartialEq)]
pub struct Completed { pub restored_fine:[u8;2] }
#[derive(Clone,Copy)]
struct Saved { mode:u32,reserved:u32,fine:u32 }

fn notice(band:Band,state:RfkState)->BtcNotice {BtcNotice{calibration:Calibration::Rxdck,state,phy_map:0x10|((band as u8)<<6)}}
fn remaining(bus:&mut impl RegisterIo,start:u64)->io::Result<u32> {
    let elapsed=bus.now_us().checked_sub(start).ok_or(Fault::new(FaultKind::Clock,0,0))?;
    TOTAL_US.checked_sub(elapsed).filter(|left|*left!=0).map(|v|v as u32).ok_or(Fault::new(FaultKind::Timeout,0,0))
}
fn scheduler(bus:&mut impl RxdckIo,timeout_us:u32)->io::Result<()> {
    let start=bus.now_us();let result=bus.scheduler_zero(timeout_us);
    let elapsed=bus.now_us().checked_sub(start).ok_or(Fault::new(FaultKind::Clock,0,0))?;
    result?;
    if elapsed>=u64::from(timeout_us) {return Err(Fault::new(FaultKind::Timeout,0xc348,timeout_us));}
    Ok(())
}
fn status(bus:&mut impl RegisterIo)->io::Result<()> {
    let raw=io::read32(bus,0xc348)?;
    if raw&0xffff!=0 {return Err(Fault::new(FaultKind::PowerState,0xc348,raw));}
    io::poll32(bus,0xc6ec,1<<7,0,8,80_000)?;bus.delay_us(0)
}
fn wait_rx(bus:&mut impl RegisterIo,path:u8)->io::Result<()> {
    let mut timed=TimedIo::new(bus,5_000);
    for _ in 0..=2_500 {
        let mode=phy::rf_read(&mut timed,path,0,MODE_MASK)?;
        timed.delay_us(0)?;
        if mode!=2 {return Ok(());}
        timed.delay_us(2)?;
    }
    Err(Fault::new(FaultKind::Timeout,0,2))
}
fn verified(bus:&mut impl RegisterIo,path:u8,at:u32,mask:u32,value:u32)->io::Result<()> {
    phy::rf_write_masked(bus,path,at,mask,value)?;
    let actual=phy::rf_read(bus,path,at,mask)?;
    if actual!=value {return Err(Fault::new(FaultKind::PowerState,at as usize,actual));}
    bus.delay_us(0)
}
fn calibrate(bus:&mut impl RegisterIo,path:u8,saved:Saved)->io::Result<()> {
    phy::rf_write_masked(bus,path,5,1,0)?;
    phy::rf_write_masked(bus,path,0x92,2,0)?;
    phy::rf_write_masked(bus,path,0,MODE_MASK,3)?;
    phy::rf_write_masked(bus,path,0x93,15,0)?;
    phy::rf_write_masked(bus,path,0x92,1,0)?;
    phy::rf_write_masked(bus,path,0x92,1,1)?;
    bus.delay_us(1_000)?;
    verified(bus,path,0x92,2,saved.fine)?;
    verified(bus,path,5,RF_MASK,saved.reserved)
}

pub(crate) fn run(bus:&mut impl RxdckIo,band:Band,start:u64)->core::result::Result<Completed,Failure> {
    let mut stage=Stage::BtcStart;let mut btc_started=false;let mut btc_stopped=false;let mut btc_error=None;
    let mut paused=false;let mut restored=false;let mut scheduler_error=None;
    let mut saved=[None;2];let mut modified=[false;2];
    let result=(|| {
        let left=remaining(bus,start)?;btc_started=true;
        dack::notify(bus,notice(band,RfkState::Start),left.min(100_000))?;
        stage=Stage::SchedulerPause;
        let left=remaining(bus,start)?;scheduler(bus,left.min(1_000_000))?;paused=true;
        let left=remaining(bus,start)?;let mut timed=TimedIo::new(bus,u64::from(left));status(&mut timed)?;
        for path in 0..2 {stage=if path==0 {Stage::WaitA}else{Stage::WaitB};wait_rx(&mut timed,path)?;}
        for path in 0..2 {
            stage=if path==0 {Stage::PathA}else{Stage::PathB};
            let value=Saved{reserved:phy::rf_read(&mut timed,path,5,RF_MASK)?,fine:phy::rf_read(&mut timed,path,0x92,2)?,mode:phy::rf_read(&mut timed,path,0,RF_MASK)?};
            saved[path as usize]=Some(value);modified[path as usize]=true;calibrate(&mut timed,path,value)?;
        }
        timed.delay_us(0)?;
        stage=Stage::SchedulerRestore;
        let left=remaining(bus,start)?;restored=true;
        if let Err(error)=scheduler(bus,left.min(1_000_000)) {scheduler_error=Some(error);return Err(error);}
        let left=remaining(bus,start)?;status(&mut TimedIo::new(bus,u64::from(left)))?;
        stage=Stage::BtcStop;
        let left=remaining(bus,start)?;btc_stopped=true;
        if let Err(error)=dack::notify(bus,notice(band,RfkState::Stop),left.min(100_000)) {btc_error=Some(error);return Err(error);}
        remaining(bus,start)?;
        Ok(Completed{restored_fine:[saved[0].unwrap().fine as u8,saved[1].unwrap().fine as u8]})
    })();
    match result {
        Ok(done)=>Ok(done),
        Err(cause)=>{
            let mut rf_restore=[None;2];
            {
                let mut cleanup=TimedIo::new(bus,1_000);
                for path in 0..2 {
                    if !modified[path] {continue;}
                    if let Some(saved)=saved[path] {
                        for (at,mask,value) in [(0,RF_MASK,saved.mode),(0x92,2,saved.fine),(5,RF_MASK,saved.reserved)] {
                            if let Err(error)=verified(&mut cleanup,path as u8,at,mask,value) {if rf_restore[path].is_none(){rf_restore[path]=Some(error);}}
                        }
                    }
                }
            }
            let scheduler_restore=if paused&&!restored {scheduler(bus,1_000_000).err()} else {scheduler_error};
            let scheduler_status=if btc_started {status(&mut TimedIo::new(bus,80_000)).err()} else {None};
            let radio_stop=if btc_started {phy::disable_bb_rf(bus).err()} else {None};
            let btc_stop=if btc_started&&!btc_stopped {dack::notify(bus,notice(band,RfkState::Stop),100_000).err()} else {btc_error};
            Err(Failure{stage,cause,scheduler_restore,scheduler_status,rf_restore,radio_stop,btc_stop})
        }
    }
}
