// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022, 2024 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! Complete bounded firmware events and ACK identities; DMA consumption alone is not an ACK.

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub enum Error { Truncated, Length }
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub struct Identity { pub category:u8,pub class:u8,pub function:u8,pub sequence:u8 }
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub enum Ack { Received(Identity), Done{command:Identity,status:u8} }
impl Ack {
    pub fn matches(self,expected:Identity)->bool { match self{Self::Received(id)=>id==expected,Self::Done{command,..}=>command==expected} }
}
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub struct Event<'a> { pub category:u8,pub class:u8,pub function:u8,pub bytes:&'a[u8] }

pub fn parse(bytes:&[u8])->Result<Event<'_>,Error>{
    if bytes.len()<8{return Err(Error::Truncated);}
    let w0=u32::from_le_bytes(bytes[0..4].try_into().unwrap());
    let w1=u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    let length=(w1&0x3fff)as usize;
    if length<8||length!=bytes.len(){return Err(Error::Length);}
    Ok(Event{category:(w0&3)as u8,class:((w0>>2)&63)as u8,function:((w0>>8)&255)as u8,bytes})
}
impl Event<'_>{
    /// Receipt and completion ACKs have different sequence positions; a completion status is preserved verbatim.
    pub fn ack(self)->Result<Option<Ack>,Error>{
        if self.category!=1||self.class!=0||self.function>1{return Ok(None);}
        if self.bytes.len()<12{return Err(Error::Truncated);}
        let raw=u32::from_le_bytes(self.bytes[8..12].try_into().unwrap());
        let sequence=if self.function==0{(raw>>16)as u8}else{(raw>>24)as u8};
        let command=Identity{category:(raw&3)as u8,class:((raw>>2)&63)as u8,function:((raw>>8)&255)as u8,sequence};
        Ok(Some(if self.function==0{Ack::Received(command)}else{Ack::Done{command,status:(raw>>16)as u8}}))
    }
}
