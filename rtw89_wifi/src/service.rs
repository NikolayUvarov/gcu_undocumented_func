//! Bounded wlanhw 2.0 adapter for one actually admitted channel; native construction remains separate.

use crate::{btc_session::{DeviceIo,data_mode::{self,DataSession}},country::{Limits,Refusal},frames,receive_service::Event,rx,rxdesc,tx};
use mind::{dev,idl::{codec::List,wire,wlanhw},ipc::Endpoint,sys,time};

const RECEIVE:[usize;2]=[9,10];
const POLL_MS:u32=2;
// Receive passes a step: each takes one descriptor of each ring. One a step let the Ryzen's packet buffer fill on a busy
// channel (2026-11-08: CMAC RX DMA error, then the firmware's L0 recovery); Linux's NAPI takes 64 a pass.
const DRAIN:usize=32;
const WORK_US:u32=100_000;
const COMMAND_US:u32=100_000;
// A send times out here this much before the data mode's own deadline for it, so that a frame that never ends stops
// the service as TransmitTimeout with what it saw, not as whatever register read ran out of its time (the Ryzen, 2026-11-08).
const FRAME_MARGIN_MS:u64=10;
// DEBUG (the Ryzen, 2026-11-08: everything runs first, hardening later): a send makes at most this many submissions, which
// fit its time at about 7 ms each; wlan's 15 at 24 Mb/s, none acknowledged, outran it, so its rate never stepped down.
const SEND_TRIES:u8=8;
// DEBUG (the same): a send that outruns its time is answered as sent and not acknowledged, and its release is awaited
// LATE_MS more while nothing else is sent; one that outruns that too stops the service. On the Ryzen one frame in some
// minutes waited past its time for a release. False: TransmitTimeout at once.
const KEEP_LATE:bool=true;
const LATE_MS:u64=1000;
const WAIT_MS:u64=1000;
const WAITERS:usize=4;
const FRAMES:usize=4;
const FRAME_BYTES:usize=frames::HEADER_BYTES+frames::FRAME_MAX;
// DEBUG (the maintainer, 2026-11-08: everything runs first, hardening later): a country that refuses the admitted
// channel is logged as such, answered as applied, and the radio keeps sending. False is wlanhw 1.2's `failed`.
const KEEP_SENDING:bool=true;

#[derive(Debug)]
pub(crate) enum Fatal{Radio(data_mode::Failure),Ipc(sys::Error),Clock,TransmitTimeout(tx::Seen),UnknownAttempts,RetainedControl,Frame(frames::Error)}
// `grace_us`: the data mode's grace when the send began; a recovery it outlasts since moves `deadline` (OUTLAST_SER).
// `call` is None once a late send was answered (KEEP_LATE) and only its release is awaited.
struct Pending{call:Option<wire::Call>,slot:usize,deadline:u64,grace_us:u64}
struct Waiter{call:wire::Call,deadline:u64}
struct Frames{bytes:[[u8;FRAME_BYTES];FRAMES],lengths:[usize;FRAMES],head:usize,count:usize}
impl Frames{
    fn new()->Self{Self{bytes:[[0;FRAME_BYTES];FRAMES],lengths:[0;FRAMES],head:0,count:0}}
    fn clear(&mut self){self.head=0;self.count=0;}
}

/// Receive slots 9 and 10 must be reserved and empty; the endpoint must be granted only to the WLAN station.
/// The borrowed data owner and all DMA remain alive after stop, failure, or dropping this adapter.
pub(crate) struct Service<'s,'q,'memory,'p,'a,'command,'receive,B:DeviceIo>{
    owner:Option<&'s mut DataSession<'q,'memory,'p,'a,'command,'receive,B>>,
    pending:Option<Pending>,waiters:[Option<Waiter>;WAITERS],frames:Frames,last_ms:u64,failure:Option<Fatal>,
    // The EIRP the channel was admitted under and the settings taken so far; `refused`: a country it cannot keep, so
    // nothing is sent (wlanhw 1.2). `event`: one for the caller's log.
    country:Option<(i8,u64)>,refused:bool,event:Option<Event>,strict:bool,
    // `late`: KEEP_LATE; `lates`: the sends that outran their time so far.
    late:bool,lates:u32,
    // DEBUG (2026-11-08): the SNAPSHOT registers at the start, and when each first changed (ms from the start, its value,
    // whether a send was pending), read once a step.
    first:data_mode::Snapshot,changes:Changes,start_ms:u64,
}
/// When each SNAPSHOT register first changed in data mode: (ms from the start, value, a send pending).
#[derive(Clone,Copy)]
pub(crate) struct Changes(pub [Option<(u64,u32,bool)>;data_mode::SNAPSHOT.len()]);
impl core::fmt::Debug for Changes{
    fn fmt(&self,f:&mut core::fmt::Formatter<'_>)->core::fmt::Result{
        for(at,change)in data_mode::SNAPSHOT.iter().zip(self.0){
            if let Some((ms,value,pending))=change{write!(f,"{:X}={:08X}@{}MS{} ",at,value,ms,if pending{"+TX"}else{""})?;}
        }Ok(())
    }
}
pub(crate) struct Stopped<'s,'q,'memory,'p,'a,'command,'receive,B:DeviceIo>{
    pub owner:&'s mut DataSession<'q,'memory,'p,'a,'command,'receive,B>,
    pub failure:Option<Fatal>,pub shutdown:data_mode::Shutdown,pub changes:Changes,
}
fn address(value:u64)->[u8;6]{core::array::from_fn(|i|(value>>(40-8*i))as u8)}
fn packed(value:[u8;6])->u64{value.into_iter().fold(0,|word,byte|(word<<8)|u64::from(byte))}
fn error(failure:&data_mode::Failure)->wlanhw::Error{
    match failure.cause{
        data_mode::Cause::Tx(tx::Error::Busy)=>wlanhw::Error::Busy,
        data_mode::Cause::Tx(tx::Error::Input(_))=>wlanhw::Error::Invalid,
        _=>wlanhw::Error::Failed,
    }
}

impl<'s,'q,'memory,'p,'a,'command,'receive,B:DeviceIo> Service<'s,'q,'memory,'p,'a,'command,'receive,B>{
    pub(crate) fn new(owner:&'s mut DataSession<'q,'memory,'p,'a,'command,'receive,B>)->Result<Self,sys::Error>{
        if RECEIVE.into_iter().any(|slot|dev::cap_info(slot).0!=mind::abi::CAP_KIND_NONE){return Err(sys::Error::NoSlot)}
        let first=owner.registers();let start_ms=time::monotonic_ns()/1_000_000;
        Ok(Self{owner:Some(owner),pending:None,waiters:[const{None};WAITERS],frames:Frames::new(),last_ms:start_ms,failure:None,
            country:None,refused:false,event:None,strict:!KEEP_SENDING,late:KEEP_LATE,lates:0,first,changes:Changes([None;data_mode::SNAPSHOT.len()]),start_ms})
    }
    /// The channel was admitted at `eirp_half_dbm` under the station's country, after `settings` set-country requests.
    pub(crate) fn under(mut self,eirp_half_dbm:i8,settings:u64)->Self{self.country=Some((eirp_half_dbm,settings));self}
    #[cfg(test)]
    pub(crate) fn take_event(&mut self)->Option<Event>{self.event.take()}
    #[cfg(test)]
    pub(crate) fn take_recovery(&mut self)->Option<data_mode::Recovery>{self.data().take_recovery()}
    /// Whether a country that refuses the channel stops sending (wlanhw 1.2), or is only logged (KEEP_SENDING).
    #[cfg(test)]
    pub(crate) fn strict(mut self,on:bool)->Self{self.strict=on;self}
    /// Whether a send that outruns its time is answered and its release awaited (KEEP_LATE), or stops the service.
    #[cfg(test)]
    pub(crate) fn keep_late(mut self,on:bool)->Self{self.late=on;self}
    fn info(&mut self)->wlanhw::Info{
        let mut channels=List::default();channels.push(self.data().channel());
        // Its own channels: 1 to 13 on 2.4 GHz at 20 MHz, its library's scope (wlanhw.wit 2.0, 550-NET-0059).
        let mut supported=List::default();for ch in 1..=13{supported.push(ch);}
        wlanhw::Info{mac:packed(self.data().mac()),bands:mind::network::BAND_2GHZ|mind::network::BAND_2GHZ_OFDM,
            channels,supported,frame_max:frames::FRAME_MAX as u16,channel:self.data().channel()}
    }
    // A new country keeps the channel if it still lets the radio send there at no less EIRP than it was admitted under.
    fn country(&mut self,code:&str,limits:&[wlanhw::Limit])->Result<(),wlanhw::Error>{
        let channel=self.data().channel();
        let Some((eirp,settings))=self.country.as_mut()else{return Err(wlanhw::Error::Failed)};
        *settings=settings.saturating_add(1);
        match Limits::accept(code,*settings,limits.iter().map(|l|(l.channel,l.eirp,l.listen))){
            Ok(taken)=>{
                let kept=taken.applied(channel).is_some_and(|a|!a.listen&&a.eirp_half_dbm>=*eirp);
                self.event=Some(Event::DataCountry{code:taken.code(),channel,kept,enforced:self.strict});
                if kept||!self.strict{Ok(())}else{Err(wlanhw::Error::Failed)}
            }
            Err(Refusal::Invalid)=>Err(wlanhw::Error::Invalid),
            Err(Refusal::Failed)=>Err(wlanhw::Error::Failed),
        }
    }
    fn data(&mut self)->&mut DataSession<'q,'memory,'p,'a,'command,'receive,B>{self.owner.as_deref_mut().unwrap()}
    fn fail(&mut self,cause:Fatal){
        if self.failure.is_none(){self.watch();self.failure=Some(cause);}
        self.data().shutdown();self.answer_pending();
    }
    fn answer_pending(&mut self){
        if let Some(call)=self.pending.take().and_then(|p|p.call){let _=wlanhw::reply_send(call,Err(wlanhw::Error::Failed));}
        for waiter in &mut self.waiters{if let Some(waiter)=waiter.take(){let _=wlanhw::reply_wait(waiter.call,0);}}
    }
    fn collect(&mut self)->Result<(),Fatal>{
        let data=self.owner.as_deref_mut().unwrap();
        for _ in 0..FRAMES{
            if self.frames.count==FRAMES{break}
            let Some(record)=data.packet()else{break};
            if record.metadata().channel!=rx::Channel::Rxq||record.metadata().packet.kind!=rxdesc::TYPE_WIFI{return Err(Fatal::RetainedControl)}
            let frame=frames::from_packet(record.metadata().packet,record.buffer()).map_err(Fatal::Frame)?;
            let at=(self.frames.head+self.frames.count)%FRAMES;
            let length=frame.write(data.channel(),data.current_filter().all_beacons(),&mut self.frames.bytes[at]).map_err(Fatal::Frame)?;
            self.frames.lengths[at]=length;self.frames.count+=1;data.consume_packet();
        }Ok(())
    }
    fn progress(&mut self,now:u64)->Result<(),Fatal>{
        let grace_ms=|p:&Pending,data:&DataSession<'q,'memory,'p,'a,'command,'receive,B>|data.grace_us().saturating_sub(p.grace_us)/1000;
        if self.pending.as_ref().is_some_and(|p|now>=p.deadline.saturating_add(grace_ms(p,self.owner.as_deref().unwrap()))){
            if !self.late||self.pending.as_ref().is_some_and(|p|p.call.is_none()){return Err(Fatal::TransmitTimeout(self.data().transmit_seen()))}
            let seen=self.data().transmit_seen();let pending=self.pending.as_mut().unwrap();
            pending.deadline=pending.deadline.saturating_add(LATE_MS);let call=pending.call.take().unwrap();
            let attempts=u8::try_from(seen.tries).unwrap_or(15).clamp(1,15);
            let _=wlanhw::reply_send(call,Ok(&wlanhw::Sent{acked:false,attempts}));
            self.data().extend_send(LATE_MS*1000);self.lates=self.lates.saturating_add(1);
            if self.event.is_none(){self.event=Some(Event::LateFrame{count:self.lates,finished:seen.finished,fetched:seen.fetched,
                released:seen.released,length:seen.length,age_us:seen.age_us})}
        }
        if self.pending.is_some(){self.finish_send()?}
        for _ in 0..DRAIN{
            let received=self.data().poll_packets(WORK_US).map_err(Fatal::Radio)?;self.collect()?;
            if received==[false,false]{break}
        }
        // A release the drain took is acted on in this step, not the next one after its receive wait.
        if self.pending.is_some(){self.finish_send()?}
        let waiting=self.frames.count as u32;
        for waiter in &mut self.waiters{
            if waiter.as_ref().is_some_and(|w|waiting!=0||now>=w.deadline){
                let _=wlanhw::reply_wait(waiter.take().unwrap().call,waiting);
            }
        }
        Ok(())
    }
    fn finish_send(&mut self)->Result<(),Fatal>{
        if let Some(done)=self.data().advance(WORK_US).map_err(Fatal::Radio)?{
            let tx::AttemptCount::Known(attempts)=done.attempts else{return Err(Fatal::UnknownAttempts)};
            if !(1..=15).contains(&attempts){return Err(Fatal::UnknownAttempts)}
            if let Some(call)=self.pending.take().unwrap().call{let _=wlanhw::reply_send(call,Ok(&wlanhw::Sent{acked:done.acked,attempts}));}
        }Ok(())
    }
    // DEBUG: the first change of each SNAPSHOT register, while the data mode runs: after its shutdown they are its own
    // (BB off, DMA stopped), which the Ryzen's log of 2026-11-08 had taken for the firmware's.
    fn watch(&mut self){
        let Some(data)=self.owner.as_deref_mut()else{return};if data.snapshot().is_some(){return}let now=data.registers();
        let ms=(time::monotonic_ns()/1_000_000).saturating_sub(self.start_ms);let pending=self.pending.is_some();
        for (i,change) in self.changes.0.iter_mut().enumerate(){
            if change.is_none()&&now.0[i]!=self.first.0[i]{*change=Some((ms,now.0[i],pending));}
        }
    }
    /// One receive and one bounded RX/TX progress step; continuous IPC or RX traffic cannot suppress the other.
    pub(crate) fn step(&mut self)->bool{
        if self.failure.is_some(){return false}
        self.watch();
        let slot=if self.pending.as_ref().is_some_and(|p|p.slot==RECEIVE[0]){RECEIVE[1]}else{RECEIVE[0]};
        let received=Endpoint::SERVICE.recv_timeout(slot,POLL_MS);
        let now=time::monotonic_ns()/1_000_000;
        if now<self.last_ms{self.fail(Fatal::Clock)}else{self.last_ms=now;if let Err(e)=self.progress(now){self.fail(e)}}
        let dispatch_now=time::monotonic_ns()/1_000_000;
        if dispatch_now<self.last_ms{self.fail(Fatal::Clock)}else{self.last_ms=dispatch_now}
        match received{
            Ok(request)=>self.dispatch(request,slot,dispatch_now),
            Err(sys::Error::Other(code))if code==mind::abi::ERR_TIMEOUT=>{},
            Err(error)=>self.fail(Fatal::Ipc(error)),
        }
        self.failure.is_none()
    }
    fn dispatch(&mut self,received:mind::ipc::Received,slot:usize,now:u64){
        let mut scratch=[0;wlanhw::REQUEST_MAX];
        let(request,call)=match wlanhw::decode(&received,slot,&mut scratch){
            Ok(decoded)=>decoded,Err(reason)=>{if received.is_call{let _=wire::reject(reason);}return},
        };
        if self.failure.is_some(){absent(request,call);return}
        match request{
            wlanhw::Request::Info=>{let info=self.info();let _=wlanhw::reply_info(call,Ok(&info));},
            wlanhw::Request::SetChannel{channel}=>{
                let result=if channel==self.data().channel(){Ok(())}else{Err(wlanhw::Error::Invalid)};
                let _=wlanhw::reply_set_channel(call,result);
            },
            wlanhw::Request::SetFilter{filter}=>{
                let result=self.data().set_filter(filter.all_beacons,address(filter.bssid),address(filter.own),COMMAND_US);
                if result.is_ok(){self.frames.clear();}
                let reply=result.as_ref().map(|_|()).map_err(error);let _=wlanhw::reply_set_filter(call,reply);
                if let Err(failure)=result{if failure.shutdown.is_some(){self.fail(Fatal::Radio(failure))}}
            },
            wlanhw::Request::Send{frame,rate,attempts}=>{
                if self.refused{let _=wlanhw::reply_send(call,Err(wlanhw::Error::Failed));return}
                if self.pending.is_some(){let _=wlanhw::reply_send(call,Err(wlanhw::Error::Busy));return}
                let Some(deadline)=now.checked_add(u64::from(COMMAND_US/1000)-FRAME_MARGIN_MS)else{
                    let _=wlanhw::reply_send(call,Err(wlanhw::Error::Failed));self.fail(Fatal::Clock);return};
                let mut call=call;
                if call.defer().is_err(){let _=wlanhw::reply_send(call,Err(wlanhw::Error::Busy));return}
                let grace_us=self.data().grace_us();
                let tries=if attempts<=15{attempts.min(SEND_TRIES)}else{attempts};
                match self.data().begin(frame,rate,tries,COMMAND_US){
                    Ok(())=>self.pending=Some(Pending{call:Some(call),slot,deadline,grace_us}),
                    Err(failure)=>{let _=wlanhw::reply_send(call,Err(error(&failure)));
                        if failure.shutdown.is_some(){self.fail(Fatal::Radio(failure))}},
                }
            },
            wlanhw::Request::Receive=>{
                if self.frames.count==0{let _=wlanhw::reply_receive(call,Err(wlanhw::Error::Empty));return}
                let at=self.frames.head;let _=wlanhw::reply_receive(call,Ok(&self.frames.bytes[at][..self.frames.lengths[at]]));
                self.frames.head=(at+1)%FRAMES;self.frames.count-=1;
            },
            wlanhw::Request::Wait=>{
                let count=self.frames.count as u32;
                if count!=0{let _=wlanhw::reply_wait(call,count);return}
                let Some(deadline)=now.checked_add(WAIT_MS)else{let _=wlanhw::reply_wait(call,0);self.fail(Fatal::Clock);return};
                let mut call=call;
                match self.waiters.iter_mut().find(|entry|entry.is_none()){
                    Some(entry)if call.defer().is_ok()=>*entry=Some(Waiter{call,deadline}),
                    _=>{let _=wlanhw::reply_wait(call,0);},
                }
            },
            wlanhw::Request::Attach{..}=>{let _=wlanhw::reply_attach(call,Err(wlanhw::Error::Invalid));},
            wlanhw::Request::Kick=>{let _=wlanhw::reply_kick(call,Err(wlanhw::Error::Invalid));},
            // wlanhw 1.2 (550-NET-0035): `failed` leaves the radio sending nothing until a country it can keep.
            wlanhw::Request::SetCountry{code,limits,..}=>{
                let reply=self.country(code.as_str(),limits.as_slice());
                match reply{Ok(())=>self.refused=false,Err(wlanhw::Error::Failed)=>self.refused=self.strict,Err(_)=>{}}
                let info=self.info();let _=wlanhw::reply_set_country(call,reply.map(|()|&info));
            },
        }
    }
    pub(crate) fn stop(mut self)->Stopped<'s,'q,'memory,'p,'a,'command,'receive,B>{
        self.answer_pending();let owner=self.owner.take().unwrap();let shutdown=owner.shutdown();
        Stopped{owner,failure:self.failure.take(),shutdown,changes:self.changes}
    }
    pub(crate) fn run(mut self,events:&mut impl FnMut(Event))->Stopped<'s,'q,'memory,'p,'a,'command,'receive,B>{
        loop{
            let more=self.step();if let Some(event)=self.event.take(){events(event)}
            if let Some(r)=self.owner.as_deref_mut().and_then(|data|data.take_recovery()){
                events(Event::Recovered{count:r.count,ser:r.ser,cmac:r.cmac,bb_us:r.bb_us,restored:r.restored,waited_us:r.waited_us,after_reconcile:r.after_reconcile})}
            if !more{break}
        }self.stop()
    }
}
impl<B:DeviceIo> Drop for Service<'_,'_,'_,'_,'_,'_,'_,B>{
    fn drop(&mut self){self.answer_pending();if let Some(owner)=self.owner.as_deref_mut(){owner.shutdown();}}
}
fn absent(request:wlanhw::Request<'_>,call:wire::Call){
    let no=wlanhw::Error::NoDevice;
    let _=match request{
        wlanhw::Request::Info=>wlanhw::reply_info(call,Err(no)),
        wlanhw::Request::SetChannel{..}=>wlanhw::reply_set_channel(call,Err(no)),
        wlanhw::Request::SetFilter{..}=>wlanhw::reply_set_filter(call,Err(no)),
        wlanhw::Request::Send{..}=>wlanhw::reply_send(call,Err(no)),
        wlanhw::Request::Receive=>wlanhw::reply_receive(call,Err(no)),
        wlanhw::Request::Wait=>wlanhw::reply_wait(call,0),
        wlanhw::Request::Attach{..}=>wlanhw::reply_attach(call,Err(no)),
        wlanhw::Request::Kick=>wlanhw::reply_kick(call,Err(no)),
        wlanhw::Request::SetCountry{..}=>wlanhw::reply_set_country(call,Err(no)),
    };
}
