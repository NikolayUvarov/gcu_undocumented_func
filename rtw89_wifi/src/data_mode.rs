// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! Owned initial packet mode; startup PHY permits remain invalid after this consuming transition.

use super::{DeviceIo,Session,Window,inbox};
use core::sync::atomic::{fence,Ordering};
use crate::{board::Board,btc_context::Initialized,channel_rfk,fwcmd,h2c,
    io::{self,Fault,FaultKind,RegisterIo},phy::{Phy,State,channel},runtime_mac,rx,rxdesc,station,tx,txdesc};

const ACTIVATE_US:u64=1_000_000;
const LIVE_US:u32=10_000;
pub(crate) const SCHEDULER:u16=0x010f;
pub(crate) const STOPPED_UNUSED:u32=1<<17;
const OWN_DMA:u32=0x0001_0f00|1<<19;

#[derive(Clone,Copy,Debug)]
pub(super) enum Mode{Startup,Data(Layout)}
#[derive(Clone,Copy,Debug)]
pub(crate) struct Layout{command:u64,receive:u64,transmit:u64,physical:u64}
impl Layout{
    pub(crate) fn matches(&self,queue:&tx::Queue<'_>)->bool{self.transmit==queue.session()&&self.physical==queue.physical()}
    pub(crate) fn matches_receive(&self,command:u64,receive:u64)->bool{self.command==command&&self.receive==receive}
    pub(crate) fn check_registers(&self,bus:&mut impl RegisterIo)->io::Result<()>{tx::check_layout(bus,self.physical)}
}

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) enum Stage{Guard,Station,Dma,Scheduler,Verify,Runtime,Filter(station::Stage)}
#[derive(Debug)]
pub(crate) enum Cause{Coexistence(crate::btc_context::Error),Identity,State,Register(Fault),Session(super::Error),Station(station::CheckFailure),Tx(tx::Error)}
impl From<Fault> for Cause{fn from(v:Fault)->Self{Self::Register(v)}}
impl From<super::Error> for Cause{fn from(v:super::Error)->Self{Self::Session(v)}}
impl From<tx::Error> for Cause{fn from(v:tx::Error)->Self{Self::Tx(v)}}
#[derive(Clone,Copy,Debug)]
pub(crate) enum Shutdown{NotActivated,Attempted{radio:Option<Fault>,transmit:tx::Stop}}
#[derive(Debug)]
pub(crate) struct Failure{pub stage:Stage,pub cause:Cause,pub shutdown:Option<Shutdown>}

/// Actual station context is consumed; the calibrated policy and board remain immutably borrowed throughout data mode.
pub(crate) struct PreparedActivation<'p>{board:&'p Board,tuned:&'p channel::Tuned,calibrated:&'p channel_rfk::Completed,
    owner:&'p Initialized,station:station::Prepared,command:u64,transmit:u64,physical:u64}

pub(crate) fn prepare<'p,B:DeviceIo>(phy:&mut Phy<'_,Session<'_,'_,'_,B>>,board:&'p Board,tuned:&'p channel::Tuned,
    calibrated:&'p channel_rfk::Completed,owner:&'p Initialized,station:station::Prepared,queue:&tx::Queue<'_>)
    ->Result<PreparedActivation<'p>,station::Failure>{
    let command=phy.bus_between_stages().command_session();
    if phy.state()!=State::ChannelTuned||phy.attempt()!=tuned.attempt||command!=tuned.session
        ||phy.identity()!=board.identity||tuned.access!=channel::Access::InitiateRadiation||!calibrated.matches(tuned)
        ||!owner.matches(tuned.attempt,command)||!station.matches(tuned,board.identity.mac)
        ||station.current_filter().is_none()||queue.state()!=tx::State::Installed||queue.session()==0||!phy.bus_between_stages().retains_packets(){
        return Err(station::Failure{stage:station::Stage::Guard,cause:station::Cause::Register(Fault::new(FaultKind::PowerState,0,0)),
            radio_stop:None,port_stop:None,selector_restore:None});
    }
    station.validate(phy,board,tuned,owner,LIVE_US)?;
    Ok(PreparedActivation{board,tuned,calibrated,owner,station,command,transmit:queue.session(),physical:queue.physical()})
}

/// Consumes the startup owner; forgetting this wrapper cannot yield an active startup Session or PHY interface.
/// The TX region remains exclusively borrowed and no failure, shutdown or Drop releases any DMA allocation.
pub(crate) struct DataSession<'q,'memory,'p,'a,'command,'receive,B:DeviceIo>{
    session:Session<'a,'command,'receive,B>,queue:&'q mut tx::Queue<'memory>,prepared:PreparedActivation<'p>,stopped:bool,
    cleanup_allowed:bool,last_shutdown:Option<Shutdown>,counts:Counts,tx_deadline:Option<u64>,
}
pub(crate) struct ActivationFailure<'q,'memory,'p,'a,'command,'receive,B:DeviceIo>{
    pub owner:DataSession<'q,'memory,'p,'a,'command,'receive,B>,pub failure:Failure,
}

#[derive(Clone,Copy,Debug,Default,Eq,PartialEq)]
pub(crate) struct Counts{pub filtered:u64,pub invalid:u64,pub pressure:u64,pub previous_epoch:u64,pub btc_events:u64,pub btc_deferred:u64,pub ppdu_reports:u64}
struct Dispatcher<'q,'memory>{queue:&'q mut tx::Queue<'memory>,filter:Option<station::Filter>,counts:&'q mut Counts,error:Option<tx::Error>}
impl super::PacketHandler for Dispatcher<'_,'_>{
    fn packet(&mut self,now:u64,channel:rx::Channel,packet:rxdesc::Packet,buffer:&[u8],full:bool)->Result<bool,super::Error>{
        if channel==rx::Channel::Rxq&&packet.kind==rxdesc::TYPE_WIFI{
            let Some(filter)=self.filter else{self.counts.previous_epoch=self.counts.previous_epoch.saturating_add(1);return Ok(true)};
            let frame=match crate::frames::from_packet(packet,buffer){Ok(frame)=>frame,Err(_)=>{
                self.counts.invalid=self.counts.invalid.saturating_add(1);return Ok(true)}};
            if !filter.accepts(frame.mpdu()){self.counts.filtered=self.counts.filtered.saturating_add(1);return Ok(true)}
            if full{self.counts.pressure=self.counts.pressure.saturating_add(1);return Ok(true)}
            return Ok(false)
        }
        let is_report=channel==rx::Channel::Rpq||packet.kind==rxdesc::TYPE_C2H&&{
            let bytes=packet.bytes(buffer).ok_or(super::Error::Malformed)?;
            let event=crate::c2h::parse(bytes).map_err(super::Error::Event)?;
            (event.category,event.class,event.function)==(1,9,1)
        };
        if !is_report{return Ok(false)}
        match self.queue.ingest(now,channel,rx::Outcome::Packet(packet),buffer){
            Ok(tx::Receive::Rpq|tx::Receive::TxReport{..})=>Ok(true),
            Ok(_)=>Err(super::Error::Malformed),
            Err(error)=>{self.error=Some(error);Err(super::Error::PacketHandler)},
        }
    }
}

impl<'q,'memory,'p,'a,'command,'receive,B:DeviceIo> DataSession<'q,'memory,'p,'a,'command,'receive,B>{
    pub(crate) fn activate(session:Session<'a,'command,'receive,B>,queue:&'q mut tx::Queue<'memory>,prepared:PreparedActivation<'p>)
        ->Result<Self,ActivationFailure<'q,'memory,'p,'a,'command,'receive,B>>{
        let mut own=Self{session,queue,prepared,stopped:false,cleanup_allowed:false,last_shutdown:None,counts:Counts::default(),tx_deadline:None};
        if let Err(failure)=own.activate_inner(){return Err(ActivationFailure{owner:own,failure})}Ok(own)
    }
    fn identity(&self)->bool{
        let p=&self.prepared;
        p.command==self.session.command_session()&&p.command==p.tuned.session&&p.transmit==self.queue.session()
            &&p.physical==self.queue.physical()&&p.calibrated.matches(p.tuned)&&p.owner.matches(p.tuned.attempt,p.command)
            &&p.station.matches(p.tuned,p.board.identity.mac)&&p.station.current_filter().is_some()&&self.session.retains_packets()
    }
    fn activate_inner(&mut self)->Result<(),Failure>{
        let mut stage=Stage::Guard;let mut modified=false;
        let result=(||->Result<(),Cause>{
            if !self.identity()||!matches!(self.session.mode,Mode::Startup)||self.session.failed.is_some(){return Err(Cause::Identity)}
            let deadline=self.session.now()?.checked_add(ACTIVATE_US).ok_or(super::Error::Clock)?;
            let left=self.session.left(deadline)?;self.session.live(left.min(LIVE_US))?;
            self.drain_previous_epoch(deadline)?;
            let layout=Layout{command:self.session.queue.session(),receive:self.session.rings.session(),transmit:self.queue.session(),physical:self.queue.physical()};
            let mut window=Window{bus:self.session.bus,deadline,last:self.session.last,clock_bad:false};
            let outcome=(||->Result<(),Cause>{
                self.queue.installed_for_data(&mut window)?;
                self.cleanup_allowed=true;stage=Stage::Station;modified=true;
                self.prepared.station.check_registers(&mut window).map_err(Cause::Station)?;
                radio(&mut window,self.prepared.tuned.channel)?;
                require(&mut window,0xc348,0xffff,0)?;
                stage=Stage::Dma;modified=true;
                fence(Ordering::Release);
                io::modify32(&mut window,fwcmd::DMA_STOP,OWN_DMA,0)?;
                require(&mut window,fwcmd::DMA_STOP,0x001f0f03,STOPPED_UNUSED)?;
                stage=Stage::Scheduler;
                runtime_mac::exchange_register(&mut window,[0x205|u32::from(SCHEDULER)<<16,0xffff,0,0],4)?;
                require(&mut window,0xc348,0xffff,u32::from(SCHEDULER))?;
                self.queue.start_data(&mut window,self.session.rings,&layout)?;window.delay_us(0)?;Ok(())
            })();
            self.session.last=window.last;outcome?;
            self.session.mode=Mode::Data(layout);stage=Stage::Verify;
            let left=self.session.left(deadline)?;self.session.live(left.min(LIVE_US))?;
            self.enable_runtime_commands(deadline)?;self.session.left(deadline)?;Ok(())
        })();
        if let Err(cause)=result{
            let shutdown=modified.then(||self.shutdown());
            let _=self.session.fail::<()>(super::Error::Session);
            return Err(Failure{stage,cause,shutdown});
        }Ok(())
    }
    fn enable_runtime_commands(&mut self,deadline:u64)->Result<(),Cause>{
        let mut window=Window{bus:self.session.bus,deadline,last:self.session.last,clock_bad:false};
        let observed=(||{
            self.session.queue.reclaim(&mut window).map_err(super::Error::Dma)?;
            let busy=io::read32(&mut window,fwcmd::DMA_BUSY)?;window.delay_us(0)?;
            if self.session.queue.pending()!=0||busy&super::TX_BUSY!=0{return Err(Cause::State)}
            Ok(())
        })();self.session.last=window.last;observed?;
        self.session.left(deadline)?;
        self.session.engine.enable_serial(self.session.last).map_err(super::Error::Command)?;Ok(())
    }
    fn drain_previous_epoch(&mut self,deadline:u64)->Result<(),Cause>{
        while let Some(record)=self.session.packet(){
            if record.metadata().channel!=rx::Channel::Rxq||record.metadata().packet.kind!=rxdesc::TYPE_WIFI{return Err(Cause::State)}
            self.session.consume_packet();self.counts.previous_epoch=self.counts.previous_epoch.saturating_add(1);
        }
        for _ in 0..=rx::DESCRIPTORS*2{
            let mut handler=Dispatcher{queue:self.queue,filter:None,counts:&mut self.counts,error:None};
            let read=self.session.poll_before_with(deadline,&mut handler)
                .map_err(|e|handler.error.map_or(Cause::Session(e),Cause::Tx))?;
            if self.session.pending_packets()!=0{return Err(Cause::State)}
            if read==[false,false]{return Ok(())}
        }Err(Cause::State)
    }
    fn live(&mut self,deadline:u64)->Result<(),Cause>{
        if self.stopped||!self.identity()||!matches!(self.session.mode,Mode::Data(_)){return Err(Cause::State)}
        if let Some(error)=self.session.failed{return Err(Cause::Session(error))}
        let mut window=Window{bus:self.session.bus,deadline,last:self.session.last,clock_bad:false};
        let result=(||{self.queue.validate_data(&mut window)?;
            self.prepared.station.check_registers(&mut window).map_err(Cause::Station)?;
                radio(&mut window,self.prepared.tuned.channel)?;Ok::<_,Cause>(())})();
        self.session.last=window.last;result?;
        let left=self.session.left(deadline)?;self.session.live(left.min(LIVE_US))?;self.session.left(deadline)?;Ok(())
    }
    fn deadline(&mut self,timeout_us:u32)->Result<u64,Cause>{
        if timeout_us==0||timeout_us>super::MAX_COMMAND_US{return Err(Cause::Session(super::Error::Request))}
        let own=self.session.now()?.checked_add(u64::from(timeout_us)).ok_or(Cause::Session(super::Error::Clock))?;
        Ok(self.tx_deadline.map_or(own,|tx|tx.min(own)))
    }
    fn runtime_failure(&mut self,cause:Cause)->Failure{
        let cause=self.session.btc_event_error.map_or(cause,Cause::Coexistence);
        let shutdown=Some(self.shutdown());Failure{stage:Stage::Runtime,cause,shutdown}
    }
    fn request(timeout_us:u32)->Result<(),Failure>{
        if timeout_us==0||timeout_us>super::MAX_COMMAND_US{return Err(Failure{stage:Stage::Runtime,cause:Cause::Session(super::Error::Request),shutdown:None})}Ok(())
    }
    pub(crate) fn command(&mut self,command:h2c::Command,body:&[u8],acks:h2c::Acks,timeout_us:u32)->Result<h2c::Outcome,Failure>{
        Self::request(timeout_us)?;
        if body.len()>h2c::MAX_BODY{return Err(Failure{stage:Stage::Runtime,cause:Cause::Session(super::Error::Request),shutdown:None})}
        let result=(||{let deadline=self.deadline(timeout_us)?;self.live(deadline)?;self.reconcile(deadline)?;
            let mut handler=Dispatcher{queue:self.queue,filter:self.prepared.station.current_filter(),counts:&mut self.counts,error:None};
            let outcome=self.session.command_deadline_with(command,body,acks,deadline,&mut handler)
                .map_err(|e|handler.error.map_or(Cause::Session(e),Cause::Tx))?;
            self.reconcile(deadline)?;self.session.left(deadline)?;Ok(outcome)})();
        result.map_err(|cause|self.runtime_failure(cause))
    }
    pub(crate) fn poll_packets(&mut self,timeout_us:u32)->Result<[bool;2],Failure>{
        Self::request(timeout_us)?;
        let result=(||{let deadline=self.deadline(timeout_us)?;self.live(deadline)?;self.reconcile(deadline)?;
            let mut handler=Dispatcher{queue:self.queue,filter:self.prepared.station.current_filter(),counts:&mut self.counts,error:None};
            let outcome=self.session.poll_before_with(deadline,&mut handler)
                .map_err(|e|handler.error.map_or(Cause::Session(e),Cause::Tx))?;
            self.reconcile(deadline)?;self.session.left(deadline)?;Ok(outcome)})();
        result.map_err(|cause|self.runtime_failure(cause))
    }
    pub(crate) fn begin(&mut self,frame:&[u8],rate:u8,max_submissions:u8,timeout_us:u32)->Result<(),Failure>{
        Self::request(timeout_us)?;
        if self.queue.pending_request(){return Err(Failure{stage:Stage::Runtime,cause:Cause::Tx(tx::Error::Busy),shutdown:None})}
        if frame.get(10..16)!=self.prepared.station.current_filter().map(|f|f.own()).as_ref().map(|v|v.as_slice()){
            return Err(Failure{stage:Stage::Runtime,cause:Cause::Tx(tx::Error::Input(txdesc::Error::Address)),shutdown:None})}
        if let Err(e)=txdesc::frame(frame,rate,max_submissions){return Err(Failure{stage:Stage::Runtime,cause:Cause::Tx(tx::Error::Input(e)),shutdown:None})}
        let result=(||{let deadline=self.deadline(timeout_us)?;self.live(deadline)?;self.reconcile(deadline)?;let left=self.session.left(deadline)?;
            let mut window=Window{bus:self.session.bus,deadline,last:self.session.last,clock_bad:false};
            let result=self.queue.begin(&mut window,frame,rate,max_submissions,self.prepared.station.mac_id(),left);
            self.session.last=window.last;result?;self.session.left(deadline)?;self.tx_deadline=Some(deadline);Ok(())})();
        result.map_err(|cause|self.runtime_failure(cause))
    }
    pub(crate) fn advance(&mut self,timeout_us:u32)->Result<Option<tx::Completion>,Failure>{
        Self::request(timeout_us)?;
        let result=(||{let deadline=self.deadline(timeout_us)?;self.live(deadline)?;self.reconcile(deadline)?;if !self.queue.pending_request(){return Ok(None)}
            let mut handler=Dispatcher{queue:self.queue,filter:self.prepared.station.current_filter(),counts:&mut self.counts,error:None};
            self.session.poll_before_with(deadline,&mut handler).map_err(|e|handler.error.map_or(Cause::Session(e),Cause::Tx))?;
            self.reconcile(deadline)?;
            let mut window=Window{bus:self.session.bus,deadline,last:self.session.last,clock_bad:false};
            let result=self.queue.advance(&mut window);self.session.last=window.last;
            let outcome=result?;self.session.left(deadline)?;if outcome.is_some(){self.tx_deadline=None;}Ok(outcome)})();
        result.map_err(|cause|self.runtime_failure(cause))
    }
    pub(crate) fn channel(&self)->u8{self.prepared.tuned.channel}
    pub(crate) fn mac(&self)->[u8;6]{self.prepared.board.identity.mac}
    pub(crate) fn current_filter(&self)->station::Filter{self.prepared.station.current_filter().unwrap()}
    pub(crate) fn counts(&self)->Counts{Counts{btc_events:self.counts.btc_events.saturating_add(self.session.counts.btc_events),ppdu_reports:self.session.counts.ppdu_reports,..self.counts}}
    pub(crate) fn packet(&self)->Option<inbox::Record<'_>>{self.session.packet()}
    pub(crate) fn consume_packet(&mut self){self.session.consume_packet()}
    pub(crate) fn terminal_packet(&self)->Option<inbox::Record<'_>>{self.session.terminal_packet()}
    pub(crate) fn pending_packets(&self)->usize{self.session.pending_packets()}
    /// Independently stop radiation and owned packet DMA; caller still verifies kernel PCI quiescence and keeps all memory.
    pub(crate) fn shutdown(&mut self)->Shutdown{
        self.stopped=true;let _=self.session.fail::<()>(super::Error::Session);
        if let Some(previous)=self.last_shutdown{return previous}
        if !self.cleanup_allowed{self.last_shutdown=Some(Shutdown::NotActivated);return Shutdown::NotActivated}
        let radio=runtime_mac::disable_radio(self.session.bus).err();let transmit=self.queue.stop(self.session.bus);
        let result=Shutdown::Attempted{radio,transmit};self.last_shutdown=Some(result);result
    }
}
impl<B:DeviceIo> Drop for DataSession<'_,'_,'_,'_,'_,'_,B>{
    fn drop(&mut self){let _=self.session.fail::<()>(super::Error::Session);self.queue.poison();}
}
fn require(bus:&mut impl RegisterIo,at:usize,mask:u32,wanted:u32)->io::Result<()>{
    let actual=io::read32(bus,at)?;if actual&mask!=wanted{return Err(Fault::new(FaultKind::PowerState,at,actual))}Ok(())
}

impl<B:fwcmd::Bus> fwcmd::Bus for Window<'_,B>{
    fn pci_command(&mut self)->io::Result<u16>{self.check()?;let result=self.bus.pci_command()?;self.check()?;Ok(result)}
}

fn radio(bus:&mut impl RegisterIo,channel:u8)->io::Result<()>{
    use crate::registers::*;
    let bb=bus.read8(R_AX_SYS_FUNC_EN)?;
    if bb==u8::MAX||bb&3!=3{return Err(Fault::new(FaultKind::PowerState,R_AX_SYS_FUNC_EN,u32::from(bb)))}
    require(bus,R_AX_SPS_DIG_ON_CTRL0,B_AX_REG_ZCDC_H_MASK,1<<17)?;
    require(bus,R_AX_WLRF_CTRL,B_AX_AFC_AFEDIG,B_AX_AFC_AFEDIG)?;
    let cycle=bus.read8(0x8040)?;
    if cycle==u8::MAX||cycle&15!=15{return Err(Fault::new(FaultKind::PowerState,0x8040,u32::from(cycle)))}
    require(bus,crate::phy::PHY_BASE+0x734,0x00ff0000,u32::from(channel)<<16)?;
    for path in 0..2{for at in [0x18,0x10018]{
        let actual=crate::phy::rf_read(bus,path,at,0x3ffff)?;
        if actual!=0x1c00|u32::from(channel){return Err(Fault::new(FaultKind::PowerState,at as usize,actual))}
    }}Ok(())
}

#[path="active_control.rs"]mod control;
pub(crate) use control::FilterChange;

#[path="runtime_control.rs"]mod coexistence;
