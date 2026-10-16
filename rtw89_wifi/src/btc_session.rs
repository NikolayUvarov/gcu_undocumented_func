//! Exclusive H2C/RX ownership; command completion does not grant coexistence or RF calibration.

use crate::{c2h,download,fwcmd,h2c,io::{self,Fault,FaultKind,RegisterIo},phy::PhyIo,post_mac,rx,rxdesc};

#[path="rx_inbox.rs"] pub mod inbox;
pub use inbox::Inbox;
#[allow(dead_code)]
#[path="data_mode.rs"] pub(crate) mod data_mode;
#[allow(dead_code)]
#[path="connection.rs"] pub(crate) mod connection;

pub const EVENT_BYTES:usize=2048;
pub const EVENT_SLOTS:usize=4;
pub const MAX_COMMAND_US:u32=100_000;
const TX_BUSY:u32=1<<18|1<<21;

// Keep the caller's absolute deadline through nested helpers and CPU payload copies before a doorbell.
pub(crate) struct Window<'a,B>{bus:&'a mut B,deadline:u64,last:u64,clock_bad:bool}
impl<B:RegisterIo> Window<'_,B>{
    fn check(&mut self)->io::Result<u32>{
        let now=self.now_us();
        if self.clock_bad{return Err(Fault::new(FaultKind::Clock,0,0))}
        self.deadline.checked_sub(now).filter(|v|*v>0).map(|v|v.min(u32::MAX as u64)as u32).ok_or(Fault::new(FaultKind::Timeout,0,0))
    }
}
impl<B:RegisterIo> RegisterIo for Window<'_,B>{
    fn read8(&mut self,a:usize)->io::Result<u8>{self.check()?;self.bus.read8(a)}
    fn read16(&mut self,a:usize)->io::Result<u16>{self.check()?;self.bus.read16(a)}
    fn read32(&mut self,a:usize)->io::Result<u32>{self.check()?;self.bus.read32(a)}
    fn write8(&mut self,a:usize,v:u8)->io::Result<()>{self.check()?;self.bus.write8(a,v)}
    fn write16(&mut self,a:usize,v:u16)->io::Result<()>{self.check()?;self.bus.write16(a,v)}
    fn write32(&mut self,a:usize,v:u32)->io::Result<()>{self.check()?;self.bus.write32(a,v)}
    fn now_us(&mut self)->u64{let now=self.bus.now_us();self.clock_bad|=now<self.last;self.last=self.last.max(now);now}
    fn delay_us(&mut self,us:u32)->io::Result<()>{if self.check()?<us{return Err(Fault::new(FaultKind::Timeout,0,0))}self.bus.delay_us(us)?;self.check().map(|_|())}
}

/// PCI configuration and the mapped BAR belong to the same exclusively owned RTL8852BE function.
pub trait DeviceIo:fwcmd::Bus { fn pci_id(&mut self)->io::Result<u32>; fn bar_len(&self)->usize; }

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub enum Error { Request,Session,Clock,Timeout,Register(Fault),Command(h2c::Error),Dma(fwcmd::Error),Receive(rx::Error),Permit(post_mac::Error),Event(c2h::Error),EventOverflow,InboxOverflow,Malformed,Rejected(u8),Coexistence,PacketHandler }
impl From<Fault> for Error { fn from(e:Fault)->Self { Self::Register(e) } }
#[derive(Clone,Copy,Debug,Default,Eq,PartialEq)]
pub struct Counts { pub non_c2h:u32,pub other_c2h:u32,pub unrelated_ack:u32 }

trait PacketHandler{
    fn packet(&mut self,now:u64,channel:rx::Channel,packet:rxdesc::Packet,buffer:&[u8],full:bool)->Result<bool,Error>;
}
struct RetainAll;
impl PacketHandler for RetainAll{
    fn packet(&mut self,_:u64,_:rx::Channel,_:rxdesc::Packet,_:&[u8],_:bool)->Result<bool,Error>{Ok(false)}
}

/// Consuming Ready prevents recreating sequence zero against the same successful firmware startup.
/// Borrowed coherent DMA objects and their backing storage stay owned after errors or Drop; external shutdown is mandatory.
pub struct Session<'a,'command,'receive,B> {
    bus:&'a mut B,queue:&'a mut fwcmd::Queue<'command>,rings:&'a mut rx::Rings<'receive>,receiving:&'a post_mac::Receiving,mode:data_mode::Mode,
    inbox:Option<&'a mut Inbox>,terminal_copy:Option<inbox::Metadata>,firmware:download::Ready,scratch:&'a mut[u8],engine:h2c::Engine,failed:Option<Error>,counts:Counts,
    pub(crate) btc:Option<crate::btc_context::Context>,events:[[u8;EVENT_BYTES];EVENT_SLOTS],lengths:[usize;EVENT_SLOTS],event_head:usize,event_count:usize,last:u64,
}
impl<'a,'command,'receive,B:DeviceIo> Session<'a,'command,'receive,B> {
    /// Startup diagnostic only: packets outside command/BTC handling are counted and discarded before data service exists.
    /// Runtime RX/TX service must use new_with_inbox and keep this Session as the only ring reader.
    pub fn new(bus:&'a mut B,queue:&'a mut fwcmd::Queue<'command>,rings:&'a mut rx::Rings<'receive>,receiving:&'a post_mac::Receiving,
        firmware:download::Ready,scratch:&'a mut[u8])->Result<Self,Error> {
        if scratch.len()<rx::BUFFER_BYTES || firmware.session()!=queue.session() || firmware.version()!=[0,29,29,15]
            || bus.bar_len()<0x20000 { return Err(Error::Session); }
        let start=bus.now_us();
        if bus.pci_id()?!=0xb85210ec { return Err(Error::Session); }
        let left=10_000u64.checked_sub(bus.now_us().checked_sub(start).ok_or(Error::Clock)?).filter(|x|*x>0).ok_or(Error::Timeout)?;
        receiving.validate(bus,queue,rings,left as u32).map_err(Error::Permit)?;
        let now=bus.now_us(); if now<start { return Err(Error::Clock); } if now-start>=10_000 { return Err(Error::Timeout); }
        Ok(Self { bus,queue,rings,receiving,mode:data_mode::Mode::Startup,firmware,scratch,engine:h2c::Engine::new(now),failed:None,counts:Counts::default(),
            inbox:None,terminal_copy:None,btc:None,events:[[0;EVENT_BYTES];EVENT_SLOTS],lengths:[0;EVENT_SLOTS],event_head:0,event_count:0,last:now })
    }
    /// The caller owns the storage; no large runtime inbox is placed in the Session or allocated per packet.
    /// A bank is single-use for a firmware session and remains inspectable after terminal errors.
    pub fn new_with_inbox(bus:&'a mut B,queue:&'a mut fwcmd::Queue<'command>,rings:&'a mut rx::Rings<'receive>,receiving:&'a post_mac::Receiving,
        firmware:download::Ready,scratch:&'a mut[u8],inbox:&'a mut Inbox)->Result<Self,Error> {
        if !inbox.bind(queue.session()){return Err(Error::Session)}
        let mut session=Self::new(bus,queue,rings,receiving,firmware,scratch)?;session.inbox=Some(inbox);Ok(session)
    }
    pub fn retains_packets(&self)->bool { self.inbox.is_some() }
    pub fn pending_packets(&self)->usize { self.inbox.as_ref().map_or(0,|inbox|inbox.pending()) }
    /// The record remains borrowed until processed; consume only after its actual owner accepts it.
    pub fn packet(&self)->Option<inbox::Record<'_>> { self.inbox.as_ref()?.front() }
    pub fn consume_packet(&mut self) { if let Some(inbox)=self.inbox.as_mut(){inbox.consume();} }
    /// Overflow is terminal: scratch cannot be reused, so the first record beyond the bank remains inspectable too.
    pub fn overflow_packet(&self)->Option<inbox::Record<'_>> {
        if self.failed!=Some(Error::InboxOverflow){return None}self.terminal_packet()
    }
    /// A validated full copy survives late release/clock failure; no freshness is inferred from old scratch bytes.
    /// This diagnostic record may also be in the bank and does not assert successful RX descriptor release.
    pub fn terminal_packet(&self)->Option<inbox::Record<'_>> {
        self.failed?;let metadata=self.terminal_copy?;
        Some(inbox::Record{metadata,buffer:&self.scratch[..metadata.packet.offset+metadata.packet.length]})
    }
    /// Read at most one packet per hardware channel; command ACK and BTC routes stay active between commands.
    pub fn poll_packets(&mut self,timeout_us:u32)->Result<[bool;2],Error> {
        if self.inbox.is_none()||timeout_us==0||timeout_us>MAX_COMMAND_US{return Err(Error::Request)}
        if let Some(e)=self.failed{return Err(e)}
        let deadline=match self.now().and_then(|now|now.checked_add(u64::from(timeout_us)).ok_or(Error::Clock)){
            Ok(v)=>v,Err(e)=>return self.fail(e),
        };
        self.poll_before(deadline)
    }
    pub fn command_session(&self)->u64 { self.queue.session() }
    pub(crate) fn abort_coexistence(&mut self){let _=self.fail::<()>(Error::Coexistence);}
    pub(crate) fn clock(&mut self)->Result<u64,Error>{self.now()}
    pub(crate) fn registers<T>(&mut self,deadline:u64,run:impl FnOnce(&mut Window<'_,B>)->io::Result<T>)->Result<T,Error>{
        if let Some(e)=self.failed{return Err(e)}
        self.cleanup_registers(deadline,run)
    }
    pub(crate) fn cleanup_registers<T>(&mut self,deadline:u64,run:impl FnOnce(&mut Window<'_,B>)->io::Result<T>)->Result<T,Error>{
        let mut window=Window{bus:self.bus,deadline,last:self.last,clock_bad:false};
        let result=run(&mut window).and_then(|v|{window.delay_us(0)?;Ok(v)});self.last=window.last;
        result.map_err(Error::Register)
    }
    pub(crate) fn poll_before(&mut self,deadline:u64)->Result<[bool;2],Error>{
        self.poll_before_with(deadline,&mut RetainAll)
    }
    fn poll_before_with(&mut self,deadline:u64,handler:&mut impl PacketHandler)->Result<[bool;2],Error>{
        if let Some(e)=self.failed{return Err(e)}
        let result=(||{let left=self.left(deadline)?;self.live(left.min(10_000))?;
            let received=[self.receive_one_with(rx::Channel::Rxq,deadline,handler)?,self.receive_one_with(rx::Channel::Rpq,deadline,handler)?];
            let now=self.now()?;self.engine.poll(now).map_err(Error::Command)?;self.left(deadline)?;Ok(received)})();
        match result{Ok(v)=>Ok(v),Err(e)=>self.fail(e)}
    }

    pub fn firmware_version(&self)->[u8;4] { self.firmware.version() }
    pub fn failed(&self)->Option<Error> { self.failed }
    pub fn counts(&self)->Counts { self.counts }
    pub fn submitted(&self)->u16 { self.engine.submitted() }
    pub fn pending_events(&self)->usize { self.event_count }
    /// BTC events are retained in order for the policy owner; no event is treated as an ACK or RFK grant.
    pub fn event(&self)->Option<c2h::Event<'_>> {
        if self.event_count==0 { return None; }
        c2h::parse(&self.events[self.event_head][..self.lengths[self.event_head]]).ok()
    }
    /// Call only after the owner has processed this event; full storage fails rather than silently dropping BT state.
    pub fn consume_event(&mut self) { if self.event_count!=0 { self.lengths[self.event_head]=0;self.event_head=(self.event_head+1)%EVENT_SLOTS;self.event_count-=1; } }
    fn fail<T>(&mut self,error:Error)->Result<T,Error> { if self.failed.is_none(){self.failed=Some(error);} self.engine.abort();Err(self.failed.unwrap()) }
    fn now(&mut self)->Result<u64,Error> {
        let now=self.bus.now_us();if now<self.last{return Err(Error::Clock)}self.last=now;Ok(now)
    }
    fn left(&mut self,deadline:u64)->Result<u32,Error> { let now=self.now()?;deadline.checked_sub(now).filter(|v|*v>0).map(|v|v.min(u32::MAX as u64)as u32).ok_or(Error::Timeout) }
    fn live(&mut self,budget:u32)->Result<(),Error> {
        if self.firmware.session()!=self.queue.session(){return Err(Error::Session)}
        match &self.mode{
            data_mode::Mode::Startup=>{self.receiving.validate(self.bus,self.queue,self.rings,budget).map_err(Error::Permit)?;},
            data_mode::Mode::Data(layout)=>{self.receiving.validate_data(self.bus,self.queue,self.rings,layout,budget).map_err(Error::Permit)?;},
        }Ok(())
    }
    fn retain(&mut self,channel:rx::Channel,packet:rxdesc::Packet)->Result<(),Error> {
        let Some(inbox)=self.inbox.as_mut() else{return Ok(())};
        let metadata=inbox::Metadata{channel,packet};
        match inbox.push(metadata,self.scratch){
            Ok(())=>Ok(()),
            Err(inbox::Error::Full)=>Err(Error::InboxOverflow),
            Err(inbox::Error::Packet)=>Err(Error::Malformed),
        }
    }
    fn dispatch(&mut self,channel:rx::Channel,packet:rxdesc::Packet,deadline:u64,handler:&mut impl PacketHandler)->Result<(),Error>{
        let now=self.now()?;let handled=handler.packet(now,channel,packet,self.scratch,self.pending_packets()==inbox::SLOTS)?;
        self.left(deadline)?;if !handled{self.retain(channel,packet)?;}Ok(())
    }
    fn receive_one_with(&mut self,channel:rx::Channel,deadline:u64,handler:&mut impl PacketHandler)->Result<bool,Error> {
        let left=self.left(deadline)?;
        let mut window=Window{bus:self.bus,deadline,last:self.last,clock_bad:false};
        let received=self.rings.receive(&mut window,channel,self.scratch,left);self.last=window.last;
        self.terminal_copy=if self.inbox.is_some(){self.rings.copied_packet().map(|packet|inbox::Metadata{channel,packet})}else{None};
        let packet=match received.map_err(Error::Receive)? {
            rx::Outcome::Empty=>return Ok(false),rx::Outcome::Dropped(_)=>return Err(Error::Malformed),rx::Outcome::Packet(p)=>p,
        };
        self.left(deadline)?;
        if packet.kind!=rxdesc::TYPE_C2H {
            self.counts.non_c2h=self.counts.non_c2h.saturating_add(1);self.dispatch(channel,packet,deadline,handler)?;
            self.left(deadline)?;self.terminal_copy=None;return Ok(true);
        }
        if channel!=rx::Channel::Rxq||packet.crc_error||packet.icv_error { return Err(Error::Malformed); }
        let bytes=packet.bytes(self.scratch).ok_or(Error::Malformed)?;
        let event=c2h::parse(bytes).map_err(Error::Event)?;
        if let Some(ack)=event.ack().map_err(Error::Event)? {
            let now=self.bus.now_us();if now<self.last{return Err(Error::Clock)}self.last=now;
            match self.engine.acknowledge(now,ack).map_err(Error::Command)? {
                h2c::Observation::Unrelated|h2c::Observation::Unexpected=>{
                    self.counts.unrelated_ack=self.counts.unrelated_ack.saturating_add(1);self.retain(channel,packet)?;
                },_=>(),
            }
        } else if event.category==2&&event.class==0x12 {
            if bytes.len()>EVENT_BYTES||self.event_count==EVENT_SLOTS{return Err(Error::EventOverflow)}
            let at=(self.event_head+self.event_count)%EVENT_SLOTS;
            self.events[at][..bytes.len()].copy_from_slice(bytes);self.lengths[at]=bytes.len();self.event_count+=1;
        } else { self.counts.other_c2h=self.counts.other_c2h.saturating_add(1);self.dispatch(channel,packet,deadline,handler)?; }
        self.left(deadline)?;self.terminal_copy=None;Ok(true)
    }
    /// One absolute budget covers ownership checks, publication, RX/ACK processing and command-DMA quiescence.
    /// Only pre-publication Full is retried; uncertain send, malformed RX, queue loss and elapsed budgets poison the session.
    pub fn command(&mut self,command:h2c::Command,body:&[u8],acks:h2c::Acks,timeout_us:u32)->Result<h2c::Outcome,Error> {
        if let Some(error)=self.failed{return Err(error)}
        if timeout_us==0||timeout_us>MAX_COMMAND_US||body.len()>h2c::MAX_BODY{return Err(Error::Request)}
        let result=self.command_inner(command,body,acks,timeout_us);
        match result {Ok(v)=>Ok(v),Err(e)=>self.fail(e)}
    }
    fn command_inner(&mut self,command:h2c::Command,body:&[u8],acks:h2c::Acks,timeout_us:u32)->Result<h2c::Outcome,Error> {
        let deadline=self.now()?.checked_add(timeout_us as u64).ok_or(Error::Clock)?;
        self.command_deadline(command,body,acks,deadline)
    }
    pub(crate) fn command_before(&mut self,command:h2c::Command,body:&[u8],acks:h2c::Acks,outer:u64)->Result<h2c::Outcome,Error>{
        if let Some(e)=self.failed{return Err(e)}
        let own=self.now()?.checked_add(MAX_COMMAND_US as u64).ok_or(Error::Clock)?;
        let result=self.command_deadline(command,body,acks,outer.min(own));
        match result{Ok(v)=>Ok(v),Err(e)=>self.fail(e)}
    }
    fn command_deadline(&mut self,command:h2c::Command,body:&[u8],acks:h2c::Acks,deadline:u64)->Result<h2c::Outcome,Error> {
        self.command_deadline_with(command,body,acks,deadline,&mut RetainAll)
    }
    fn command_deadline_with(&mut self,command:h2c::Command,body:&[u8],acks:h2c::Acks,deadline:u64,handler:&mut impl PacketHandler)->Result<h2c::Outcome,Error> {
        let timeout_us=self.left(deadline)?;
        let left=self.left(deadline)?;self.live(left.min(10_000))?;
        let mut ticket=None;let mut completed=None;
        for _ in 0..=timeout_us/10+2 {
            let left=self.left(deadline)?;let now=self.last;self.engine.poll(now).map_err(Error::Command)?;
            if ticket.is_none() {
                let mut window=Window{bus:self.bus,deadline,last:self.last,clock_bad:false};
                let submitted=self.engine.submit(&mut h2c::CommandQueue{bus:&mut window,queue:self.queue},command,body,acks,left);self.last=window.last;
                match submitted {
                    Ok(sent)=>ticket=Some(sent),Err(h2c::Error::Full)=>(),Err(e)=>return Err(Error::Command(e)),
                }
            }
            self.receive_one_with(rx::Channel::Rxq,deadline,handler)?;self.receive_one_with(rx::Channel::Rpq,deadline,handler)?;
            let now=self.now()?;self.engine.poll(now).map_err(Error::Command)?;
            if let Some(sent)=ticket { if completed.is_none() { completed=self.engine.take(now,sent).map_err(Error::Command)?; } }
            if let Some(outcome)=completed {
                if let h2c::Outcome::Rejected(status)=outcome{return Err(Error::Rejected(status))}
                self.left(deadline)?;let mut window=Window{bus:self.bus,deadline,last:self.last,clock_bad:false};
                let observed=(||{self.queue.reclaim(&mut window).map_err(Error::Dma)?;
                    let busy=io::read32(&mut window,fwcmd::DMA_BUSY)?;window.delay_us(0)?;Ok::<_,Error>(busy)})();
                self.last=window.last;let busy=observed?;
                if self.queue.pending()==0&&busy&TX_BUSY==0 { self.left(deadline)?;return Ok(outcome); }
            }
            self.left(deadline)?;let mut window=Window{bus:self.bus,deadline,last:self.last,clock_bad:false};
            let delayed=window.delay_us(10);self.last=window.last;delayed?;
        }
        Err(Error::Timeout)
    }
}

fn fault(error:Error)->Fault {
    match error {Error::Register(e)=>e,Error::Clock=>Fault::new(FaultKind::Clock,fwcmd::INDEX,0),
        Error::Timeout=>Fault::new(FaultKind::Timeout,fwcmd::INDEX,0),_=>Fault::new(FaultKind::PowerState,fwcmd::INDEX,0)}
}
impl<B:DeviceIo> RegisterIo for Session<'_,'_,'_,B> {
    fn read8(&mut self,at:usize)->io::Result<u8>{self.bus.read8(at)}
    fn read16(&mut self,at:usize)->io::Result<u16>{self.bus.read16(at)}
    fn read32(&mut self,at:usize)->io::Result<u32>{self.bus.read32(at)}
    fn write8(&mut self,at:usize,v:u8)->io::Result<()>{self.bus.write8(at,v)}
    fn write16(&mut self,at:usize,v:u16)->io::Result<()>{self.bus.write16(at,v)}
    fn write32(&mut self,at:usize,v:u32)->io::Result<()>{self.bus.write32(at,v)}
    fn now_us(&mut self)->u64{self.bus.now_us()}
    fn delay_us(&mut self,us:u32)->io::Result<()>{self.bus.delay_us(us)}
}
impl<B:DeviceIo> PhyIo for Session<'_,'_,'_,B> {
    fn pci_id(&mut self)->io::Result<u32>{if let Some(e)=self.failed{return Err(fault(e))}self.bus.pci_id()}
    fn pci_command(&mut self)->io::Result<u16>{if let Some(e)=self.failed{return Err(fault(e))}self.bus.pci_command()}
    fn command_session(&self)->u64{if self.failed.is_some(){0}else{self.queue.session()}}
    fn bar_len(&self)->usize{self.bus.bar_len()}
    fn receive_permit(&mut self,timeout_us:u32)->io::Result<post_mac::ReceivePermit<'_>> {
        if let Some(error)=self.failed{return Err(fault(error))}
        if !matches!(self.mode,data_mode::Mode::Startup){return Err(fault(Error::Session))}
        match self.receiving.validate(self.bus,self.queue,self.rings,timeout_us) {
            Ok(permit)=>Ok(permit),
            Err(error)=>{self.failed=Some(Error::Permit(error));self.engine.abort();Err(fault(Error::Permit(error)))},
        }
    }
    fn send_rf_page(&mut self,path:u8,page:u8,payload:&[u8],timeout_us:u32)->io::Result<()> {
        if path>1||page!=0||payload.is_empty()||payload.len()>294*4||payload.len()%4!=0{return Err(Fault::new(FaultKind::Access,0,payload.len()as u32))}
        self.command(h2c::Command{category:2,class:8+path,function:page},payload,h2c::Acks{received:false,done:false},timeout_us).map(|_|()).map_err(fault)
    }
}

impl<B:DeviceIo> crate::rxdck::RxdckIo for Session<'_,'_,'_,B> {
    fn scheduler_zero(&mut self,timeout_us:u32)->io::Result<()> {
        if let Some(error)=self.failed{return Err(fault(error))}
        if !matches!(self.mode,data_mode::Mode::Startup){return Err(fault(Error::Session))}
        if timeout_us==0||timeout_us>1_000_000{return Err(Fault::new(FaultKind::Access,0xc348,timeout_us))}
        let result=(||{
            let deadline=self.now()?.checked_add(u64::from(timeout_us)).ok_or(Error::Clock)?;
            let left=self.left(deadline)?;self.live(left.min(10_000))?;
            let left=self.left(deadline)?;
            self.registers(deadline,|window|crate::runtime_mac::scheduler_zero(window,left))?;
            self.left(deadline)?;Ok(())
        })();
        match result{Ok(())=>Ok(()),Err(error)=>self.fail::<()>(error).map_err(fault)}
    }
}

#[cfg(test)]
#[path="../../tests/rtw89_btc_session_cases.rs"]
mod tests;
