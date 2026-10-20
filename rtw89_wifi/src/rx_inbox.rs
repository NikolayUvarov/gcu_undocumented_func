//! Caller-owned copies for the single RX dispatcher; no packet allocation or DMA ownership is transferred.

use crate::{rx,rxdesc};

pub const SLOTS:usize=4;
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub struct Metadata { pub channel:rx::Channel,pub packet:rxdesc::Packet }
#[derive(Clone,Copy,Debug)]
pub struct Record<'a> { pub(crate) metadata:Metadata,pub(crate) buffer:&'a[u8] }
impl Record<'_> {
    pub fn metadata(&self)->Metadata { self.metadata }
    pub fn buffer(&self)->&[u8] { self.buffer }
    pub fn bytes(&self)->&[u8] { self.metadata.packet.bytes(self.buffer).unwrap() }
}
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) enum Error { Full,Packet }

/// Allocate this bank outside the driver's small native stack and lend it for the entire runtime session.
/// A bank can be attached once; old bytes cannot be reinterpreted as records from a new firmware session.
pub struct Inbox { buffers:[[u8;rx::BUFFER_BYTES];SLOTS],metadata:[Option<Metadata>;SLOTS],head:usize,count:usize,session:Option<u64> }
impl Inbox {
    pub const fn new()->Self { Self{buffers:[[0;rx::BUFFER_BYTES];SLOTS],metadata:[None;SLOTS],head:0,count:0,session:None} }
    pub fn pending(&self)->usize { self.count }
    pub fn front(&self)->Option<Record<'_>> {
        let metadata=self.metadata[self.head]?;
        Some(Record{metadata,buffer:&self.buffers[self.head][..metadata.packet.offset+metadata.packet.length]})
    }
    pub fn consume(&mut self) {
        if self.count!=0 { self.metadata[self.head]=None;self.head=(self.head+1)%SLOTS;self.count-=1; }
    }
    pub(crate) fn bind(&mut self,session:u64)->bool {
        if session==0||self.session.is_some()||self.count!=0{return false}
        self.session=Some(session);true
    }
    pub(crate) fn push(&mut self,metadata:Metadata,buffer:&[u8])->Result<(),Error> {
        let end=metadata.packet.offset.checked_add(metadata.packet.length).filter(|&v|v<=rx::BUFFER_BYTES&&v<=buffer.len()).ok_or(Error::Packet)?;
        if metadata.packet.length==0{return Err(Error::Packet)}
        if self.count==SLOTS{return Err(Error::Full)}
        let at=(self.head+self.count)%SLOTS;
        self.buffers[at][..end].copy_from_slice(&buffer[..end]);self.metadata[at]=Some(metadata);self.count+=1;Ok(())
    }
}
