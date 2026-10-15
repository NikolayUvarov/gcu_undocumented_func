// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! Pure envelopes for RTL8852B firmware 0.29.29.15 BTC profile 105; no session or RFK permission is established.

use crate::c2h;

pub const MAX_BODY:usize=1800;
pub const SLOT_COUNT:usize=18;

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub enum Error{Envelope(c2h::Error),TooLong,Truncated,Length,Version,Slots,BtInfo}

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub struct Tdma{pub fields:[u8;8]}
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub struct Slot{pub duration:u16,pub table:u32,pub kind:u16}
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub struct BtVersion{pub coex_version:u32,pub firmware_version:u32,pub features:u32}

/// All 18 source slots are present; update_map is retained as received, not inferred command correlation.
#[derive(Clone,Copy,Debug)]
pub struct Slots<'a>{bytes:&'a[u8]}
impl Slots<'_>{
    pub fn count(&self)->u8{self.bytes[1]}
    pub fn update_map(&self)->u32{le32(&self.bytes[4..8])}
    pub fn get(&self,index:usize)->Option<Slot>{
        if index>=SLOT_COUNT{return None;}
        let b=&self.bytes[8+index*8..16+index*8];
        Some(Slot{duration:le16(&b[..2]),table:le32(&b[2..6]),kind:le16(&b[6..])})
    }
}

#[derive(Clone,Copy,Debug)]
pub enum Report<'a>{Tdma(Tdma),Slots(Slots<'a>),BtVersion(BtVersion),Other{wire_type:u8,canonical_type:Option<u8>,bytes:&'a[u8]}}

/// The complete stream is checked before iteration, so a malformed suffix cannot follow a partially accepted prefix.
#[derive(Clone,Copy,Debug)]
pub struct Reports<'a>{bytes:&'a[u8]}
impl<'a> Reports<'a>{
    fn parse(bytes:&'a[u8])->Result<Self,Error>{
        if bytes.len()>MAX_BODY{return Err(Error::TooLong);}
        let mut rest=bytes;
        while !rest.is_empty(){let(ty,body,next)=split(rest)?;validate(ty,body)?;rest=next;}
        Ok(Self{bytes})
    }
    pub fn iter(&self)->ReportsIter<'a>{ReportsIter{rest:self.bytes}}
}
pub struct ReportsIter<'a>{rest:&'a[u8]}
impl<'a> Iterator for ReportsIter<'a>{
    type Item=Report<'a>;
    fn next(&mut self)->Option<Self::Item>{
        if self.rest.is_empty(){return None;}
        let(ty,b,next)=split(self.rest).expect("validated immutable report envelope");self.rest=next;
        Some(match ty{
            1=>Report::Tdma(Tdma{fields:b[4..12].try_into().unwrap()}),
            2=>Report::Slots(Slots{bytes:b}),
            8=>Report::BtVersion(BtVersion{coex_version:le32(&b[4..8]),firmware_version:le32(&b[8..12]),features:le32(&b[12..16])}),
            _=>Report::Other{wire_type:ty,canonical_type:canonical(ty),bytes:b},
        })
    }
    fn size_hint(&self)->(usize,Option<usize>){(0,Some(self.rest.len()/3))}
}
impl core::iter::FusedIterator for ReportsIter<'_>{}

/// Explicit classification keeps BT state changes and overflow visible to the session owner.
#[derive(Clone,Copy,Debug)]
pub enum Event<'a>{
    Reports(Reports<'a>),BtInfo(&'a[u8;8]),Scoreboard(&'a[u8]),BtRegister(&'a[u8]),RunInfo(&'a[u8]),
    Psd(&'a[u8]),DeviceInfo(&'a[u8]),Overflow(&'a[u8]),Loopback(&'a[u8]),
    Other{category:u8,class:u8,function:u8,bytes:&'a[u8]},
}

/// The caller must select the exact pinned firmware profile and process every event before granting fresh RFK work.
/// Report presence or MAC DONE alone is not evidence that a particular policy was applied to this startup session.
pub fn parse_105(bytes:&[u8])->Result<Event<'_>,Error>{
    let event=c2h::parse(bytes).map_err(Error::Envelope)?;let body=&event.bytes[8..];
    if body.len()>MAX_BODY{return Err(Error::TooLong);}
    if event.category!=2||event.class!=0x12{
        return Ok(Event::Other{category:event.category,class:event.class,function:event.function,bytes:body});
    }
    Ok(match event.function{
        0=>Event::Reports(Reports::parse(body)?),
        1=>{if body.len()!=8||body[1]!=6{return Err(Error::BtInfo);}Event::BtInfo(body.try_into().unwrap())},
        2=>Event::Scoreboard(body),
        3=>{if body.len()<4{return Err(Error::Truncated);}Event::BtRegister(body)},
        4=>Event::RunInfo(body),5=>Event::Psd(body),6=>Event::DeviceInfo(body),
        7=>Event::Overflow(body),
        8=>{if body.is_empty(){return Err(Error::Truncated);}Event::Loopback(body)},
        _=>Event::Other{category:event.category,class:event.class,function:event.function,bytes:body},
    })
}

fn le16(b:&[u8])->u16{u16::from_le_bytes(b.try_into().unwrap())}
fn le32(b:&[u8])->u32{u32::from_le_bytes(b.try_into().unwrap())}
fn canonical(ty:u8)->Option<u8>{match ty{0..=5=>Some(ty),6..=12=>Some(ty+1),_=>None}}
fn split(b:&[u8])->Result<(u8,&[u8],&[u8]),Error>{
    if b.len()<3{return Err(Error::Truncated);}
    let end=3+usize::from(le16(&b[1..3]));
    if end>b.len(){return Err(Error::Length);}Ok((b[0],&b[3..end],&b[end..]))
}
fn validate(ty:u8,b:&[u8])->Result<(),Error>{
    let(length,version)=match ty{1=>(12,3),2=>(152,1),8=>(16,1),_=>return Ok(())};
    if b.len()!=length{return Err(Error::Length);}
    if b[0]!=version{return Err(Error::Version);}
    if ty==2&&b[1]as usize>SLOT_COUNT{return Err(Error::Slots);}Ok(())
}

#[cfg(test)]
#[path="../../tests/rtw89_btc_reports_cases.rs"]
mod tests;
