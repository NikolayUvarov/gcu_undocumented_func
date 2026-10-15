// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! Actual AX firmware PHY capabilities; OTP calibration and channel/regulatory permission are separate.

use crate::{board::Board,download,fwcmd::{Bus,Queue},io::{self,Fault,FaultKind,RegisterIo,TimedIo},runtime_mac};

pub const MAX_TIMEOUT_US:u32=1_100_000;
const RESPONSE:usize=0x8150;

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub enum Stage{Guard,Mailbox,Decode}
#[derive(Debug)]
pub struct Failure{pub stage:Stage,pub cause:Fault,pub command_stop:Option<download::Stop>}

/// Constructed only by a successful response from this command queue's firmware session; not a radio-ready token.
#[derive(Debug)]
pub struct Capabilities{raw:[u32;4],session:u64}
impl Capabilities{
    pub fn raw(&self)->[u32;4]{self.raw}
    pub fn raw_rx_nss(&self)->u8{(self.raw[0]>>16)as u8}
    pub fn raw_tx_nss(&self)->u8{self.raw[1]as u8}
    pub fn raw_tx_antennas(&self)->u8{(self.raw[3]>>8)as u8}
    pub fn raw_rx_antennas(&self)->u8{(self.raw[3]>>16)as u8}
    pub fn tx_nss_defaulted(&self)->bool{self.raw_tx_nss()==0}
    pub fn rx_nss_defaulted(&self)->bool{self.raw_rx_nss()==0}
    pub fn tx_nss(&self)->u8{if self.tx_nss_defaulted(){2}else{self.raw_tx_nss().min(2)}}
    pub fn rx_nss(&self)->u8{if self.rx_nss_defaulted(){2}else{self.raw_rx_nss().min(2)}}
    pub fn tx_path_diversity(&self)->bool{self.raw_tx_nss()==1&&self.raw_tx_antennas()==2&&self.raw_rx_antennas()==2}
    pub fn antenna_tx(&self)->u8{if self.raw_tx_antennas()==1||self.tx_path_diversity(){2}else{0}}
    pub fn antenna_rx(&self)->u8{if self.raw_rx_antennas()==1{2}else{0}}
    pub fn effective_rx_path(&self)->u8{if self.antenna_rx()==0{3}else{self.antenna_rx()}}
    pub(crate) fn session(&self)->u64{self.session}
}

fn remaining(bus:&mut impl RegisterIo,start:u64,budget:u32)->io::Result<u64>{
    let elapsed=bus.now_us().checked_sub(start).ok_or(Fault::new(FaultKind::Clock,RESPONSE,0))?;
    u64::from(budget).checked_sub(elapsed).filter(|left|*left>0).ok_or(Fault::new(FaultKind::Timeout,RESPONSE,0))
}

/// Call once after download and before runtime MAC setup, with exclusive ownership of the same device and mailbox.
/// An error after guard requires a fresh firmware session; keep DMA until external mastering-off/quiescence is verified.
pub fn query(bus:&mut impl Bus,queue:&mut Queue<'_>,board:&Board,bar_bytes:usize,ready:&download::Ready,timeout_us:u32)->Result<Capabilities,Failure>{
    let fail=|cause|Failure{stage:Stage::Guard,cause,command_stop:None};
    if timeout_us==0||timeout_us>MAX_TIMEOUT_US||ready.session()!=queue.session(){
        return Err(fail(Fault::new(FaultKind::Access,RESPONSE,timeout_us)));
    }
    let start=bus.now_us();
    runtime_mac::guard(bus,queue,board,bar_bytes).map_err(fail)?;
    let budget=remaining(bus,start,timeout_us).map_err(fail)?;
    let mut stage=Stage::Mailbox;
    let result=(||{
        let mut timed=TimedIo::new(bus,budget);
        let raw=runtime_mac::exchange_register(&mut timed,[0x103,0,0,0],3)?;
        stage=Stage::Decode;
        if (raw[0]>>8)&15!=4{return Err(Fault::new(FaultKind::PowerState,RESPONSE,raw[0]));}
        timed.delay_us(0)?;Ok(raw)
    })().and_then(|raw|remaining(bus,start,timeout_us).map(|_|raw));
    match result{
        Ok(raw)=>Ok(Capabilities{raw,session:queue.session()}),
        Err(cause)=>Err(Failure{stage,cause,command_stop:Some(download::stop(bus,queue))}),
    }
}

#[cfg(test)]
#[path="../../tests/rtw89_phycap_cases.rs"]
mod tests;
