// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! RTL8852B raw MPDU descriptors and independent RPQ/C2H status decoding.

pub(crate) const MAX_FRAME: usize = 2346;
pub(crate) const WD_BYTES: usize = 64;
pub(crate) const SELECTED_LIMIT:u8=1;
pub(crate) const LEGACY_RATES: [u8;12] = [2,4,11,22,12,18,24,36,48,72,96,108];

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) enum Error { Length, Frame, Rate, Count, Address, Report }
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) struct Frame {
    pub length:usize, pub header:usize, pub queue:usize, pub qsel:u8,
    pub tid_indicate:bool, pub group:bool, pub sequence:u16, pub rate:u8,
}

/// Metadata follows the actual header; hardware encryption, sequence replacement and aggregation stay off.
pub(crate) fn frame(bytes:&[u8],rate:u8,max_submissions:u8)->Result<Frame,Error>{
    if !(24..=MAX_FRAME).contains(&bytes.len()){return Err(Error::Length);}
    if !(1..=15).contains(&max_submissions){return Err(Error::Count);}
    let rate=LEGACY_RATES.iter().position(|v|*v==rate).ok_or(Error::Rate)? as u8;
    let fc=u16::from_le_bytes([bytes[0],bytes[1]]);
    if fc&3!=0{return Err(Error::Frame);}
    let kind=(fc>>2)&3;
    let (mut header,mut tid)=(24,0);
    let queue=match kind{
        0=>{if fc&0x0300!=0{return Err(Error::Frame);}if fc&0x8000!=0{header+=4;}4},
        2=>{
            if fc&0x0300==0x0300{header+=6;}
            if fc&0x0080!=0{
                tid=*bytes.get(header).ok_or(Error::Length)?&15;
                if tid>7{return Err(Error::Frame);}
                bytes.get(header+1).ok_or(Error::Length)?;
                if bytes[header]&0xe0!=0{return Err(Error::Frame);}
                header+=2;if fc&0x8000!=0{header+=4;}
            }
            match tid{0|3=>0,1|2=>1,4|5=>2,6|7=>3,_=>unreachable!()}
        },
        _=>return Err(Error::Frame),
    };
    if header>bytes.len(){return Err(Error::Length);}
    Ok(Frame{length:bytes.len(),header,queue,qsel:if queue==4{0x12}else{queue as u8},
        tid_indicate:matches!(tid,2|3|5|7),group:bytes[4]&1!=0,
        sequence:u16::from_le_bytes([bytes[22],bytes[23]])>>4,rate})
}

fn put(out:&mut[u8;WD_BYTES],at:usize,value:u32){out[at..at+4].copy_from_slice(&value.to_le_bytes());}

/// Selected hardware limit one, fixed legacy rate, no RTS, aggregation or hardware sequence substitution.
pub(crate) fn descriptor(meta:Frame,mac_id:u8,wp_sequence:u16,payload:u64)->Result<[u8;WD_BYTES],Error>{
    if mac_id>127||wp_sequence>0x7fff{return Err(Error::Address);}
    if payload==0||payload.checked_add(meta.length as u64).is_none_or(|end|end>1u64<<32){return Err(Error::Address);}
    let mut out=[0;WD_BYTES];let channel=if meta.queue==4{8}else{meta.queue as u32};
    put(&mut out,0,1<<22|channel<<16|((meta.header/2)as u32)<<11|1<<7);
    put(&mut out,8,u32::from(mac_id)<<24|u32::from(meta.tid_indicate)<<23|u32::from(meta.qsel)<<17|meta.length as u32);
    put(&mut out,12,u32::from(meta.sequence));
    put(&mut out,24,1<<30|u32::from(meta.rate)<<16|1<<10);
    put(&mut out,28,1<<31|u32::from(SELECTED_LIMIT)<<25|u32::from(meta.rate)<<16);
    put(&mut out,36,1<<10);put(&mut out,40,u32::from(wp_sequence&15));
    out[48..50].copy_from_slice(&(wp_sequence|0x8000).to_le_bytes());
    out[56..58].copy_from_slice(&(meta.length as u16).to_le_bytes());
    out[58..60].copy_from_slice(&0x8001u16.to_le_bytes());
    put(&mut out,60,payload as u32);Ok(out)
}

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) enum Status{Done,RetryLimit,Lifetime,MacIdDrop}
impl Status{
    fn decode(value:u8)->Result<Self,Error>{match value{0=>Ok(Self::Done),1=>Ok(Self::RetryLimit),2=>Ok(Self::Lifetime),3=>Ok(Self::MacIdDrop),_=>Err(Error::Report)}}
}
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) struct Report{pub sequence:u16,pub status:Status,pub qsel:u8,pub mac_id:u8}
pub(crate) fn rpq(bytes:&[u8])->Result<Report,Error>{
    let value=crate::rxdesc::report(bytes).map_err(|_|Error::Report)?;
    if value.polluted{return Err(Error::Report);}
    Ok(Report{sequence:value.sequence,status:Status::decode(value.status)?,qsel:value.queue,mac_id:value.mac_id})
}

/// Optional firmware diagnostic; RPQ itself has no attempt count and DMA has no over-the-air status.
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) struct FirmwareReport{pub sequence:u8,pub status:Status,pub raw_data_tx_count:u8}
pub(crate) fn firmware_report(bytes:&[u8])->Result<Option<FirmwareReport>,Error>{
    let event=crate::c2h::parse(bytes).map_err(|_|Error::Report)?;
    if (event.category,event.class,event.function)!=(1,9,1){return Ok(None);}
    if bytes.len()!=32{return Err(Error::Report);}
    let w2=u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    let w5=u32::from_le_bytes(bytes[20..24].try_into().unwrap());
    Ok(Some(FirmwareReport{sequence:((w2>>8)&15)as u8,status:Status::decode(((w2>>6)&3)as u8)?,raw_data_tx_count:((w5>>8)&63)as u8}))
}
