//! Bounded wlanhw 1.1 adapter for one actually admitted channel; native construction remains separate.

use crate::{btc_session::{DeviceIo,data_mode::{self,DataSession}},frames,rx,rxdesc,tx};
use mind::{dev,idl::{codec::List,wire,wlanhw},ipc::Endpoint,sys,time};

const RECEIVE:[usize;2]=[9,10];
const POLL_MS:u32=2;
const WORK_US:u32=100_000;
const COMMAND_US:u32=100_000;
const WAIT_MS:u64=1000;
const WAITERS:usize=4;
const FRAMES:usize=4;
const FRAME_BYTES:usize=frames::HEADER_BYTES+frames::FRAME_MAX;

#[derive(Debug)]
pub(crate) enum Fatal{Radio(data_mode::Failure),Ipc(sys::Error),Clock,TransmitTimeout,UnknownAttempts,RetainedControl,Frame(frames::Error)}
struct Pending{call:wire::Call,slot:usize,deadline:u64}
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
}
pub(crate) struct Stopped<'s,'q,'memory,'p,'a,'command,'receive,B:DeviceIo>{
    pub owner:&'s mut DataSession<'q,'memory,'p,'a,'command,'receive,B>,
    pub failure:Option<Fatal>,pub shutdown:data_mode::Shutdown,
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
        Ok(Self{owner:Some(owner),pending:None,waiters:[const{None};WAITERS],frames:Frames::new(),last_ms:time::monotonic_ns()/1_000_000,failure:None})
    }
    fn data(&mut self)->&mut DataSession<'q,'memory,'p,'a,'command,'receive,B>{self.owner.as_deref_mut().unwrap()}
    fn fail(&mut self,cause:Fatal){
        if self.failure.is_none(){self.failure=Some(cause);}
        self.data().shutdown();self.answer_pending();
    }
    fn answer_pending(&mut self){
        if let Some(pending)=self.pending.take(){let _=wlanhw::reply_send(pending.call,Err(wlanhw::Error::Failed));}
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
        if self.pending.as_ref().is_some_and(|p|now>=p.deadline){return Err(Fatal::TransmitTimeout)}
        if self.pending.is_some(){
            if let Some(done)=self.data().advance(WORK_US).map_err(Fatal::Radio)?{
                let tx::AttemptCount::Known(attempts)=done.attempts else{return Err(Fatal::UnknownAttempts)};
                if !(1..=15).contains(&attempts){return Err(Fatal::UnknownAttempts)}
                let pending=self.pending.take().unwrap();
                let _=wlanhw::reply_send(pending.call,Ok(&wlanhw::Sent{acked:done.acked,attempts}));
            }
        }else{self.data().poll_packets(WORK_US).map_err(Fatal::Radio)?;}
        self.collect()?;let waiting=self.frames.count as u32;
        for waiter in &mut self.waiters{
            if waiter.as_ref().is_some_and(|w|waiting!=0||now>=w.deadline){
                let _=wlanhw::reply_wait(waiter.take().unwrap().call,waiting);
            }
        }
        Ok(())
    }
    /// One receive and one bounded RX/TX progress step; continuous IPC or RX traffic cannot suppress the other.
    pub(crate) fn step(&mut self)->bool{
        if self.failure.is_some(){return false}
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
            wlanhw::Request::Info=>{
                let mut channels=List::default();channels.push(self.data().channel());
                let info=wlanhw::Info{mac:packed(self.data().mac()),bands:mind::network::BAND_2GHZ|mind::network::BAND_2GHZ_OFDM,
                    channels,frame_max:frames::FRAME_MAX as u16,channel:self.data().channel()};
                let _=wlanhw::reply_info(call,Ok(&info));
            },
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
                if self.pending.is_some(){let _=wlanhw::reply_send(call,Err(wlanhw::Error::Busy));return}
                let Some(deadline)=now.checked_add(u64::from(COMMAND_US/1000))else{
                    let _=wlanhw::reply_send(call,Err(wlanhw::Error::Failed));self.fail(Fatal::Clock);return};
                let mut call=call;
                if call.defer().is_err(){let _=wlanhw::reply_send(call,Err(wlanhw::Error::Busy));return}
                match self.data().begin(frame,rate,attempts,COMMAND_US){
                    Ok(())=>self.pending=Some(Pending{call,slot,deadline}),
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
            // wlanhw 1.2 (550-NET-0035): no country's limits applied yet, so none taken.
            wlanhw::Request::SetCountry{..}=>{let _=wlanhw::reply_set_country(call,Err(wlanhw::Error::Failed));},
        }
    }
    pub(crate) fn stop(mut self)->Stopped<'s,'q,'memory,'p,'a,'command,'receive,B>{
        self.answer_pending();let owner=self.owner.take().unwrap();let shutdown=owner.shutdown();
        Stopped{owner,failure:self.failure.take(),shutdown}
    }
    pub(crate) fn run(mut self)->Stopped<'s,'q,'memory,'p,'a,'command,'receive,B>{while self.step(){}self.stop()}
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
