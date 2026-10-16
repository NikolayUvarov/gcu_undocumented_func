// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! Shared private coexistence operations; only actual startup and packet owners implement this transport.

use super::{Context,Error,Connection,SCOREBOARD,left};
use crate::{btc_session::{DeviceIo,Session,Window},btc_control::{self,Antenna},c2h,h2c,io,phy::PhyIo};

pub(crate) trait Transport {
    type Bus:DeviceIo;
    fn clock(&mut self)->Result<u64,Error>;
    fn event(&self)->Option<c2h::Event<'_>>;
    fn consume_event(&mut self);
    fn registers<T>(&mut self,deadline:u64,run:impl FnOnce(&mut Window<'_,Self::Bus>)->io::Result<T>)->Result<T,Error>;
    fn command_before(&mut self,command:h2c::Command,body:&[u8],acks:h2c::Acks,deadline:u64,context:&mut Context)->Result<h2c::Outcome,Error>;
    fn poll_before(&mut self,deadline:u64,context:&mut Context)->Result<[bool;2],Error>;
    fn validate_owner(&mut self,timeout_us:u32)->Result<(),Error>;
}
impl<B:DeviceIo> Transport for Session<'_,'_,'_,B>{
    type Bus=B;
    fn clock(&mut self)->Result<u64,Error>{Session::clock(self).map_err(Error::Session)}
    fn event(&self)->Option<c2h::Event<'_>>{Session::event(self)}
    fn consume_event(&mut self){Session::consume_event(self)}
    fn registers<T>(&mut self,deadline:u64,run:impl FnOnce(&mut Window<'_,B>)->io::Result<T>)->Result<T,Error>{Session::registers(self,deadline,run).map_err(Error::Session)}
    fn command_before(&mut self,command:h2c::Command,body:&[u8],acks:h2c::Acks,deadline:u64,_context:&mut Context)->Result<h2c::Outcome,Error>{Session::command_before(self,command,body,acks,deadline).map_err(Error::Session)}
    fn poll_before(&mut self,deadline:u64,_context:&mut Context)->Result<[bool;2],Error>{Session::poll_before(self,deadline).map_err(Error::Session)}
    fn validate_owner(&mut self,timeout_us:u32)->Result<(),Error>{self.receive_permit(timeout_us)?;Ok(())}
}

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) enum Progress{Stable,DeferredAntenna,DeferredEvents}
impl Context {
    pub(crate) fn runtime<T:Transport>(&mut self,transport:&mut T,attempt:u64,session:u64,deadline:u64)->Result<Progress,Error>{
        if attempt==0||session==0||self.attempt!=attempt||self.session!=session||self.rfk.is_some()
            ||self.connection!=Connection::Reported||self.policy!=crate::btc_codec::Policy::NoLink{return Err(Error::Order)}
        Self::radio_active(transport,deadline)?;self.events(transport,deadline)?;
        let raw=transport.registers(deadline,|b|io::read32(b,SCOREBOARD))?;
        self.dirty|=self.bt.update_score(raw);if self.bt.raw&4!=0{return Err(Error::Whql)}
        if !self.dirty{
            self.policy_consistent()?;let remaining=left(transport,deadline)?.min(10_000);
            transport.validate_owner(remaining)?;left(transport,deadline)?;return Ok(Progress::Stable)
        }
        self.dirty=false;
        let desired=if self.bt.state().enabled{Antenna::NoLink}else{Antenna::WlanOnly};
        let applied=transport.registers(deadline,|b|btc_control::antenna_runtime(b,desired,100_000))?;
        self.common(transport,false,deadline)?;
        // Runtime batches yield under continuous valid traffic; startup's empty-ring drain remains a separate contract.
        for _ in 0..8{self.events(transport,deadline)?;let received=transport.poll_before(deadline,self)?;self.events(transport,deadline)?;
            if received==[false,false]{break}}
        let raw=transport.registers(deadline,|b|io::read32(b,SCOREBOARD))?;
        self.dirty|=self.bt.update_score(raw);if self.bt.raw&4!=0{return Err(Error::Whql)}
        self.policy_consistent()?;let remaining=left(transport,deadline)?.min(10_000);
        transport.validate_owner(remaining)?;left(transport,deadline)?;
        let changed=self.dirty;self.dirty|=!applied;
        Ok(if !applied{Progress::DeferredAntenna}else if changed{Progress::DeferredEvents}else{Progress::Stable})
    }
}
