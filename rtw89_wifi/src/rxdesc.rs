// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! Bounded AX RX descriptor decoding from the pinned core.c/core.h/txrx.h and PCIe layouts.

pub const BUFFER_BYTES: usize = 11454 + 40 + 4;
pub const TYPE_WIFI: u8 = 0;
pub const TYPE_C2H: u8 = 10;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error { Truncated, Length, Fragment, Descriptor, Buffer }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Packet {
    pub offset: usize, pub length: usize, pub kind: u8, pub long: bool,
    pub crc_error: bool, pub icv_error: bool, pub hardware_decrypted: bool,
}
impl Packet { pub fn bytes<'a>(&self, buffer: &'a [u8]) -> Option<&'a [u8]> { buffer.get(self.offset..self.offset.checked_add(self.length)?) } }

fn word(bytes: &[u8], at: usize) -> Result<u32,Error> {
    let raw=bytes.get(at..at.checked_add(4).ok_or(Error::Length)?).ok_or(Error::Truncated)?;
    Ok(u32::from_le_bytes(raw.try_into().unwrap()))
}

/// Parse one complete packet; fragmented chains are dropped explicitly, never presented as complete data.
pub fn parse(bytes: &[u8]) -> Result<Packet,Error> {
    let info=word(bytes,0)?;
    let written=(info&0x3fff)as usize;
    if written<20 || written>BUFFER_BYTES || written>bytes.len() { return Err(Error::Length); }
    if info&0xc000!=0xc000 { return Err(Error::Fragment); }
    let w0=word(bytes,4)?;
    let long=w0&(1<<31)!=0;
    let descriptor=if long{32}else{16};
    let shift=((w0>>14)&3)as usize*2;
    let driver=((w0>>28)&7)as usize*8;
    let offset=4+descriptor+shift+driver;
    let length=(w0&0x3fff)as usize;
    if offset>written || length==0 || offset.checked_add(length).is_none_or(|end|end>written) { return Err(Error::Descriptor); }
    let flags=word(bytes,16)?;
    Ok(Packet{offset,length,kind:((w0>>24)&15)as u8,long,
        crc_error:flags&(1<<9)!=0,icv_error:flags&(1<<10)!=0,hardware_decrypted:flags&4!=0})
}

/// RTL8852BE RPQ uses the four-byte report, not the eight-byte format of newer chips.
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub struct Report { pub polluted:bool,pub sequence:u16,pub status:u8,pub queue:u8,pub mac_id:u8 }
pub fn report(bytes:&[u8])->Result<Report,Error>{
    if bytes.len()!=4{return Err(Error::Length);}
    let raw=word(bytes,0)?;
    Ok(Report{polluted:raw>>31!=0,sequence:((raw>>16)&0x7fff)as u16,status:((raw>>13)&7)as u8,queue:((raw>>8)&31)as u8,mac_id:raw as u8})
}
