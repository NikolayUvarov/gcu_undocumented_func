// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! Concrete packet-mode coexistence transport; all commands and data share the retained RX reader.

use super::{Cause,Counts,DataSession,Dispatcher,Mode,radio};
use crate::btc_session::PacketHandler;
use crate::{btc_context::{self,runtime::{Transport,Progress}},btc_session::{DeviceIo,Session,Window},c2h,h2c,io::{self,RegisterIo},station,tx};

struct Observe<'s,'q,'memory>{inner:Dispatcher<'q,'memory>,context:&'s mut btc_context::Context}
impl PacketHandler for Observe<'_,'_,'_>{
    fn btc_event(&mut self,bytes:&[u8],score:Option<u32>)->Result<bool,btc_context::Error>{
        self.context.observe(bytes)?;if let Some(raw)=score{self.context.observe_score(raw);}Ok(true)
    }
    fn packet(&mut self,now:u64,channel:crate::rx::Channel,packet:crate::rxdesc::Packet,buffer:&[u8],full:bool)->Result<bool,crate::btc_session::Error>{
        self.inner.packet(now,channel,packet,buffer,full)
    }
}

struct Runtime<'s,'memory,'a,'command,'receive,B:DeviceIo>{
    session:&'s mut Session<'a,'command,'receive,B>,queue:&'s mut tx::Queue<'memory>,counts:&'s mut Counts,
    filter:station::Filter,channel:u8,deadline:u64,error:Option<tx::Error>,
}
impl<B:DeviceIo> Runtime<'_,'_,'_,'_,'_,B>{
    fn live(&mut self,timeout_us:u32)->Result<(),btc_context::Error>{
        if self.session.failed.is_some(){return Err(btc_context::Error::Session(self.session.failed.unwrap()))}
        let Mode::Data(layout)=self.session.mode else{return Err(btc_context::Error::Order)};
        if !layout.matches(self.queue){return Err(btc_context::Error::Order)}
        let mut window=Window{bus:self.session.bus,deadline:self.deadline,last:self.session.last,clock_bad:false};
        let result=(||{self.queue.validate_data(&mut window).map_err(|error|{self.error=Some(error);btc_context::Error::Transmit})?;
            radio(&mut window,self.channel)?;window.delay_us(0)?;Ok::<_,btc_context::Error>(())})();
        self.session.last=window.last;result?;
        let remaining=self.session.left(self.deadline)?.min(timeout_us).min(10_000);
        self.session.live(remaining)?;self.session.left(self.deadline)?;Ok(())
    }
}
impl<B:DeviceIo> Transport for Runtime<'_,'_,'_,'_,'_,B>{
    type Bus=B;
    fn clock(&mut self)->Result<u64,btc_context::Error>{Ok(self.session.clock()?)}
    fn event(&self)->Option<c2h::Event<'_>>{self.session.event()}
    fn consume_event(&mut self){self.session.consume_event();self.counts.btc_events=self.counts.btc_events.saturating_add(1)}
    fn registers<T>(&mut self,deadline:u64,run:impl FnOnce(&mut Window<'_,B>)->io::Result<T>)->Result<T,btc_context::Error>{
        Ok(self.session.registers(deadline.min(self.deadline),run)?)
    }
    fn command_before(&mut self,command:h2c::Command,body:&[u8],acks:h2c::Acks,deadline:u64,context:&mut btc_context::Context)->Result<h2c::Outcome,btc_context::Error>{
        self.live(10_000)?;
        let mut handler=Observe{inner:Dispatcher{queue:self.queue,filter:Some(self.filter),counts:self.counts,error:None},context};
        let result=self.session.command_deadline_with(command,body,acks,deadline.min(self.deadline),&mut handler);
        self.error=handler.inner.error;result.map_err(|e|if self.error.is_some(){btc_context::Error::Transmit}else{self.session.btc_event_error.unwrap_or(btc_context::Error::Session(e))})
    }
    fn poll_before(&mut self,deadline:u64,context:&mut btc_context::Context)->Result<[bool;2],btc_context::Error>{
        let mut handler=Observe{inner:Dispatcher{queue:self.queue,filter:Some(self.filter),counts:self.counts,error:None},context};
        let result=self.session.poll_before_with(deadline.min(self.deadline),&mut handler);
        self.error=handler.inner.error;result.map_err(|e|if self.error.is_some(){btc_context::Error::Transmit}else{self.session.btc_event_error.unwrap_or(btc_context::Error::Session(e))})
    }
    fn validate_owner(&mut self,timeout_us:u32)->Result<(),btc_context::Error>{self.live(timeout_us)}
}
impl<B:DeviceIo> DataSession<'_,'_,'_,'_,'_,'_,B>{
    pub(super) fn reconcile(&mut self,deadline:u64)->Result<Progress,Cause>{
        let filter=self.prepared.station.current_filter().ok_or(Cause::State)?;
        let mut context=self.session.btc.take().ok_or(Cause::State)?;
        let mut transport=Runtime{session:&mut self.session,queue:self.queue,counts:&mut self.counts,
            filter,channel:self.prepared.tuned.channel,deadline,error:None};
        let result=context.runtime(&mut transport,self.prepared.tuned.attempt,self.prepared.command,deadline);
        let transmit=transport.error;self.session.btc=Some(context);
        if result.is_ok_and(|v|v!=Progress::Stable){self.counts.btc_deferred=self.counts.btc_deferred.saturating_add(1)}
        result.map_err(|cause|transmit.map_or(Cause::Coexistence(cause),Cause::Tx))
    }
}
