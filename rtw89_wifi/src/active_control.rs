//! Sealed active command access; the caller retains the sole RX dispatcher and the original absolute deadline.

use super::{Cause,Counts,DataSession,Dispatcher,Failure,Mode,Stage,radio};
use crate::{btc_session::{DeviceIo,Session,Window},h2c,io::{self,RegisterIo},rx,station,tx};

/// Only DataSession can create this view after live ownership and no-pending-TX admission.
pub(crate) struct FilterChange<'s,'memory,'a,'command,'receive,B:DeviceIo>{
    session:&'s mut Session<'a,'command,'receive,B>,queue:&'s mut tx::Queue<'memory>,counts:&'s mut Counts,
    attempt:u64,command:u64,channel:u8,policy:u64,board_mac:[u8;6],deadline:u64,
}
impl<B:DeviceIo> FilterChange<'_,'_,'_,'_,'_,B>{
    pub(crate) fn attempt(&self)->u64{self.attempt}
    pub(crate) fn session(&self)->u64{self.command}
    pub(crate) fn channel(&self)->u8{self.channel}
    pub(crate) fn policy_version(&self)->u64{self.policy}
    pub(crate) fn physical_board_mac(&self)->[u8;6]{self.board_mac}
    fn live(&mut self)->Result<(),Cause>{
        if self.queue.pending_request()||self.session.failed.is_some()||self.command!=self.session.command_session(){return Err(Cause::State)}
        let Mode::Data(layout)=self.session.mode else{return Err(Cause::State)};
        if !layout.matches(self.queue){return Err(Cause::Identity)}
        let mut window=Window{bus:self.session.bus,deadline:self.deadline,last:self.session.last,clock_bad:false};
        let result=(||{self.queue.validate_data(&mut window)?;radio(&mut window,self.channel)?;window.delay_us(0)?;Ok::<_,Cause>(())})();
        self.session.last=window.last;result?;
        let remaining=self.session.left(self.deadline)?;self.session.live(remaining.min(10_000))?;
        self.session.left(self.deadline)?;Ok(())
    }
    pub(crate) fn command_cam(&mut self,body:&[u8;60])->Result<(),Cause>{
        self.live()?;
        let mut handler=Dispatcher{queue:self.queue,filter:None,counts:self.counts,error:None};
        self.session.command_deadline_with(h2c::Command{category:1,class:6,function:0},body,
            h2c::Acks{received:false,done:true},self.deadline,&mut handler)
            .map_err(|e|handler.error.map_or(Cause::Session(e),Cause::Tx))?;
        self.session.left(self.deadline)?;Ok(())
    }
    pub(crate) fn registers<T>(&mut self,run:impl FnOnce(&mut Window<'_,B>)->io::Result<T>)->Result<T,Cause>{
        let mut window=Window{bus:self.session.bus,deadline:self.deadline,last:self.session.last,clock_bad:false};
        let result=run(&mut window).and_then(|v|{window.delay_us(0)?;Ok(v)});self.session.last=window.last;
        result.map_err(Cause::Register)
    }
    pub(crate) fn finish(&mut self)->Result<(),Cause>{
        for _ in 0..=rx::DESCRIPTORS*2{
            let mut handler=Dispatcher{queue:self.queue,filter:None,counts:self.counts,error:None};
            let progress=self.session.poll_before_with(self.deadline,&mut handler)
                .map_err(|e|handler.error.map_or(Cause::Session(e),Cause::Tx))?;
            if self.session.pending_packets()!=0{return Err(Cause::State)}
            if progress==[false,false]{return self.live()}
        }Err(Cause::State)
    }
}

impl<B:DeviceIo> DataSession<'_,'_,'_,'_,'_,'_,B>{
    /// The existing raw-six-byte wlanhw policy is retained; publication follows actual CAM completion and epoch drain.
    pub(crate) fn set_filter(&mut self,all_beacons:bool,bssid:[u8;6],own:[u8;6],timeout_us:u32)->Result<(),Failure>{
        Self::request(timeout_us)?;
        if self.queue.pending_request(){return Err(Failure{stage:Stage::Runtime,cause:Cause::Tx(tx::Error::Busy),shutdown:None})}
        let deadline=match self.deadline(timeout_us).and_then(|d|{self.live(d)?;self.drain_previous_epoch(d)?;Ok(d)}){
            Ok(deadline)=>deadline,Err(cause)=>return Err(self.runtime_failure(cause)),
        };
        let filter=self.prepared.station.filter(all_beacons,bssid,own);
        let mut update=FilterChange{session:&mut self.session,queue:self.queue,counts:&mut self.counts,
            attempt:self.prepared.tuned.attempt,command:self.prepared.command,channel:self.prepared.tuned.channel,
            policy:self.prepared.tuned.policy_version,board_mac:self.prepared.board.identity.mac,deadline};
        match self.prepared.station.reconfigure(&mut update,filter){
            Ok(())=>Ok(()),Err(station::FilterFailure{stage,cause,..})=>{
                let mut failure=self.runtime_failure(cause);failure.stage=Stage::Filter(stage);Err(failure)
            },
        }
    }
}
