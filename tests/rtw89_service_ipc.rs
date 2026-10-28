//! Kernel-call seam only; service dispatch uses the actual generated schema, codec and wire ownership code.
use crate::{abi,sys};
use std::{cell::RefCell,collections::{HashMap,VecDeque}};

pub mod ipc{
    use super::*;
    #[derive(Clone,Copy,Debug)]pub struct Message{pub data:[usize;2],pub cap:usize}
    impl Message{
        pub fn new(a:usize,b:usize)->Self{Self{data:[a,b],cap:0}}
        pub fn with_cap(mut self,cap:usize,_:u8)->Self{self.cap=cap;self}
        pub fn with_cap_moved(self,cap:usize,rights:u8)->Self{self.with_cap(cap,rights)}
    }
    #[derive(Clone,Copy,Debug)]pub struct Received{pub data:[usize;2],pub sender:u64,pub badge:u16,pub cap_received:bool,pub is_call:bool,pub irq:Option<usize>}
    #[derive(Clone,Copy)]pub struct Endpoint(pub usize);
    struct Request{id:usize,words:[usize;2],kind:usize,is_call:bool}
    #[derive(Clone,Copy)]enum Cap{Memory(usize),Reply(usize),Other(usize)}
    #[derive(Default)]struct State{
        now:u64,next:usize,queue:VecDeque<Request>,buffers:HashMap<usize,Box<[u8;4096]>>,caps:HashMap<usize,Cap>,
        current:Option<usize>,replies:HashMap<usize,[usize;2]>,drops:HashMap<usize,usize>,receives:Vec<(usize,u32)>,
        cancelled:Vec<usize>,save_fail:bool,duplicate_drops:usize,
    }
    thread_local!{static STATE:RefCell<State>=RefCell::new(State::default());}
    pub fn reset(){STATE.with(|s|*s.borrow_mut()=State::default());}
    pub fn clock()->u64{STATE.with(|s|s.borrow().now)}
    pub fn advance(ms:u64){STATE.with(|s|s.borrow_mut().now+=ms*1_000_000);}
    pub fn backwards(){STATE.with(|s|s.borrow_mut().now=0);}
    pub fn save_failure(value:bool){STATE.with(|s|s.borrow_mut().save_fail=value);}
    pub fn cancel(id:usize){STATE.with(|s|s.borrow_mut().cancelled.push(id));}
    pub fn enqueue(words:[usize;2],kind:usize,body:&[u8],is_call:bool)->usize{STATE.with(|s|{
        let mut s=s.borrow_mut();s.next+=1;let id=s.next;
        let mut bytes=Box::new([0;4096]);bytes[..body.len()].copy_from_slice(body);s.buffers.insert(id,bytes);
        s.queue.push_back(Request{id,words,kind,is_call});id
    })}
    pub fn words(method:usize,field:usize)->usize{enqueue([method|1<<8|field<<16,0],abi::CAP_KIND_NONE,&[],true)}
    pub fn buffer(method:usize,body:&[u8])->usize{enqueue([method|1<<8|body.len()<<16,0],abi::CAP_KIND_MEMORY,body,true)}
    pub fn response(id:usize)->Option<([usize;2],Vec<u8>)>{STATE.with(|s|{let s=s.borrow();
        s.replies.get(&id).map(|w|(*w,s.buffers[&id].to_vec()))})}
    pub fn dropped(id:usize)->usize{STATE.with(|s|s.borrow().drops.get(&id).copied().unwrap_or(0))}
    pub fn outstanding()->usize{STATE.with(|s|s.borrow().caps.len())}
    pub fn duplicates()->usize{STATE.with(|s|s.borrow().duplicate_drops)}
    pub fn receives()->Vec<(usize,u32)>{STATE.with(|s|s.borrow().receives.clone())}
    pub fn occupy(slot:usize){STATE.with(|s|s.borrow_mut().caps.insert(slot,Cap::Other(abi::CAP_KIND_ENDPOINT)));}
    pub fn kind(slot:usize)->usize{STATE.with(|s|match s.borrow().caps.get(&slot){
        Some(Cap::Memory(_))=>abi::CAP_KIND_MEMORY,Some(Cap::Reply(_))=>abi::CAP_KIND_REPLY,Some(Cap::Other(kind))=>*kind,None=>abi::CAP_KIND_NONE})}
    pub fn memory(slot:usize)->Option<(usize,usize)>{STATE.with(|s|{let s=s.borrow();
        let Cap::Memory(id)=s.caps.get(&slot)?else{return None};Some((s.buffers[id].as_ptr()as usize,4096))})}
    impl Endpoint{
        pub const SERVICE:Self=Self(1);
        pub fn recv_timeout(&self,receive:usize,ms:u32)->sys::Result<Received>{STATE.with(|s|{
            let mut s=s.borrow_mut();assert_eq!(self.0,1);assert_ne!(ms,0);assert!(!s.caps.contains_key(&receive),"receive overwrote a retained capability");
            assert!(s.current.is_none(),"a previous call has no reply or saved continuation");
            s.receives.push((receive,ms));s.now+=u64::from(ms)*1_000_000;
            let Some(request)=s.queue.pop_front()else{return Err(sys::Error::Other(abi::ERR_TIMEOUT))};
            if request.kind!=abi::CAP_KIND_NONE{let cap=if request.kind==abi::CAP_KIND_MEMORY{Cap::Memory(request.id)}else{Cap::Other(request.kind)};s.caps.insert(receive,cap);}
            if request.is_call{s.current=Some(request.id);}
            Ok(Received{data:request.words,sender:42,badge:0,cap_received:request.kind!=abi::CAP_KIND_NONE,is_call:request.is_call,irq:None})
        })}
        pub fn call_timeout(&self,_:&Message,_:usize,_:u32)->sys::Result<Received>{Err(sys::Error::Invalid)}
    }
    pub fn save_reply()->sys::Result<usize>{STATE.with(|s|{let mut s=s.borrow_mut();if s.save_fail{return Err(sys::Error::NoSlot)}
        let id=s.current.take().ok_or(sys::Error::Peer)?;let slot=100+id;assert!(s.caps.insert(slot,Cap::Reply(id)).is_none());Ok(slot)})}
    pub fn reply(message:&Message)->sys::Result<()>{STATE.with(|s|{let mut s=s.borrow_mut();let id=s.current.take().ok_or(sys::Error::Peer)?;
        if s.cancelled.contains(&id){return Err(sys::Error::Peer)}assert!(s.replies.insert(id,message.data).is_none());Ok(())})}
    pub fn reply_saved(slot:usize,message:&Message)->sys::Result<()>{STATE.with(|s|{let mut s=s.borrow_mut();
        let Some(Cap::Reply(id))=s.caps.remove(&slot)else{return Err(sys::Error::Rights)};
        if s.cancelled.contains(&id){return Err(sys::Error::Peer)}assert!(s.replies.insert(id,message.data).is_none());Ok(())})}
    pub fn drop_cap(slot:usize)->sys::Result<()>{STATE.with(|s|{let mut s=s.borrow_mut();match s.caps.remove(&slot){
        Some(Cap::Memory(id))=>{*s.drops.entry(id).or_default()+=1;Ok(())},Some(_)=>Ok(()),None=>{s.duplicate_drops+=1;Err(sys::Error::Rights)},
    }})}
    pub fn revoke(_:usize)->sys::Result<usize>{Ok(0)}
}
pub mod mem{
    use super::*;
    pub struct Pages(Vec<u8>);
    impl Pages{
        pub fn new(size:usize)->Option<Self>{Some(Self(vec![0;size]))}
        pub fn as_slice(&self)->&[u8]{&self.0}
        pub fn as_mut_slice(&mut self)->&mut[u8]{&mut self.0}
        pub fn share(&self)->sys::Result<usize>{Err(sys::Error::Invalid)}
    }
    pub struct Mapping{address:usize,length:usize}
    impl Mapping{
        pub fn new(slot:usize)->sys::Result<Self>{let(address,length)=ipc::memory(slot).ok_or(sys::Error::Rights)?;Ok(Self{address,length})}
        pub fn len(&self)->usize{self.length}
        pub fn as_slice(&self)->&[u8]{unsafe{std::slice::from_raw_parts(self.address as*const u8,self.length)}}
        pub fn as_mut_slice(&mut self)->&mut[u8]{unsafe{std::slice::from_raw_parts_mut(self.address as*mut u8,self.length)}}
    }
}
pub mod dev{pub fn cap_info(slot:usize)->(usize,usize,usize){(super::ipc::kind(slot),0,0)}}
pub mod time{pub fn monotonic_ns()->u64{super::ipc::clock()}}
pub mod network{pub const BAND_2GHZ:u8=1;pub const BAND_2GHZ_OFDM:u8=4;}
