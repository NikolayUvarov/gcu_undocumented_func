// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! Initial NO_LINK MACID/port context; packet transmission and association remain separate.

use crate::{board::Board, btc_context::{self, Initialized}, btc_session::{self, DeviceIo, Session},
    channel_rfk, h2c, io::{self, Fault, FaultKind, RegisterIo, TimedIo},
    phy::{self, channel, Phy, PhyIo, State}, phycap::Capabilities};

const BAR_BYTES:usize=0x40020;
const TOTAL_US:u64=600_000;
const SELECTOR:usize=0xc04;
const WINDOW:usize=0x40000;
const DMAC:u32=0x18800000;
const CMAC:u32=0x18840000;
const PORT:usize=0xc400;
const CONTROL_MASK:[u32;8]=[1<<21,0,0,0,0xe00,0xffff0000,0xf0c0000,0];
const INITIAL_CMAC:[u32;8]=[4,0x400a0004,0,0,0,0x0e43000b,0,0xb8109];
const RX_FILTER:usize=0xce20;
const RX_FILTER_MASK:u32=!0x003f0000;
const RX_FILTER_HOST:u32=0x03004400;

#[path="station_filter.rs"]mod filter;
pub(crate) use filter::Filter;

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) enum Stage{Guard,Port,Tables,MacId,Role,Join,Cam,Control,Filter,Verify}
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) enum Cause{Register(Fault),Session(btc_session::Error),Owner(btc_context::Error)}
impl From<Fault> for Cause{fn from(value:Fault)->Self{Self::Register(value)}}
impl From<btc_session::Error> for Cause{fn from(value:btc_session::Error)->Self{Self::Session(value)}}
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) struct CheckFailure{pub cause:Fault,pub selector_restore:Option<Fault>}
#[derive(Debug)]
pub(crate) struct Failure{pub stage:Stage,pub cause:Cause,pub radio_stop:Option<Fault>,pub port_stop:Option<Fault>,pub selector_restore:Option<Fault>}
/// One initial client role with MACID/port/CAM zero; this is neither association nor TX permission.
#[derive(Debug)]
pub(crate) struct Prepared{attempt:u64,session:u64,channel:u8,policy_version:u64,mac:[u8;6],cmac:[u32;8],filter:Option<Filter>}
impl Prepared{
    pub(crate) fn matches(&self,tuned:&channel::Tuned,mac:[u8;6])->bool{
        self.attempt==tuned.attempt&&self.session==tuned.session&&self.channel==tuned.channel
            &&self.policy_version==tuned.policy_version&&self.mac==mac&&self.attempt!=0&&self.session!=0
    }
    pub(crate) fn mac_id(&self)->u8{0}
    pub(crate) fn mac(&self)->[u8;6]{self.mac}
    /// A software decision only; the runtime owner must establish hardware filtering and a stable receive epoch.
    pub(crate) fn filter(&self,all_beacons:bool,bssid:[u8;6],own:[u8;6])->Filter{
        Filter::new(self.mac,all_beacons,bssid,own)
    }
    pub(crate) fn current_filter(&self)->Option<Filter>{self.filter}
    /// Fixed station-register checks only; the caller supplies its time window and actual device/session ownership.
    /// This neither accepts active DMA ownership nor grants transmission; it preserves unrelated firmware fields.
    pub(crate) fn check_registers(&self,bus:&mut impl RegisterIo)->Result<(),CheckFailure>{
        let selector=io::read32(bus,SELECTOR).map_err(|cause|CheckFailure{cause,selector_restore:None})?;
        let result=verify(bus,&self.cmac).and_then(|()|if self.filter.is_some(){verify_filter(bus)}else{Ok(())});
        let restore=select(bus,selector).and_then(|()|bus.delay_us(0)).err();
        match(result,restore){(Ok(()),None)=>Ok(()),(Err(cause),selector_restore)=>Err(CheckFailure{cause,selector_restore}),
            (Ok(()),Some(cause))=>Err(CheckFailure{cause,selector_restore:Some(cause)})}
    }
    /// Revalidate before consuming this context into data activation; the stopped-TX PHY/RX contract remains unchanged.
    pub(crate) fn validate<B:DeviceIo>(&self,phy:&mut Phy<'_,Session<'_,'_,'_,B>>,board:&Board,tuned:&channel::Tuned,
        owner:&Initialized,timeout_us:u32)->Result<(),Failure>{
        let bare=|cause|Failure{stage:Stage::Guard,cause,radio_stop:None,port_stop:None,selector_restore:None};
        let session=phy.bus_between_stages().command_session();
        if timeout_us==0||timeout_us>10_000||phy.state()!=State::ChannelTuned||phy.identity()!=board.identity
            ||phy.bus_between_stages().bar_len()<BAR_BYTES||self.attempt!=phy.attempt()||self.session!=session
            ||!self.matches(tuned,board.identity.mac)||!owner.matches(self.attempt,session){return Err(bare(fault(0,0).into()));}
        let start=phy.bus_between_stages().now_us();let mut selector_restore=None;let mut stage=Stage::Guard;
        let result=(||{
            owner.validate(phy,timeout_us).map_err(Cause::Owner)?;
            budget(phy.bus_between_stages(),start,u64::from(timeout_us))?;
            phy::guard(phy.bus_between_stages(),&board.identity,phy::RegisterPhase::AfterDack)?;
            let left=budget(phy.bus_between_stages(),start,u64::from(timeout_us))?;stage=Stage::Verify;
            let mut timed=TimedIo::new(phy.bus_between_stages(),u64::from(left));
            self.check_registers(&mut timed).map_err(|e|{selector_restore=e.selector_restore;Cause::Register(e.cause)})?;
            budget(phy.bus_between_stages(),start,u64::from(timeout_us))?;Ok(())
        })();
        result.map_err(|cause|{
            let radio_stop=phy::disable_bb_rf(phy.bus_between_stages()).err();phy.mark_failed();
            let port_stop={let mut timed=TimedIo::new(phy.bus_between_stages(),1_000);
                field(&mut timed,PORT,4|1<<9|1<<12,0).and_then(|()|timed.delay_us(0)).err()};
            Failure{stage,cause,radio_stop,port_stop,selector_restore}
        })
    }
    /// Initial configuration only; no client-visible receive epoch exists yet and ordinary TX remains stopped.
    pub(crate) fn configure_filter<B:DeviceIo>(&mut self,phy:&mut Phy<'_,Session<'_,'_,'_,B>>,board:&Board,
        tuned:&channel::Tuned,owner:&Initialized,filter:Filter)->Result<(),Failure>{
        let bare=|cause|Failure{stage:Stage::Guard,cause,radio_stop:None,port_stop:None,selector_restore:None};
        if self.filter.is_some(){return Err(bare(fault(RX_FILTER,0).into()));}
        let start=phy.bus_between_stages().now_us();
        let deadline=start.checked_add(TOTAL_US).ok_or_else(||bare(Fault::new(FaultKind::Clock,0,0).into()))?;
        self.validate(phy,board,tuned,owner,10_000)?;
        let mut stage=Stage::Cam;let mut selector_restore=None;
        let result=(||{
            remaining(phy.bus_between_stages(),start)?;
            command(phy.bus_between_stages(),6,0,&cam(filter.own(),filter.bssid()),true,deadline)?;
            stage=Stage::Filter;
            phy.bus_between_stages().registers(deadline,|bus|field(bus,RX_FILTER,RX_FILTER_MASK,RX_FILTER_HOST))?;
            stage=Stage::Verify;
            phy.bus_between_stages().registers(deadline,|bus|{
                self.check_registers(bus).map_err(|e|{selector_restore=e.selector_restore;e.cause})?;
                verify_filter(bus)
            })?;
            live(phy,board,owner,start)?;Ok(())
        })();
        match result{Ok(())=>{self.filter=Some(filter);Ok(())},Err(cause)=>{
            let radio_stop=phy::disable_bb_rf(phy.bus_between_stages()).err();phy.mark_failed();
            let port_stop={let mut timed=TimedIo::new(phy.bus_between_stages(),1_000);
                field(&mut timed,PORT,4|1<<9|1<<12,0).and_then(|()|timed.delay_us(0)).err()};
            Err(Failure{stage,cause,radio_stop,port_stop,selector_restore})
        }}
    }
}
fn fault(at:usize,value:u32)->Fault{Fault::new(FaultKind::PowerState,at,value)}
fn remaining(bus:&mut impl RegisterIo,start:u64)->io::Result<u32>{budget(bus,start,TOTAL_US)}
fn budget(bus:&mut impl RegisterIo,start:u64,limit:u64)->io::Result<u32>{
    let used=bus.now_us().checked_sub(start).ok_or(Fault::new(FaultKind::Clock,0,0))?;
    limit.checked_sub(used).filter(|v|*v>0).map(|v|v as u32).ok_or(Fault::new(FaultKind::Timeout,0,0))
}
fn field(bus:&mut impl RegisterIo,at:usize,mask:u32,bits:u32)->io::Result<()>{
    io::modify32(bus,at,mask,bits)?;let value=io::read32(bus,at)?;
    if value&mask!=bits{return Err(fault(at,value));}Ok(())
}
fn field16(bus:&mut impl RegisterIo,at:usize,mask:u16,bits:u16)->io::Result<()>{
    let old=bus.read16(at)?;bus.write16(at,(old&!mask)|bits)?;let value=bus.read16(at)?;
    if value&mask!=bits{return Err(fault(at,u32::from(value)));}Ok(())
}
fn field8(bus:&mut impl RegisterIo,at:usize,mask:u8,bits:u8)->io::Result<()>{
    io::modify8(bus,at,mask,bits)?;let value=bus.read8(at)?;
    if value&mask!=bits{return Err(fault(at,u32::from(value)));}Ok(())
}
fn select(bus:&mut impl RegisterIo,address:u32)->io::Result<()>{
    bus.write32(SELECTOR,address)?;let value=io::read32(bus,SELECTOR)?;
    if value!=address{return Err(fault(SELECTOR,value));}Ok(())
}
fn port(bus:&mut impl RegisterIo)->io::Result<()>{
    field(bus,PORT,0x13c1b,0)?;
    field(bus,0xc414,0xffff,100)?;field8(bus,0xc590,0xff,0)?;
    field8(bus,0xca08,3,3)?;field16(bus,0xc426,0xff00,0)?;
    field(bus,0xc63c,1<<16|1,0)?;
    field(bus,0xc404,0xff,2)?;field(bus,0xc404,0x0fff0000,200<<16)?;
    field(bus,0xc408,0x0fff0000,0)?;field16(bus,0xc40e,0xfff,5)?;
    field(bus,0xc6a0,0x3f,0)?;field(bus,0xc568,0xfffffe,0)?;
    field(bus,PORT,4,4)?;bus.delay_us(20)?;
    field(bus,0xc40c,0xfff,160)?;field(bus,0xce84,0x7ff,0)?;bus.delay_us(0)
}
fn tables(bus:&mut impl RegisterIo)->io::Result<()>{
    for i in 0..4{select(bus,DMAC+4*i)?;bus.write32(WINDOW,0)?;if io::read32(bus,WINDOW)?!=0{return Err(fault(WINDOW,i));}}
    select(bus,CMAC)?;
    for(i,value)in INITIAL_CMAC.into_iter().enumerate(){bus.write32(WINDOW+4*i,value)?;}
    verify_table(bus,CMAC,&INITIAL_CMAC)
}
fn verify_table(bus:&mut impl RegisterIo,address:u32,words:&[u32])->io::Result<()>{
    select(bus,address)?;
    for(i,&wanted)in words.iter().enumerate(){let value=io::read32(bus,WINDOW+4*i)?;
        if value!=wanted{return Err(fault(WINDOW+4*i,value));}}
    bus.delay_us(0)
}
fn encode<const N:usize>(words:[u32;N],out:&mut[u8]){for(i,word)in words.into_iter().enumerate(){out[i*4..i*4+4].copy_from_slice(&word.to_le_bytes());}}
fn cam(mac:[u8;6],bssid:[u8;6])->[u8;60]{
    let hash=mac.into_iter().fold(0u8,|a,b|a^b);let peer_hash=bssid.into_iter().fold(0u8,|a,b|a^b);let mut out=[0;60];
    encode([0,0x40<<16,1|u32::from(hash)<<16|u32::from(peer_hash)<<24,0,u32::from_le_bytes([mac[0],mac[1],mac[2],mac[3]]),
        u32::from_le_bytes([mac[4],mac[5],bssid[0],bssid[1]]),u32::from_le_bytes([bssid[2],bssid[3],bssid[4],bssid[5]]),
        0,0,2<<16,0,0,8<<16,1|0x3f<<2|u32::from(u16::from_le_bytes([bssid[0],bssid[1]]))<<16,
        u32::from_le_bytes([bssid[2],bssid[3],bssid[4],bssid[5]])],&mut out);out
}
fn verify_filter(bus:&mut impl RegisterIo)->io::Result<()>{
    let value=io::read32(bus,RX_FILTER)?;
    if value&RX_FILTER_MASK!=RX_FILTER_HOST{return Err(fault(RX_FILTER,value));}bus.delay_us(0)
}
fn control(antenna:u8)->([u8;68],[u32;8]){
    let paths=if antenna==0{3}else{antenna};let mut words=[0u32;17];words[0]=0x80;
    words[1]=1<<21;words[9]=1<<21;words[13]=0xe00;
    words[6]=u32::from(paths)<<16|u32::from(paths==3)<<22;words[14]=0xffff0000;words[15]=0xf0c0000;
    let mut expected=INITIAL_CMAC;
    for i in 0..8{expected[i]=(expected[i]&!words[i+9])|words[i+1];}
    let mut out=[0;68];encode(words,&mut out);(out,expected)
}
fn verify(bus:&mut impl RegisterIo,expected:&[u32;8])->io::Result<()>{
    for(at,mask,wanted)in[(PORT,0x13e1f,4),(0x9eb0,1,0),(0xc660,0x30,0x10)]{
        let value=io::read32(bus,at)?;if value&mask!=wanted{return Err(fault(at,value));}}
    // Firmware owns unrelated control words after ROLE/JOIN; validate the fields written by this CCTL command.
    select(bus,CMAC)?;
    for i in 0..8{let value=io::read32(bus,WINDOW+4*i)?;
        if value&CONTROL_MASK[i]!=expected[i]&CONTROL_MASK[i]{return Err(fault(WINDOW+4*i,value));}}
    bus.delay_us(0)
}
fn live<B:DeviceIo>(phy:&mut Phy<'_,Session<'_,'_,'_,B>>,board:&Board,owner:&Initialized,start:u64)->Result<(),Cause>{
    let left=remaining(phy.bus_between_stages(),start)?;owner.validate(phy,left.min(10_000)).map_err(Cause::Owner)?;
    phy::guard(phy.bus_between_stages(),&board.identity,phy::RegisterPhase::AfterDack)?;
    remaining(phy.bus_between_stages(),start)?;Ok(())
}
fn command<B:DeviceIo>(bus:&mut Session<'_,'_,'_,B>,class:u8,function:u8,body:&[u8],done:bool,deadline:u64)->Result<(),Cause>{
    bus.command_before(h2c::Command{category:1,class,function},body,h2c::Acks{received:!done,done},deadline)?;Ok(())
}
/// The same calibrated exclusive device/session; initial local MAC only, no fabricated associated BSSID or security keys.
pub(crate) fn prepare<B:DeviceIo>(phy:&mut Phy<'_,Session<'_,'_,'_,B>>,board:&Board,tuned:&channel::Tuned,
    calibrated:&channel_rfk::Completed,owner:&Initialized,caps:&Capabilities)->Result<Prepared,Failure>{
    let bare=|cause|Failure{stage:Stage::Guard,cause,radio_stop:None,port_stop:None,selector_restore:None};
    let session=phy.bus_between_stages().command_session();let attempt=phy.attempt();
    if phy.state()!=State::ChannelTuned||phy.identity()!=board.identity||phy.bus_between_stages().bar_len()<BAR_BYTES
        ||tuned.attempt!=attempt||tuned.session!=session||caps.session()!=session||!calibrated.matches(tuned)
        ||!owner.matches(attempt,session){return Err(bare(fault(0,0).into()));}
    let start=phy.bus_between_stages().now_us();let deadline=start.checked_add(TOTAL_US).ok_or_else(||bare(Fault::new(FaultKind::Clock,0,0).into()))?;
    live(phy,board,owner,start).map_err(bare)?;
    let old=io::read32(phy.bus_between_stages(),PORT).map_err(|e|bare(e.into()))?;
    if old&(4|1<<9|1<<12)!=0{return Err(bare(fault(PORT,old).into()));}
    let selector=io::read32(phy.bus_between_stages(),SELECTOR).map_err(|e|bare(e.into()))?;
    let (payload,expected)=control(caps.antenna_tx());let mut stage=Stage::Port;
    let result=(||{
        phy.bus_between_stages().registers(deadline,|bus|port(bus))?;
        stage=Stage::Tables;phy.bus_between_stages().registers(deadline,|bus|tables(bus))?;
        stage=Stage::MacId;let mut pause=[0u8;32];pause[16]=1;
        command(phy.bus_between_stages(),9,8,&pause,false,deadline)?;
        stage=Stage::Role;command(phy.bus_between_stages(),8,4,&(1u32<<13).to_le_bytes(),true,deadline)?;
        stage=Stage::Join;command(phy.bus_between_stages(),8,0,&(1u32<<8|1<<26).to_le_bytes(),true,deadline)?;
        stage=Stage::Cam;command(phy.bus_between_stages(),6,0,&cam(board.identity.mac,[0;6]),true,deadline)?;
        stage=Stage::Control;command(phy.bus_between_stages(),5,2,&payload,true,deadline)?;
        stage=Stage::Verify;phy.bus_between_stages().registers(deadline,|bus|{verify(bus,&expected)?;select(bus,selector)})?;
        live(phy,board,owner,start)?;
        Ok(Prepared{attempt,session,channel:tuned.channel,policy_version:tuned.policy_version,mac:board.identity.mac,cmac:expected,filter:None})
    })();
    match result{Ok(value)=>Ok(value),Err(cause)=>{
        let radio_stop=phy::disable_bb_rf(phy.bus_between_stages()).err();phy.mark_failed();
        let port_stop={let mut timed=TimedIo::new(phy.bus_between_stages(),1_000);field(&mut timed,PORT,4|1<<9|1<<12,0).and_then(|()|timed.delay_us(0)).err()};
        let selector_restore={let mut timed=TimedIo::new(phy.bus_between_stages(),1_000);select(&mut timed,selector).and_then(|()|timed.delay_us(0)).err()};
        Err(Failure{stage,cause,radio_stop,port_stop,selector_restore})
    }}
}
