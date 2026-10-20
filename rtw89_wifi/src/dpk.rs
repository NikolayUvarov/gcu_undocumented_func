// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! Private initial 2 GHz/20 MHz DPK engine; typed admission is required; no native calibration-power provider exists.

use crate::{board::Board,btc_context::Initialized,dack::{self,BtcNotice,Calibration,RfkState},dpk_tables as tables,
    io::{self,Fault,FaultKind,RegisterIo,TimedIo},phy::{self,Phy,State,PHY_BASE,channel},rxdck::RxdckIo,tssi};

const RF_MASK:u32=0xfffff;
const TOTAL_US:u64=3_000_000;
const BB:[u32;3]=[0x2344,0x5800,0x7800];
const RF:[u32;11]=[0xde,0xdf,0x8b,0x90,0x97,0x85,0x1e,0,2,5,0x10005];
const KIP:[u32;3]=[0x813c,0x8124,0x8120];

struct Admission{channel:u8,tssi:[bool;2]}
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) enum Stage{Guard,BtcStart,SchedulerPause,WaitA,WaitB,Backup,Afe,PathA,PathB,Restore,SchedulerRestore,BtcStop,Verify}
#[derive(Debug)]
pub(crate) struct Failure{pub stage:Stage,pub cause:Fault,pub disable:[Option<Fault>;2],pub bb_restore:Option<Fault>,
    pub kip_restore:[Option<Fault>;2],pub rf_restore:[Option<Fault>;2],pub tssi_restore:[Option<Fault>;2],
    pub scheduler_restore:Option<Fault>,pub scheduler_status:Option<Fault>,pub radio_stop:Option<Fault>,pub btc_stop:Option<Fault>,
    pub nctl_uncertain:bool,pub restore_skipped:bool,pub disable_skipped:bool}
impl Failure{fn guard(cause:Fault)->Self{Self{stage:Stage::Guard,cause,disable:[None;2],bb_restore:None,kip_restore:[None;2],rf_restore:[None;2],tssi_restore:[None;2],scheduler_restore:None,scheduler_status:None,radio_stop:None,btc_stop:None,nctl_uncertain:false,restore_skipped:false,disable_skipped:false}}}
#[derive(Clone,Copy,Debug,Default,Eq,PartialEq)]
pub(crate) struct PathResult{pub txagc:u8,pub thermal:u8,pub correlation_index:u8,pub correlation:u8,pub dc:[u16;2]}
#[derive(Debug,Eq,PartialEq)]
pub(crate) struct ResultData{pub channel:u8,pub gain_scale:u8,pub paths:[PathResult;2]}
#[derive(Debug)]
pub(crate) struct Completed{attempt:u64,session:u64,channel:u8,policy_version:u64,pub data:ResultData}
impl Completed{
    pub(crate) fn matches(&self,tuned:&channel::Tuned)->bool{self.attempt!=0&&self.session!=0&&self.attempt==tuned.attempt&&self.session==tuned.session
        &&self.channel==tuned.channel&&self.policy_version==tuned.policy_version&&tuned.access==channel::Access::InitiateRadiation}
}
#[derive(Default)]
struct Saved{bb:[Option<u32>;3],rf:[[Option<u32>;11];2],kip:[[Option<u32>;3];2],tssi:[Option<u32>;2]}

fn fault(at:u32,value:u32)->Fault{Fault::new(FaultKind::PowerState,PHY_BASE+at as usize,value)}
fn remaining(bus:&mut impl RegisterIo,start:u64)->io::Result<u32>{
    let elapsed=bus.now_us().checked_sub(start).ok_or(Fault::new(FaultKind::Clock,0,0))?;
    TOTAL_US.checked_sub(elapsed).filter(|n|*n>0).map(|n|n as u32).ok_or(Fault::new(FaultKind::Timeout,0,0))
}
fn field(bus:&mut impl RegisterIo,at:u32,mask:u32,value:u32)->io::Result<()>{
    if mask==0||value>mask>>mask.trailing_zeros()||(value<<mask.trailing_zeros())&!mask!=0{return Err(Fault::new(FaultKind::Access,at as usize,value));}
    if mask==u32::MAX{bus.write32(PHY_BASE+at as usize,value)}else{io::modify32(bus,PHY_BASE+at as usize,mask,value<<mask.trailing_zeros())}
}
fn read(bus:&mut impl RegisterIo,at:u32,mask:u32)->io::Result<u32>{Ok((io::read32(bus,PHY_BASE+at as usize)?&mask)>>mask.trailing_zeros())}
fn verified(bus:&mut impl RegisterIo,at:u32,mask:u32,value:u32)->io::Result<()>{field(bus,at,mask,value)?;let actual=read(bus,at,mask)?;if actual!=value{return Err(fault(at,actual));}bus.delay_us(0)}
fn rf_restore(bus:&mut impl RegisterIo,path:u8,at:u32,value:u32)->io::Result<()>{phy::rf_write_masked(bus,path,at,RF_MASK,value)?;let actual=phy::rf_read(bus,path,at,RF_MASK)?;if actual!=value{return Err(fault(at,actual));}bus.delay_us(0)}
fn table(bus:&mut impl RegisterIo,values:&[(u32,u32,u32)])->io::Result<()>{for &(at,mask,value)in values{field(bus,at,mask,value)?;}bus.delay_us(0)}
fn notice(state:RfkState)->BtcNotice{BtcNotice{calibration:Calibration::Dpk,state,phy_map:0x10}}
fn scheduler(bus:&mut impl RxdckIo,budget:u32)->io::Result<()>{
    let start=bus.now_us();bus.scheduler_zero(budget)?;
    let elapsed=bus.now_us().checked_sub(start).ok_or(Fault::new(FaultKind::Clock,0xc348,0))?;
    if elapsed>=u64::from(budget){return Err(Fault::new(FaultKind::Timeout,0xc348,budget));}Ok(())
}
fn status(bus:&mut impl RegisterIo)->io::Result<()>{
    let value=io::read32(bus,0xc348)?;if value&0xffff!=0{return Err(Fault::new(FaultKind::PowerState,0xc348,value));}
    io::poll32(bus,0xc6ec,1<<7,0,8,80_000)?;bus.delay_us(0)
}
fn wait_rx(bus:&mut impl RegisterIo,path:u8)->io::Result<()>{
    let mut timed=TimedIo::new(bus,5_000);
    for _ in 0..=2_500{let mode=phy::rf_read(&mut timed,path,0,0xf0000)?;timed.delay_us(0)?;if mode!=2{return Ok(())}timed.delay_us(2)?;}
    Err(Fault::new(FaultKind::Timeout,0,2))
}
fn one_shot(bus:&mut impl RegisterIo,path:u8,id:u8,pending:&mut bool)->io::Result<()>{
    *pending=true;field(bus,0x8000,u32::MAX,(u32::from(id)<<8)|(0x19+(u32::from(path)<<4)))?;
    io::poll32(bus,PHY_BASE+0xbff8,255,0x55,1,20_000)?;bus.delay_us(1)?;
    field(bus,0x80d4,u32::MAX,0x30000)?;io::poll32(bus,PHY_BASE+0x80fc,0xffff,0x8000,1,2_000)?;
    field(bus,0x8010,255,0)?;bus.delay_us(0)?;*pending=false;Ok(())
}
fn txagc(bus:&mut impl RegisterIo,path:u8,value:u8,pending:&mut bool)->io::Result<()>{
    phy::rf_write_masked(bus,path,0x10001,RF_MASK,u32::from(value))?;field(bus,0x5864,1<<29,1)?;
    one_shot(bus,path,0x19,pending)?;field(bus,0x5864,1<<29,0)
}
fn rxagc(bus:&mut impl RegisterIo,path:u8,pending:&mut bool)->io::Result<()>{
    let mode=phy::rf_read(bus,path,0,RF_MASK)?;field(bus,0x8078,RF_MASK,mode)?;field(bus,0x5864,1<<29,1)?;
    one_shot(bus,path,0x15,pending)?;field(bus,0x5864,1<<29,0)?;field(bus,0x80d4,0xf0000,8)
}
fn offset(bus:&mut impl RegisterIo,path:u8,gain:i8,pending:&mut bool)->io::Result<u8>{
    let old=phy::rf_read(bus,path,0x10001,RF_MASK)?;if old>0x3f{return Err(fault(0x10001,old));}
    let value=(old as i32-i32::from(gain)).clamp(0x2e,0x3f)as u8;txagc(bus,path,value,pending)?;Ok(value)
}
fn signed12(value:u32)->u16{(((value&0xfff)as i16)<<4>>4).unsigned_abs()}
fn sync(bus:&mut impl RegisterIo,path:u8,result:&mut PathResult,pending:&mut bool)->io::Result<u16>{
    one_shot(bus,path,0x10,pending)?;field(bus,0x80d4,0x3f0000,0)?;
    result.correlation_index=read(bus,0x80fc,255)?as u8;result.correlation=read(bus,0x80fc,0xff00)?as u8;
    field(bus,0x80d4,0x3f0000,9)?;result.dc=[signed12(read(bus,0x80fc,0xfff0000)?),signed12(read(bus,0x80fc,0xfff)?)];
    if result.dc[0]>200||result.dc[1]>200||result.correlation<170{return Err(fault(0x80fc,(u32::from(result.dc[0])<<16)|u32::from(result.dc[1])));}
    field(bus,0x80d4,0x3f0000,0)?;read(bus,0x80fc,0xfff0000).map(|v|v as u16)
}
fn dgain_offset(value:u16)->i8{
    const BOUNDS:[u16;14]=[0xaa5,0x97d,0x875,0x789,0x6b7,0x5fc,0x556,0x4c1,0x43d,0x3c7,0x35e,0x2ac,0x262,0x220];
    for(index,bound)in BOUNDS.into_iter().enumerate(){if value>=bound{return 6-index as i8;}}-8
}
fn pas_high(bus:&mut impl RegisterIo)->io::Result<bool>{
    field(bus,0x80d4,0xff0000,6)?;field(bus,0x80bc,1<<14,0)?;field(bus,0x80c0,0xff0000,8)?;
    let mut energy=[0u64;2];for(index,select)in [0,31].into_iter().enumerate(){
        field(bus,0x80c0,0xff000000,select)?;
        let i=u64::from(signed12(read(bus,0x80fc,0xffff0000)?));let q=u64::from(signed12(read(bus,0x80fc,0xffff)?));energy[index]=i*i+q*q;
    }Ok(energy[0]>=energy[1]*8/5)
}
fn agc(bus:&mut impl RegisterIo,path:u8,result:&mut PathResult,pending:&mut bool)->io::Result<u8>{
    let mut step=0;let mut attempts=0;let mut limited=false;let mut gain=0x38;let mut dgain=0;let mut loss=0;
    for _ in 0..200{
        if attempts>=6{return Err(fault(0x80fc,attempts));}
        match step{
            0=>{dgain=sync(bus,path,result,pending)?;step=if limited{2}else{1};},
            1=>{let rxbb=phy::rf_read(bus,path,0,0x3e0)?as i32;let adjust=dgain_offset(dgain);let next=rxbb+i32::from(adjust);
                if !(0..=31).contains(&next){limited=true;}
                phy::rf_write_masked(bus,path,0,0x3e0,next.clamp(0,31)as u32)?;
                if adjust!=0||attempts==0{field(bus,0x813c+(u32::from(path)<<8),4,1)?;field(bus,0x813c+(u32::from(path)<<8),1,1)?;}
                step=if !(342..=1922).contains(&dgain){0}else{2};attempts+=1;},
            2=>{field(bus,0x81ac+(u32::from(path)<<8),0xff000000,0x90)?;one_shot(bus,path,0x13,pending)?;
                field(bus,0x80d4,0x3f0000,6)?;field(bus,0x80bc,1<<14,1)?;loss=read(bus,0x80fc,0xf0)?as u8;
                step=if (loss==0&&pas_high(bus)?)||loss>=7{3}else if loss==0{4}else{5};},
            3=>{if gain==0x2e{return Ok(gain)}gain=offset(bus,path,3,pending)?;step=2;attempts+=1;},
            4=>{if gain==0x3f{return Ok(gain)}gain=offset(bus,path,-2,pending)?;step=2;attempts+=1;},
            5=>return offset(bus,path,loss as i8,pending),
            _=>return Err(fault(0x80fc,step)),
        }
        bus.delay_us(0)?;
    }Err(fault(0x80fc,200))
}
fn gain_scale(bus:&mut impl RegisterIo)->io::Result<u8>{
    let value=read(bus,0x44a0,u32::MAX)?;
    if ((value>>12)&31)+(value&127)>=44{
        for at in [0x81bc,0x82bc]{let actual=read(bus,at,0x7fffff)?;if actual!=0x7f7f7f{return Err(fault(at,actual));}}Ok(0x7f)
    }else{Ok(0x5b)}
}
fn path(bus:&mut impl RegisterIo,path:u8,scale:u8,pending:&mut bool)->io::Result<PathResult>{
    let base=u32::from(path)<<8;
    phy::rf_write_masked(bus,path,5,1,0)?;phy::rf_write_masked(bus,path,0x10005,1,0)?;
    field(bus,0x8008,u32::MAX,0x80)?;field(bus,0x8088,u32::MAX,0x807f030a)?;field(bus,0x8120+base,u32::MAX,0xce000a08)?;
    txagc(bus,path,0x38,pending)?;
    for(at,mask,value)in [(0,RF_MASK,0x50220),(0x83,255,0xf2),(0xdf,1<<12,1),(0x9e,1<<8,1),(0xde,4,1),(0x1a,0x7000,1),(0x1a,0xc00,0),
        (0x8f,0xc00,3),(0x93,15,0),(0x92,1,0),(0x92,1,1)]{phy::rf_write_masked(bus,path,at,mask,value)?;}
    bus.delay_us(1_000)?;field(bus,0x806c,6,1)?;one_shot(bus,path,0x16,pending)?;rxagc(bus,path,pending)?;
    field(bus,0x81ac+base,0xff000000,0x90)?;
    let mut result=PathResult::default();result.txagc=agc(bus,path,&mut result,pending)?;
    for value in [1,0,1]{phy::rf_write_masked(bus,path,0x42,1<<19,value)?;}bus.delay_us(200)?;
    result.thermal=phy::rf_read(bus,path,0x42,0x7e)?as u8;
    field(bus,0x80a0,3,0)?;field(bus,0x80a0,0x1f00,3)?;field(bus,0x8070,0xf0000000,1)?;one_shot(bus,path,0x11,pending)?;
    phy::rf_write_masked(bus,path,0,0xf0000,3)?;
    field(bus,0x8104+base,1<<8,0)?;field(bus,0x81c4+base,0x3f00,u32::from(result.txagc))?;field(bus,0x81b4+base,0x1ff0000,0x78)?;
    field(bus,0x81dc+base,1<<16,1)?;field(bus,0x81dc+base,1<<16,0)?;
    field(bus,0x81bc+base,u32::MAX,if scale==0x7f{0x7f7f7f}else{0x5b5b5b})?;
    let order=3>>read(bus,0x80a0,3)?;field(bus,0x81bc+base,0x06000000,order)?;
    field(bus,0x81a0+base,u32::MAX,0)?;field(bus,0x8070,1<<31,0)?;
    field(bus,0x81bc+base,0xff000000,(order<<1)|1)?;bus.delay_us(0)?;Ok(result)
}

fn save(bus:&mut impl RegisterIo,saved:&mut Saved,tssi:[bool;2])->io::Result<()>{
    for(i,at)in BB.into_iter().enumerate(){saved.bb[i]=Some(read(bus,at,u32::MAX)?);}
    for path in 0..2{
        for(i,at)in KIP.into_iter().enumerate(){saved.kip[path][i]=Some(read(bus,at+((path as u32)<<8),u32::MAX)?);}
        for(i,at)in RF.into_iter().enumerate(){saved.rf[path][i]=Some(phy::rf_read(bus,path as u8,at,RF_MASK)?);}
        if tssi[path]{let at=0x5818+((path as u32)<<13);saved.tssi[path]=Some(read(bus,at,1<<30)?);field(bus,at,1<<30,1)?;}
    }bus.delay_us(0)
}
fn restore(bus:&mut impl RegisterIo,saved:&Saved)->io::Result<()>{
    table(bus,tables::AFE_RESTORE)?;
    for(i,at)in BB.into_iter().enumerate(){if let Some(value)=saved.bb[i]{verified(bus,at,u32::MAX,value)?;}}
    for path in 0..2{
        table(bus,tables::KIP)?;field(bus,0x81c8+((path as u32)<<8),1<<15,1)?;
        for(i,at)in KIP.into_iter().enumerate(){if let Some(value)=saved.kip[path][i]{verified(bus,at+((path as u32)<<8),u32::MAX,value)?;}}
        for(i,at)in RF.into_iter().enumerate(){if let Some(value)=saved.rf[path][i]{rf_restore(bus,path as u8,at,value)?;}}
        if let Some(value)=saved.tssi[path]{verified(bus,0x5818+((path as u32)<<13),1<<30,value)?;}
    }bus.delay_us(0)
}
fn first(slot:&mut Option<Fault>,result:io::Result<()>){if let Err(error)=result{if slot.is_none(){*slot=Some(error);}}}
fn cleanup(bus:&mut impl RegisterIo,saved:&Saved,changed:bool,failure:&mut Failure){
    if changed{
        for path in 0..2{let mut timed=TimedIo::new(bus,1_000);failure.disable[path]=verified(&mut timed,0x81bc+((path as u32)<<8),1<<24,0).err();}
        let mut timed=TimedIo::new(bus,10_000);
        for &(at,mask,value)in tables::AFE_RESTORE{first(&mut failure.bb_restore,field(&mut timed,at,mask,value));}
        for(i,at)in BB.into_iter().enumerate(){if let Some(value)=saved.bb[i]{first(&mut failure.bb_restore,verified(&mut timed,at,u32::MAX,value));}}first(&mut failure.bb_restore,timed.delay_us(0));
    }
    for path in 0..2{
        if changed{
            let mut timed=TimedIo::new(bus,10_000);
            for &(at,mask,value)in tables::KIP{first(&mut failure.kip_restore[path],field(&mut timed,at,mask,value));}
            first(&mut failure.kip_restore[path],field(&mut timed,0x81c8+((path as u32)<<8),1<<15,1));
            for(i,at)in KIP.into_iter().enumerate(){if let Some(value)=saved.kip[path][i]{first(&mut failure.kip_restore[path],verified(&mut timed,at+((path as u32)<<8),u32::MAX,value));}}first(&mut failure.kip_restore[path],timed.delay_us(0));
            let mut timed=TimedIo::new(bus,10_000);
            for(i,at)in RF.into_iter().enumerate(){if let Some(value)=saved.rf[path][i]{first(&mut failure.rf_restore[path],rf_restore(&mut timed,path as u8,at,value));}}first(&mut failure.rf_restore[path],timed.delay_us(0));
        }
        if let Some(value)=saved.tssi[path]{let mut timed=TimedIo::new(bus,1_000);failure.tssi_restore[path]=verified(&mut timed,0x5818+((path as u32)<<13),1<<30,value).err();}
    }
}

fn engine(bus:&mut impl RxdckIo,admission:Admission,start:u64)->Result<ResultData,Failure>{
    let mut failure=Failure::guard(fault(0,0));
    if !(1..=13).contains(&admission.channel)||admission.tssi!=[true;2]{return Err(failure);}
    let mut saved=Saved::default();let mut changed=false;let mut btc=false;let mut stopped=false;
    let mut paused=false;let mut resumed=false;let mut pending=false;
    let result=(||{
        failure.stage=Stage::Guard;let left=remaining(bus,start)?;status(&mut TimedIo::new(bus,u64::from(left)))?;
        let left=remaining(bus,start)?;let scale=gain_scale(&mut TimedIo::new(bus,u64::from(left)))?;
        failure.stage=Stage::BtcStart;let left=remaining(bus,start)?;btc=true;dack::notify(bus,notice(RfkState::Start),left.min(100_000))?;
        failure.stage=Stage::SchedulerPause;let left=remaining(bus,start)?;scheduler(bus,left.min(1_000_000))?;paused=true;
        let left=remaining(bus,start)?;let mut timed=TimedIo::new(bus,u64::from(left));status(&mut timed)?;
        for path in 0..2{failure.stage=if path==0{Stage::WaitA}else{Stage::WaitB};wait_rx(&mut timed,path)?;}
        failure.stage=Stage::Backup;save(&mut timed,&mut saved,admission.tssi)?;
        failure.stage=Stage::Afe;changed=true;table(&mut timed,tables::AFE)?;
        let mut paths=[PathResult::default();2];
        for p in 0..2{failure.stage=if p==0{Stage::PathA}else{Stage::PathB};paths[p]=path(&mut timed,p as u8,scale,&mut pending)?;}
        failure.stage=Stage::Restore;restore(&mut timed,&saved)?;
        failure.stage=Stage::SchedulerRestore;let left=remaining(bus,start)?;resumed=true;
        if let Err(error)=scheduler(bus,left.min(1_000_000)){failure.scheduler_restore=Some(error);return Err(error)}
        let left=remaining(bus,start)?;status(&mut TimedIo::new(bus,u64::from(left)))?;
        failure.stage=Stage::BtcStop;let left=remaining(bus,start)?;stopped=true;
        if let Err(error)=dack::notify(bus,notice(RfkState::Stop),left.min(100_000)){failure.btc_stop=Some(error);return Err(error)}
        remaining(bus,start)?;Ok(ResultData{channel:admission.channel,gain_scale:scale,paths})
    })();
    match result{
        Ok(data)=>Ok(data),
        Err(cause)=>{failure.cause=cause;failure.nctl_uncertain=pending;failure.restore_skipped=pending||stopped;failure.disable_skipped=pending||stopped;
            // STOP may have released ownership; only independent radio shutdown remains authorized.
            if stopped{failure.radio_stop=phy::disable_bb_rf(bus).err();return Err(failure)}
            if !pending{cleanup(bus,&saved,changed,&mut failure);}
            if btc&&pending{failure.radio_stop=phy::disable_bb_rf(bus).err();}
            if paused&&!resumed{failure.scheduler_restore=scheduler(bus,1_000_000).err();}
            if btc{failure.scheduler_status=status(&mut TimedIo::new(bus,80_000)).err();if !pending{failure.radio_stop=phy::disable_bb_rf(bus).err();}}
            if btc&&!stopped{failure.btc_stop=dack::notify(bus,notice(RfkState::Stop),100_000).err();}Err(failure)}
    }
}

/// Consume separate RFK transmission authority after actual same-channel TSSI and initialized coexistence.
/// No native authority provider exists until raw DPK levels and platform SAR/EIRP are resolved.
pub(crate) fn run<B:RxdckIo>(phy:&mut Phy<'_,B>,board:&Board,tuned:&channel::Tuned,
    initialized:&Initialized,tssi:&tssi::Completed,authority:channel::CalibrationTransmit)->Result<Completed,Failure>{
    let attempt=phy.attempt();let session=phy.bus_between_stages().command_session();
    if phy.state()!=State::ChannelTuned||!initialized.matches(attempt,session)||tuned.attempt!=attempt||tuned.session!=session
        ||!tssi.matches(tuned)||!authority.matches(tuned,channel::CalibrationEngine::Dpk){return Err(Failure::guard(fault(0,0)));}
    let bus=phy.bus_between_stages();let start=bus.now_us();
    phy::guard(bus,&board.identity,phy::RegisterPhase::AfterDack).map_err(Failure::guard)?;
    remaining(bus,start).map_err(Failure::guard)?;
    let result=(||{
        let data=engine(bus,Admission{channel:tuned.channel,tssi:tssi.is_tssi_mode()},start)?;
        let checked=phy::guard(bus,&board.identity,phy::RegisterPhase::AfterDack).and_then(|()|remaining(bus,start).map(|_|()));
        if let Err(cause)=checked{let mut failure=Failure::guard(cause);failure.stage=Stage::Verify;return Err(failure);}
        Ok(Completed{attempt,session,channel:tuned.channel,policy_version:tuned.policy_version,data})
    })();
    match result{Ok(done)=>Ok(done),Err(mut error)=>{
        if error.stage==Stage::Verify{error.radio_stop=phy::disable_bb_rf(phy.bus_between_stages()).err();}
        phy.mark_failed();Err(error)
    }}
}

#[cfg(test)]
#[path="../../tests/rtw89_dpk_cases.rs"]
mod tests;
