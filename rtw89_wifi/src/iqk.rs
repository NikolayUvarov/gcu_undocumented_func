// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! Private initial 2 GHz/20 MHz wideband IQK; no native calibration-TX policy provider exists yet.

use crate::{board::Board,btc_context::Initialized,dack::{self,Band,BtcNotice,Calibration,RfkState},
    io::{self,Fault,FaultKind,RegisterIo,TimedIo},iqk_tables as table,phy::{self,Phy,State},rxdck::{self,RxdckIo}};

const BASE:usize=0x10000;
const RF_MASK:u32=0xfffff;
const TOTAL_US:u64=3_300_000;
const BB_BACKUP:[usize;3]=[0x2344,0x5800,0x7800];
const RF_BACKUP:[u32;11]=[0xde,0xdf,0x8b,0x90,0x97,0x85,0x1e,0,2,5,0x10005];

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) enum Stage{Guard,ChannelRxDck,BtcStart,Pause,Wait,Init,NestedStart,Backup,Setup,Lok,Tx,Rx,Report,Restore,NestedStop,Resume,BtcStop,Quality}
#[derive(Clone,Copy,Debug,Default,Eq,PartialEq)]
pub(crate) struct PathResult{
    pub lok_attempts:u8,pub lok_idac:u32,pub lok_vbuf:u32,pub lok_range_fail:bool,
    pub lok_fail:[bool;4],pub tx_fail:[bool;4],pub rx_fail:[bool;4],pub restore_fail:bool,
    pub enable:u32,pub tx_result:u32,pub rx_result:u32,
}
impl PathResult{
    fn failed(&self)->bool{self.lok_range_fail||self.lok_fail.iter().chain(self.tx_fail.iter()).chain(self.rx_fail.iter()).any(|v|*v)||self.restore_fail}
}
#[derive(Debug)]
pub(crate) struct Failure{
    pub stage:Stage,pub cause:Fault,pub channel_rx:Option<rxdck::Failure>,pub results:[PathResult;2],
    pub nctl_uncertain:bool,pub register_restore:Option<Fault>,pub scheduler_restore:Option<Fault>,
    pub scheduler_status:Option<Fault>,pub radio_stop:Option<Fault>,pub nested_stop:Option<Fault>,pub btc_stop:Option<Fault>,
}
impl Failure{
    fn guard(cause:Fault)->Self{Self{stage:Stage::Guard,cause,channel_rx:None,results:[PathResult::default();2],nctl_uncertain:false,
        register_restore:None,scheduler_restore:None,scheduler_status:None,radio_stop:None,nested_stop:None,btc_stop:None}}
}
#[derive(Debug)]
pub(crate) struct Completed{attempt:u64,session:u64,channel:u8,policy_version:u64,pub results:[PathResult;2]}
impl Completed{
    pub(crate) fn matches(&self,attempt:u64,session:u64,channel:u8,policy_version:u64)->bool{
        attempt!=0&&session!=0&&self.attempt==attempt&&self.session==session&&self.channel==channel&&self.policy_version==policy_version
    }
}
fn bad(at:usize,value:u32)->Fault{Fault::new(FaultKind::PowerState,at,value)}
fn left(bus:&mut impl RegisterIo,start:u64)->io::Result<u32>{
    let used=bus.now_us().checked_sub(start).ok_or(Fault::new(FaultKind::Clock,0,0))?;
    TOTAL_US.checked_sub(used).filter(|v|*v>0).map(|v|v as u32).ok_or(Fault::new(FaultKind::Timeout,0,0))
}
fn bb(bus:&mut impl RegisterIo,at:usize,mask:u32,value:u32)->io::Result<()>{
    io::modify32(bus,BASE+at,mask,(value<<mask.trailing_zeros())&mask)
}
fn read_bb(bus:&mut impl RegisterIo,at:usize)->io::Result<u32>{io::read32(bus,BASE+at)}
fn rf(bus:&mut impl RegisterIo,path:u8,at:u32,mask:u32,value:u32)->io::Result<()>{phy::rf_write_masked(bus,path,at,mask,value)}
fn table(bus:&mut impl RegisterIo,rows:&[(usize,u32,u32)])->io::Result<()>{for &(at,mask,value) in rows{bb(bus,at,mask,value)?;}Ok(())}
fn notice(state:RfkState)->BtcNotice{BtcNotice{calibration:Calibration::Iqk,state,phy_map:if matches!(state,RfkState::OneshotStart|RfkState::OneshotStop){0x13}else{0x10}}}
fn notify(bus:&mut impl RxdckIo,state:RfkState,start:u64)->io::Result<()>{let timeout=left(bus,start)?.min(100_000);dack::notify(bus,notice(state),timeout)}
fn status(bus:&mut impl RegisterIo)->io::Result<()>{
    let value=io::read32(bus,0xc348)?;if value&0xffff!=0{return Err(bad(0xc348,value));}
    io::poll32(bus,0xc6ec,1<<7,0,8,80_000)?;bus.delay_us(0)
}
fn scheduler(bus:&mut impl RxdckIo,timeout:u32)->io::Result<()>{
    let start=bus.now_us();bus.scheduler_zero(timeout)?;
    let elapsed=bus.now_us().checked_sub(start).ok_or(Fault::new(FaultKind::Clock,0xc348,0))?;
    if elapsed>=u64::from(timeout){return Err(Fault::new(FaultKind::Timeout,0xc348,timeout));}
    status(&mut TimedIo::new(bus,u64::from(timeout)-elapsed))
}
fn wait(bus:&mut impl RegisterIo,path:u8)->io::Result<()>{
    let mut timed=TimedIo::new(bus,5_000);
    for _ in 0..=2_500{let mode=phy::rf_read(&mut timed,path,0,0xf0000)?;timed.delay_us(0)?;
        if mode!=2{return Ok(());}timed.delay_us(2)?;}
    Err(Fault::new(FaultKind::Timeout,0,2))
}
#[derive(Clone,Copy)]
struct Saved{bb:[u32;3],rf:[u32;11],path:u8}
fn backup(bus:&mut impl RegisterIo,path:u8)->io::Result<Saved>{
    let mut saved=Saved{bb:[0;3],rf:[0;11],path};
    for (at,value) in BB_BACKUP.iter().zip(saved.bb.iter_mut()){*value=read_bb(bus,*at)?;}
    for (at,value) in RF_BACKUP.iter().zip(saved.rf.iter_mut()){*value=phy::rf_read(bus,path,*at,RF_MASK)?;}Ok(saved)
}
fn restore_registers(bus:&mut impl RegisterIo,saved:Saved)->io::Result<()>{
    let mut first=None;
    for &(at,mask,value) in table::RESTORE_NONDBCC_PATH01{if let Err(e)=bb(bus,at,mask,value){first.get_or_insert(e);}}
    for (at,value) in BB_BACKUP.iter().zip(saved.bb){
        let result=(||{bus.write32(BASE+*at,value)?;let actual=read_bb(bus,*at)?;
            if actual!=value{return Err(bad(BASE+*at,actual));}Ok(())})();if let Err(e)=result{first.get_or_insert(e);}
    }
    for (at,value) in RF_BACKUP.iter().zip(saved.rf){
        let result=(||{rf(bus,saved.path,*at,RF_MASK,value)?;let actual=phy::rf_read(bus,saved.path,*at,RF_MASK)?;
            if actual!=value{return Err(bad(*at as usize,actual));}Ok(())})();if let Err(e)=result{first.get_or_insert(e);}
    }
    if let Err(e)=bus.delay_us(0){first.get_or_insert(e);}first.map_or(Ok(()),Err)
}
fn complete(bus:&mut impl RegisterIo,pending:&mut bool)->io::Result<bool>{
    io::poll32(bus,BASE+0xbff8,255,0x55,1,8_200)?;
    *pending=false;
    bus.delay_us(200)?;let failed=read_bb(bus,0x8008)?&(1<<26)!=0;
    bb(bus,0x8010,255,0)?;bus.delay_us(0)?;Ok(failed)
}
fn shot(bus:&mut impl RegisterIo,path:u8,kind:u32,pending:&mut bool)->io::Result<bool>{
    if *pending{return Err(bad(BASE+0x8000,kind));}
    bb(bus,0x5864,1<<29,u32::from(kind!=0x800))?;
    *pending=true;bus.write32(BASE+0x8000,(kind|8|1<<(4+path))+1)?;bus.delay_us(1)?;
    let failed=complete(bus,pending)?;bb(bus,0x5864,1<<29,0)?;Ok(failed)
}
fn clocks(bus:&mut impl RegisterIo,rx:bool)->io::Result<()>{
    bb(bus,0x12b8,1<<30,1)?;bb(bus,0x32b8,1<<30,1)?;bus.delay_us(1)?;
    bb(bus,0x030c,0xff000000,if rx{15}else{31})?;bus.delay_us(1)?;
    bb(bus,0x030c,0xff000000,if rx{3}else{19})?;
    bb(bus,0x032c,0xffff0000,if rx{0xa001}else{1})?;bus.delay_us(1)?;
    bb(bus,0x032c,0xffff0000,if rx{0xa041}else{0x41})?;
    if rx{for at in [0x12a0,0x32a0]{bb(bus,at,0x70000,1)?;bb(bus,at,1<<19,1)?;}
        bb(bus,0x700,1<<24,1)?;bb(bus,0x700,0x6000000,0)?;}Ok(())
}
fn setup(bus:&mut impl RegisterIo,path:u8)->io::Result<()>{
    table(bus,table::SET_NONDBCC_PATH01)?;let offset=usize::from(path)<<8;
    bb(bus,0x8104+offset,1,0)?;bb(bus,0x8154+offset,8,0)?;
    rf(bus,path,5,1,0)?;rf(bus,path,0x10005,1,0)?;
    bus.write32(BASE+0x8008,0x80)?;bus.write32(BASE+0x8088,0x81ff010a)
}
fn lok(bus:&mut impl RegisterIo,path:u8,result:&mut PathResult,pending:&mut bool)->io::Result<()>{
    clocks(bus,false)?;let offset=usize::from(path)<<8;
    for ibias in 1..=3{
        result.lok_attempts=ibias;
        for (at,mask,value) in [(0xef,RF_MASK,2),(0x33,RF_MASK,0),(0x3f,RF_MASK,u32::from(ibias)),(0xef,RF_MASK,0),
            (0x7c,1<<5,1),(0x90,0x300,0),(0x51,1<<19,0),(0x51,1<<11,0),(0x52,1<<11,1),(0x55,31,0),
            (0xef,4,1),(0x33,255,0),(0,0xffff0,0x403e)]{rf(bus,path,at,mask,value)?;}
        bus.delay_us(1)?;bb(bus,0x802c,0xfff,0x21)?;
        rf(bus,path,0x11,3,0)?;rf(bus,path,0x11,0x70,6)?;
        for (i,(kind,tg,iqp)) in [(0x100,0,9),(0x300,0x12,0x24),(0x200,0,9),(0x300,0x12,0x24)].into_iter().enumerate(){
            rf(bus,path,0x11,0x1f000,tg)?;bus.write32(BASE+0x81cc+offset,iqp)?;
            if kind==0x200{bb(bus,0x802c,0xfff,0x21)?;}
            result.lok_fail[i]=shot(bus,path,kind,pending)?;
        }
        result.lok_idac=phy::rf_read(bus,path,0x58,RF_MASK)?;result.lok_vbuf=phy::rf_read(bus,path,0x0a,RF_MASK)?;
        let idac=result.lok_idac;let buf=result.lok_vbuf;
        result.lok_range_fail=!(2..=29).contains(&((idac>>15)&31))||!(2..=29).contains(&((idac>>10)&31))
            ||!(2..=61).contains(&((buf>>14)&63))||!(2..=61).contains(&((buf>>4)&63));
        if !result.lok_range_fail{break;}
    }Ok(())
}
fn tx(bus:&mut impl RegisterIo,path:u8,result:&mut PathResult,pending:&mut bool)->io::Result<()>{
    let offset=usize::from(path)<<8;
    for (group,(track,gain,iqp)) in [(4,8,9),(4,14,0x12),(6,6,0x1b),(6,14,0x24)].into_iter().enumerate(){
        rf(bus,path,0x11,3,0)?;rf(bus,path,0x11,0x70,track)?;rf(bus,path,0x11,0x1f000,gain)?;
        bus.write32(BASE+0x81cc+offset,iqp)?;bb(bus,0x8154+offset,0x100,1)?;bb(bus,0x8154+offset,0x10,1)?;
        bb(bus,0x8154+offset,4,0)?;bb(bus,0x8154+offset,3,group as u32)?;bb(bus,0x8010,255,0)?;
        result.tx_fail[group]=shot(bus,path,0x800,pending)?;
        bb(bus,0x9fe0,1<<(8+group+usize::from(path)*4),u32::from(result.tx_fail[group]))?;
    }
    bb(bus,0x8124+offset,0xf00,if result.tx_fail.iter().any(|v|*v){0}else{5})
}
fn rx(bus:&mut impl RegisterIo,path:u8,result:&mut PathResult,pending:&mut bool)->io::Result<()>{
    clocks(bus,true)?;let offset=usize::from(path)<<8;
    rf(bus,path,0,0xf0000,12)?;rf(bus,path,0x20,1<<8,1)?;
    let channel=phy::rf_read(bus,path,0x18,RF_MASK)?;rf(bus,path,0x1f,RF_MASK,channel)?;
    for (group,(gain,c2,c1)) in [(0x212,0,3),(0x21c,0,3),(0x350,0x28,2),(0x360,0x5f,1)].into_iter().enumerate(){
        rf(bus,path,0,0x3ff0,gain)?;rf(bus,path,0x83,0x1fc00,c2)?;rf(bus,path,0x83,0x300,c1)?;
        bb(bus,0x8154+offset,0x100,1)?;bb(bus,0x8154+offset,0x10,0)?;bb(bus,0x8154+offset,7,group as u32)?;
        result.rx_fail[group]=shot(bus,path,0xb00,pending)?;
        bb(bus,0x9fe0,1<<(16+group+usize::from(path)*4),u32::from(result.rx_fail[group]))?;
    }
    rf(bus,path,0x20,1<<7,0)?;bb(bus,0x8124+offset,15,if result.rx_fail.iter().any(|v|*v){0}else{5})
}
fn report(bus:&mut impl RegisterIo,path:u8,result:&mut PathResult,fail_count:&mut u32)->io::Result<()>{
    let offset=usize::from(path)<<8;
    let flags=[result.lok_fail[0],result.lok_fail[2],result.tx_fail.iter().any(|v|*v),result.rx_fail.iter().any(|v|*v)];
    for (bit,fail) in flags.into_iter().enumerate(){bb(bus,0x9fe0,1<<(bit+usize::from(path)*4),u32::from(fail))?;}
    result.enable=read_bb(bus,0x8124+offset)?;result.tx_result=read_bb(bus,0x8138+offset)?;result.rx_result=read_bb(bus,0x813c+offset)?;
    bb(bus,0x9fe8,0xff00,u32::from(path)+1)?;if flags.iter().any(|v|*v){*fail_count+=1;}
    bb(bus,0x9fe8,0xff0000<<(u32::from(path)*4),*fail_count)
}
fn restore(bus:&mut impl RegisterIo,path:u8,result:&mut PathResult,pending:&mut bool)->io::Result<()>{
    let offset=usize::from(path)<<8;
    bus.write32(BASE+0x8138+offset,if result.tx_fail.iter().any(|v|*v){0x40000002}else{0x40000000})?;
    bus.write32(BASE+0x813c+offset,if result.rx_fail.iter().any(|v|*v){0x40000002}else{0x40000000})?;
    if *pending{return Err(bad(BASE+0x8000,0xe19));}
    *pending=true;bus.write32(BASE+0x8000,0xe19+(u32::from(path)<<4))?;result.restore_fail=complete(bus,pending)?;
    bb(bus,0x8010,255,0)?;bus.write32(BASE+0x8008,0)?;bus.write32(BASE+0x8088,0x80000000)?;
    bb(bus,0x8120,1<<28,0)?;bb(bus,0x8220,1<<28,0)?;bb(bus,0x8220,1<<16,0)?;
    rf(bus,path,0xef,4,0)?;rf(bus,path,0xef,4,0)?;rf(bus,path,0,0xf0000,3)?;rf(bus,path,5,1,1)?;rf(bus,path,0x10005,1,1)
}

fn engine(bus:&mut impl RxdckIo,channel:u8,start:u64)->core::result::Result<[PathResult;2],Failure>{
    let mut stage=Stage::BtcStart;let mut outer=false;let mut outer_stopped=false;let mut outer_error=None;
    let mut nested=false;let mut nested_error=None;let mut paused=false;let mut resumed=false;let mut resume_error=None;
    let mut saved=None;let mut pending=false;let mut results=[PathResult::default();2];let mut fail_count=0;
    let result=(||{
        outer=true;notify(bus,RfkState::Start,start)?;
        stage=Stage::Pause;let timeout=left(bus,start)?.min(1_000_000);scheduler(bus,timeout)?;paused=true;
        stage=Stage::Wait;let remaining=left(bus,start)?;let mut timed=TimedIo::new(bus,u64::from(remaining));
        wait(&mut timed,0)?;wait(&mut timed,1)?;
        stage=Stage::Init;timed.write32(BASE+0x9fe0,0)?;
        for path in 0..2u8{
            stage=Stage::NestedStart;nested=true;notify(bus,RfkState::OneshotStart,start)?;
            let remaining=left(bus,start)?;let mut timed=TimedIo::new(bus,u64::from(remaining));
            let actual=phy::rf_read(&mut timed,path,0x18,RF_MASK)?;
            if actual&0x3e3ff!=u32::from(channel){return Err(bad(0x18,actual));}
            let _synth=read_bb(&mut timed,0x35c)?&0xc00;
            bb(&mut timed,0x9fe0,0xff000000,0x2a)?;
            for (mask,value) in [(15,0),(0xf0,0),(0xff00,u32::from(channel))]{bb(&mut timed,0x9fe4,mask<<(path*16),value)?;}
            stage=Stage::Backup;saved=Some(backup(&mut timed,path)?);
            stage=Stage::Setup;setup(&mut timed,path)?;
            let result=&mut results[path as usize];
            stage=Stage::Lok;lok(&mut timed,path,result,&mut pending)?;
            stage=Stage::Tx;tx(&mut timed,path,result,&mut pending)?;
            stage=Stage::Rx;rx(&mut timed,path,result,&mut pending)?;
            stage=Stage::Report;report(&mut timed,path,result,&mut fail_count)?;
            stage=Stage::Restore;restore(&mut timed,path,result,&mut pending)?;
            restore_registers(&mut timed,saved.unwrap())?;saved=None;
            timed.delay_us(0)?;
            stage=Stage::NestedStop;nested=false;
            if let Err(e)=notify(bus,RfkState::OneshotStop,start){nested_error=Some(e);return Err(e);}
        }
        stage=Stage::Resume;let timeout=left(bus,start)?.min(1_000_000);resumed=true;
        if let Err(e)=scheduler(bus,timeout){resume_error=Some(e);return Err(e);}
        if results.iter().any(PathResult::failed){stage=Stage::Quality;return Err(bad(BASE+0x8008,1));}
        stage=Stage::BtcStop;outer_stopped=true;
        if let Err(e)=notify(bus,RfkState::Stop,start){outer_error=Some(e);return Err(e);}
        left(bus,start)?;Ok(results)
    })();
    match result{
        Ok(done)=>Ok(done),
        Err(cause)=>{
            let register_restore=if pending{None}else{saved.and_then(|v|restore_registers(&mut TimedIo::new(bus,20_000),v).err())};
            let scheduler_restore=if paused&&!resumed{scheduler(bus,1_000_000).err()}else{resume_error};
            let scheduler_status=if outer{status(&mut TimedIo::new(bus,80_000)).err()}else{None};
            let radio_stop=if outer{phy::disable_bb_rf(bus).err()}else{None};
            let nested_stop=if nested{dack::notify(bus,notice(RfkState::OneshotStop),100_000).err()}else{nested_error};
            let btc_stop=if outer&&!outer_stopped{dack::notify(bus,notice(RfkState::Stop),100_000).err()}else{outer_error};
            Err(Failure{stage,cause,channel_rx:None,results,nctl_uncertain:pending,register_restore,scheduler_restore,scheduler_status,radio_stop,nested_stop,btc_stop})
        }
    }
}

/// Initial channel RFK owns a fresh Tune and explicit calibration power admission; ordinary TX remains zero.
/// The caller must retain the same exclusive device/session, firmware and DMA until independent shutdown.
pub(crate) fn run<B:RxdckIo>(phy:&mut Phy<'_,B>,board:&Board,tuned:&phy::channel::Tuned,
    initialized:&Initialized,authority:phy::channel::CalibrationTransmit)->core::result::Result<Completed,Failure>{
    let attempt=phy.attempt();let session=phy.bus_between_stages().command_session();
    if phy.state()!=State::ChannelTuned||!initialized.matches(attempt,session)
        ||tuned.attempt!=attempt||tuned.session!=session||!(1..=13).contains(&tuned.channel)
        ||!authority.matches(tuned,phy::channel::CalibrationEngine::Iqk){return Err(Failure::guard(bad(0,0)));}
    let bus=phy.bus_between_stages();let start=bus.now_us();
    phy::guard(bus,&board.identity,phy::RegisterPhase::AfterDack).map_err(Failure::guard)?;
    if let Err(cause)=left(bus,start){return Err(Failure::guard(cause));}
    if let Err(error)=rxdck::run(bus,Band::Ghz2,start){
        let mut failure=Failure::guard(error.cause);failure.stage=Stage::ChannelRxDck;failure.channel_rx=Some(error);
        phy.mark_failed();return Err(failure);
    }
    let result=(||{
        phy::guard(bus,&board.identity,phy::RegisterPhase::AfterDack).map_err(Failure::guard)?;
        let iqk_start=bus.now_us();let results=engine(bus,tuned.channel,iqk_start)?;
        let elapsed=bus.now_us().checked_sub(start).ok_or_else(||Failure::guard(Fault::new(FaultKind::Clock,0,0)))?;
        if elapsed>=6_000_000{return Err(Failure::guard(Fault::new(FaultKind::Timeout,0,0)));}
        phy::guard(bus,&board.identity,phy::RegisterPhase::AfterDack).map_err(Failure::guard)?;
        let elapsed=bus.now_us().checked_sub(start).ok_or_else(||Failure::guard(Fault::new(FaultKind::Clock,0,0)))?;
        if elapsed>=6_000_000{return Err(Failure::guard(Fault::new(FaultKind::Timeout,0,0)));}
        Ok(Completed{attempt,session,channel:tuned.channel,policy_version:tuned.policy_version,results})
    })();
    match result{
        Ok(done)=>Ok(done),
        Err(mut error)=>{
            if error.stage==Stage::Guard{error.radio_stop=phy::disable_bb_rf(phy.bus_between_stages()).err();}
            phy.mark_failed();Err(error)
        }
    }
}

#[cfg(test)]
#[path="../../tests/rtw89_iqk_cases.rs"]mod tests;
