// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022, 2024 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! Private AX MAC report tail; complete BTC/PHY/RFK/trim/path readiness remains a caller prerequisite.

use crate::{download, fwcmd::{Bus, Queue}, io::{self, Fault, FaultKind, RegisterIo, TimedIo}, post_mac, runtime_mac, rx};

const PPDU:usize=0xce40;
const FORWARD:usize=0x9c18;
const RTS:usize=0xc614;
const BUDGET_US:u64=10_000;

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) enum Stage{Guard,Ppdu,Forward,Rts,Verify}
#[derive(Debug)]
pub(crate) struct Failure{
    pub stage:Stage,pub cause:post_mac::Error,pub report_stop:Option<Fault>,
    pub receive_stop:Option<rx::Stop>,pub radio_stop:Option<Fault>,pub command_stop:Option<download::Stop>,
}
#[derive(Debug)]
pub(crate) struct Configured{pub command_session:u64,pub receive_session:u64,pub encoded_rts:u16}

fn remaining(bus:&mut impl RegisterIo,start:u64)->io::Result<u64>{
    let elapsed=bus.now_us().checked_sub(start).ok_or(Fault::new(FaultKind::Clock,0,0))?;
    BUDGET_US.checked_sub(elapsed).filter(|n|*n>0).ok_or(Fault::new(FaultKind::Timeout,0,0))
}
fn require(bus:&mut impl RegisterIo,at:usize,mask:u32,expected:u32)->io::Result<()>{
    let actual=io::read32(bus,at)?;
    if actual&mask!=expected{return Err(Fault::new(FaultKind::PowerState,at,actual));}Ok(())
}
fn field16(bus:&mut impl RegisterIo,mask:u16,value:u16)->io::Result<()>{
    let old=bus.read16(RTS)?;bus.write16(RTS,(old&!mask)|value)?;
    let actual=bus.read16(RTS)?;
    if actual&mask!=value{return Err(Fault::new(FaultKind::PowerState,RTS,actual as u32));}Ok(())
}
fn thresholds(requested:Option<u32>)->u16{
    let (time,length)=match requested{None|Some(u32::MAX)=>(88,4080),Some(bytes)=>(255<<5,bytes)};
    ((time>>5).min(255)as u16)<<8|(length>>4).min(255)as u16
}

/// Only the completed source PHY/BTC/RFK/trim/path pipeline may call this; a permit alone is not that readiness.
/// Same-device exclusive ownership and retained DMA remain mandatory; this neither enables TX nor exposes a radio.
pub(crate) fn configure(bus:&mut impl Bus,queue:&mut Queue<'_>,rings:&mut rx::Rings<'_>,receiving:&post_mac::Receiving,
    requested_rts:Option<u32>)->Result<Configured,Failure>{
    let fail=|cause|Failure{stage:Stage::Guard,cause,report_stop:None,receive_stop:None,radio_stop:None,command_stop:None};
    let start=bus.now_us();
    {let _permit=receiving.validate(bus,queue,rings,BUDGET_US as u32).map_err(fail)?;}
    let guard=(||{
        let budget=remaining(bus,start)?;let mut timed=TimedIo::new(bus,budget);
        require(&mut timed,0x200,3<<17,1<<17)?;require(&mut timed,0x2f0,1<<17,1<<17)?;
        let bb=timed.read8(2)?;let cycle=timed.read8(0x8040)?;
        if bb&3!=3||cycle&15!=15||bb==255||cycle==255{return Err(Fault::new(FaultKind::PowerState,0x8040,cycle as u32));}
        timed.delay_us(0)
    })();
    guard.map_err(|cause|fail(cause.into()))?;
    let encoded=thresholds(requested_rts);let mut stage=Stage::Ppdu;
    let result=(||{
        let budget=remaining(bus,start)?;let mut timed=TimedIo::new(bus,budget);
        timed.write32(PPDU,0x2b)?;require(&mut timed,PPDU,u32::MAX,0x2b)?;
        stage=Stage::Forward;io::modify32(&mut timed,FORWARD,3,1)?;require(&mut timed,FORWARD,3,1)?;
        stage=Stage::Rts;field16(&mut timed,0xff00,encoded&0xff00)?;field16(&mut timed,255,encoded&255)?;
        stage=Stage::Verify;require(&mut timed,PPDU,u32::MAX,0x2b)?;
        require(&mut timed,FORWARD,3,1)?;
        let actual=timed.read16(RTS)?;
        if actual!=encoded{return Err(Fault::new(FaultKind::PowerState,RTS,actual as u32));}
        timed.delay_us(0)?;Ok::<(),Fault>(())
    })().and_then(|()|remaining(bus,start).map(|_|()));
    if let Err(cause)=result{
        let report_stop=(||{
            let mut timed=TimedIo::new(bus,1000);io::modify32(&mut timed,PPDU,1,0)?;
            require(&mut timed,PPDU,1,0)?;timed.delay_us(0)
        })().err();
        let receive_stop=Some(rings.stop(bus));let radio_stop=runtime_mac::disable_radio(bus).err();
        let command_stop=Some(download::stop(bus,queue));
        return Err(Failure{stage,cause:cause.into(),report_stop,receive_stop,radio_stop,command_stop});
    }
    Ok(Configured{command_session:queue.session(),receive_session:rings.session(),encoded_rts:encoded})
}
