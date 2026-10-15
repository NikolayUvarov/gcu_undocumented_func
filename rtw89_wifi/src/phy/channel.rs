// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022, 2024 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! Private initial 2 GHz/20 MHz MAC/BB/RF/power transaction; channel RFK and TX authorization remain separate.

use super::{Phy,PhyIo,State,RegisterPhase,PHY_BASE,Gain,dm,tail,guard,rf_read,rf_write_masked,disable_bb_rf};
use crate::{board::Board,calibration,channel_policy as defaults,channel_power::{self,Constraints},download::DefaultTables,
    io::{self,Fault,FaultKind,RegisterIo,TimedIo}};

const TOTAL_US:u64=2_200_000;
const RF_MASK:u32=0xfffff;

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) enum Access{Disabled,PassiveOnly,InitiateRadiation}
/// A policy owner must derive these fields from actual regulatory/platform evidence, including antenna/system gain.
/// This private input has no native provider yet; Linux country observations alone cannot construct authority.
pub(crate) struct Rule{pub country:[u8;2],pub version:u64,pub center_mhz:u16,pub access:Access,
    pub eirp_half_dbm:i8,pub conducted_half_dbm:i8}
pub(crate) enum Platform{Unknown,Resolved(Constraints)}
#[derive(Debug)]
pub(crate) struct Policy{country:[u8;2],plan:u8,version:u64,channel:u8,access:Access,group:defaults::Group,limits:Constraints}
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) enum PolicyError{Board,Country,Frequency,Disabled,UnknownPlatform,Power}

impl Policy{
    pub(crate) fn resolve(board:&Board,rule:Rule,platform:Platform)->Result<Self,PolicyError>{
        let hint=defaults::board_country(board).map_err(|_|PolicyError::Board)?;
        let selected=defaults::country_default(rule.country).ok_or(PolicyError::Country)?;
        if rule.version==0||rule.country==*b"00"{return Err(PolicyError::Country);}
        if let defaults::Hint::Programmed(programmed)=hint.hint{
            if programmed.alpha2!=rule.country{return Err(PolicyError::Country);}
        }
        let delta=rule.center_mhz.checked_sub(2407).ok_or(PolicyError::Frequency)?;
        if delta%5!=0||!(1..=13).contains(&(delta/5)){return Err(PolicyError::Frequency);}
        if rule.access==Access::Disabled{return Err(PolicyError::Disabled);}
        let Platform::Resolved(mut limits)=platform else{return Err(PolicyError::UnknownPlatform);};
        if !(-64..=63).contains(&rule.eirp_half_dbm)||!(-64..=63).contains(&rule.conducted_half_dbm)
            ||rule.conducted_half_dbm>rule.eirp_half_dbm
            ||limits.per_ntx.iter().chain(limits.per_path.iter()).any(|v|!(-64..=63).contains(v)){
            return Err(PolicyError::Power);
        }
        for value in &mut limits.per_ntx{*value=(*value).min(rule.conducted_half_dbm);}
        // Per-path values are source SAR reference differences, already resolved for this platform and frequency.
        Ok(Self{country:hint.raw,plan:hint.raw_channel_plan,version:rule.version,channel:(delta/5)as u8,
            access:rule.access,group:selected.groups[0],limits})
    }
}

/// Delegate to the same owned register mailbox as runtime_mac::scheduler_zero; no retry after an uncertain error.
/// Both phases retain the initially verified zero scheduler, and do not grant any packet or calibration TX.
pub(crate) trait ChannelIo:PhyIo{fn scheduler_zero(&mut self,timeout_us:u32)->io::Result<()>;}
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) enum Stage{Guard,Pause,Prepare,Mac,Baseband,Radio,Power,Finish,Restore,Verify}
#[derive(Debug)]
pub(crate) struct Failure{pub stage:Stage,pub cause:Fault,pub scheduler_restore:Option<Fault>,
    pub radio_cleanup:Option<Fault>,pub radio_stop:Option<Fault>}
#[derive(Debug)]
pub(crate) struct Tuned{pub attempt:u64,pub session:u64,pub channel:u8,pub policy_version:u64,pub access:Access}

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) enum CalibrationEngine{Iqk,Tssi,Dpk}
/// RFK may radiate with the ordinary scheduler stopped; channel selection is insufficient authority.
/// No production constructor exists until actual RFK raw levels and platform SAR/EIRP are resolved.
#[derive(Debug)]
pub(crate) struct CalibrationTransmit{attempt:u64,session:u64,channel:u8,policy_version:u64,engine:CalibrationEngine}
impl CalibrationTransmit{
    pub(crate) fn matches(&self,tuned:&Tuned,engine:CalibrationEngine)->bool{
        tuned.access==Access::InitiateRadiation&&self.attempt!=0&&self.session!=0&&self.policy_version!=0
            &&self.attempt==tuned.attempt&&self.session==tuned.session&&self.channel==tuned.channel
            &&self.policy_version==tuned.policy_version&&self.engine==engine&&(1..=13).contains(&self.channel)
    }
    #[cfg(test)]
    pub(crate) fn modeled(tuned:&Tuned,engine:CalibrationEngine)->Option<Self>{
        let token=Self{attempt:tuned.attempt,session:tuned.session,channel:tuned.channel,policy_version:tuned.policy_version,engine};
        token.matches(tuned,engine).then_some(token)
    }
}

fn fault(at:usize,value:u32)->Fault{Fault::new(FaultKind::PowerState,at,value)}
fn left(bus:&mut impl RegisterIo,start:u64)->io::Result<u64>{
    let elapsed=bus.now_us().checked_sub(start).ok_or(Fault::new(FaultKind::Clock,0,0))?;
    TOTAL_US.checked_sub(elapsed).filter(|v|*v>0).ok_or(Fault::new(FaultKind::Timeout,0,0))
}
fn field(bus:&mut impl RegisterIo,at:usize,mask:u32,value:u32)->io::Result<()>{
    let bits=(value<<mask.trailing_zeros())&mask;io::modify32(bus,at,mask,bits)?;
    let actual=io::read32(bus,at)?;if actual&mask!=bits{return Err(fault(at,actual));}Ok(())
}
fn bb(bus:&mut impl RegisterIo,at:usize,mask:u32,value:u32)->io::Result<()>{field(bus,PHY_BASE+at,mask,value)}
fn rf(bus:&mut impl RegisterIo,path:u8,at:u32,mask:u32,value:u32)->io::Result<()>{
    rf_write_masked(bus,path,at,mask,value)?;
    let actual=rf_read(bus,path,at,mask)?;if actual!=value{return Err(fault(at as usize,actual));}Ok(())
}
fn scheduler(bus:&mut impl ChannelIo,timeout:u32)->io::Result<()>{
    let start=bus.now_us();bus.scheduler_zero(timeout)?;
    let elapsed=bus.now_us().checked_sub(start).ok_or(Fault::new(FaultKind::Clock,0xc348,0))?;
    if elapsed>=u64::from(timeout){return Err(Fault::new(FaultKind::Timeout,0xc348,timeout));}
    let mut timed=TimedIo::new(bus,u64::from(timeout)-elapsed);
    let actual=io::read32(&mut timed,0xc348)?;if actual&0xffff!=0{return Err(fault(0xc348,actual));}
    io::poll32(&mut timed,0xc6ec,1<<7,0,8,80_000)?;timed.delay_us(0)
}
fn tssi(bus:&mut impl RegisterIo,hold:bool)->io::Result<()>{
    for at in [0x58dc,0x5818,0x78dc,0x7818]{bb(bus,at,1<<30,u32::from(hold))?;}Ok(())
}
fn reset(bus:&mut impl RegisterIo,enable:bool)->io::Result<()>{
    if enable{
        bb(bus,0x1200,0x70000000,0)?;bb(bus,0x3200,0x70000000,0)?;
        bb(bus,0x704,2,1)?;bb(bus,0x2344,1<<31,0)?;bb(bus,0xc3c,1<<9,0)
    }else{
        bb(bus,0x2344,1<<31,1)?;bb(bus,0xc3c,1<<9,1)?;
        bb(bus,0x1200,0x70000000,7)?;bb(bus,0x3200,0x70000000,7)?;
        bus.delay_us(1)?;bb(bus,0x704,2,0)
    }
}
fn reset_all(bus:&mut impl RegisterIo)->io::Result<()>{
    bb(bus,0x1200,0x70000000,7)?;bb(bus,0x3200,0x70000000,7)?;bus.delay_us(1)?;
    bb(bus,0x704,2,1)?;bb(bus,0x704,2,0)?;
    bb(bus,0x1200,0x70000000,0)?;bb(bus,0x3200,0x70000000,0)?;bb(bus,0x704,2,1)
}
fn mac(bus:&mut impl RegisterIo)->io::Result<()>{
    io::modify8(bus,0xc010,3,0)?;bus.write32(0xc088,0)?;io::modify8(bus,0xc628,0x13,0x10)?;
    for(at,mask,wanted)in[(0xc010,3,0),(0xc628,0x13,0x10)]{
        let value=bus.read8(at)?;if value==255||value&mask!=wanted{return Err(fault(at,u32::from(value)));}
    }
    let value=io::read32(bus,0xc088)?;if value!=0{return Err(fault(0xc088,value));}Ok(())
}
fn average(a:i8,b:i8)->u8{((i16::from(a)+i16::from(b))>>1)as i8 as u8}
fn gain(bus:&mut impl RegisterIo,g:&Gain)->io::Result<()>{
    for p in 0..2{
        let base=if p==0{0x4678}else{0x475c};
        for(i,(off,shift))in[(0,16),(0,24),(4,0),(4,8),(4,16),(4,24),(8,0)].into_iter().enumerate(){
            bb(bus,base+off,255<<shift,g.lna[0][p][i]as u8 as u32)?;
        }
        for i in 0..2{bb(bus,base+8,255<<(16+i*8),g.tia[0][p][i]as u8 as u32)?;}
    }Ok(())
}
fn rpl(bus:&mut impl RegisterIo,g:&Gain)->io::Result<()>{
    let a=|v:[[i8;2];4]|average(v[0][0],v[0][1]);
    let b=|i|average(g.offset40[0][0][i],g.offset40[0][1][i]);
    let c=|i|average(g.offset80[0][0][i],g.offset80[0][1][i]);
    let value=u32::from_le_bytes([0,a(g.offset20),b(0),b(1)]);
    for at in [0x49b0,0x4a00]{bb(bus,at,0xffffff00,value>>8)?;}
    for(at,value)in[(0x49b4,[b(2),c(0),c(1),c(10)]),(0x4a04,[b(2),c(0),c(1),c(10)]),
        (0x49b8,[c(2),c(3),c(4),c(9)]),(0x4a08,[c(2),c(3),c(4),c(9)])]{bus.write32(PHY_BASE+at,u32::from_le_bytes(value))?;}Ok(())
}
fn baseband(bus:&mut impl RegisterIo,channel:u8,path:u8,cal:&calibration::Calibration,prefix:&dm::Prefix)->io::Result<()>{
    const BARKER:[u32;13]=[0x1cfea,0x1d0e1,0x1d1d7,0x1d2cd,0x1d3c3,0x1d4b9,0x1d5b0,0x1d6a6,0x1d79c,0x1d892,0x1d988,0x1da7f,0x1db75];
    const CCK:[u32;13]=[0x27de3,0x27f35,0x28088,0x281da,0x2832d,0x2847f,0x285d2,0x28724,0x28877,0x289c9,0x28b1c,0x28c6e,0x28dc1];
    bb(bus,0x23b0,0x7ffff,BARKER[channel as usize-1])?;bb(bus,0x23b4,0x7ffff,CCK[channel as usize-1])?;
    bb(bus,0x4738,1<<17,1)?;bb(bus,0x4aa4,1<<17,1)?;
    bb(bus,0x49c0,127,match channel{1=>109,2..=6=>108,7..=10=>107,_=>106})?;
    for(i,v)in[0x3d23ff,0x29b354,0xfc1c8,0xfdb053,0xf86f9a,0xfaef92,0xfe5fcc,0xffdff5].into_iter().enumerate(){bb(bus,0x2300+4*i,0xffffff,v)?;}
    let defaults=super::gain_defaults();gain(bus,&defaults)?;
    tail::gain_offset(bus,cal,prefix,tail::Subband::Ghz2,path)?;rpl(bus,&defaults)?;
    bb(bus,0x49c0,0xc0000000,0)?;bb(bus,0x49c4,0x3000,0)?;bb(bus,0x49c4,0xf00,0)?;
    bb(bus,0x12b0,0xfff,0x333)?;bb(bus,0x32b0,0xfff,0x333)?;
    for(at,mask,value)in[(0xc0ec,0x6000,0),(0xc0e4,0x30,2),(0xc1ec,0x6000,0),(0xc1e4,0x30,2)]{bb(bus,at,mask,value)?;}
    if path!=3{bb(bus,if path==1{0x32b0}else{0x12b0},0xfff,0x111)?;}
    bb(bus,0x700,1<<5,1)?;bb(bus,0x2344,1<<31,0)?;bb(bus,0x734,0x00ff0000,u32::from(channel))?;
    bb(bus,0x46f8,1<<12,0)?;bb(bus,0x47b8,1<<12,0)?;bb(bus,0x4440,1<<31,0)?;reset_all(bus)
}

#[derive(Default)]
struct Cleanup{ldo:Option<u32>,trigger:bool,synlut:bool,mmd:bool,rx_lut:[bool;2]}
fn arfc(bus:&mut impl RegisterIo,value:u32,saved:&mut Cleanup)->io::Result<()>{
    let old=rf_read(bus,0,0xb1,RF_MASK)?;saved.ldo=Some(old);
    rf(bus,0,0xb1,0x1c0,1)?;rf_write_masked(bus,0,0x18,RF_MASK,value)?;
    let mut timed=TimedIo::new(bus,1_000);let mut idle=false;
    for _ in 0..=1_000{let busy=rf_read(&mut timed,0,0xb7,1<<8)?;timed.delay_us(0)?;
        if busy==0{idle=true;break;}timed.delay_us(1)?;}
    if !idle{return Err(Fault::new(FaultKind::Timeout,0xb7,1));}
    rf(bus,0,0xb1,RF_MASK,old)?;saved.ldo=None;Ok(())
}
fn lock(bus:&mut impl RegisterIo,saved:&mut Cleanup)->io::Result<()>{
    if rf_read(bus,0,0xc5,1<<15)?==0{
        saved.mmd=true;rf(bus,0,0xd5,1<<8,1)?;rf(bus,0,0xd5,1<<6,0)?;rf(bus,0,0xd5,1<<6,1)?;
        rf(bus,0,0xd5,1<<8,0)?;saved.mmd=false;
    }
    bus.delay_us(10)?;
    if rf_read(bus,0,0xc5,1<<15)?==0{
        saved.trigger=true;rf(bus,0,0xd3,1<<8,1)?;let value=rf_read(bus,0,0x18,RF_MASK)?;arfc(bus,value,saved)?;
        rf(bus,0,0xd3,1<<8,0)?;saved.trigger=false;
    }
    if rf_read(bus,0,0xc5,1<<15)?==0{
        for at in [0xa0,0xaf]{let value=rf_read(bus,0,at,RF_MASK)?;rf(bus,0,at,RF_MASK,value)?;}
        saved.synlut=true;rf(bus,0,0xdd,1<<4,1)?;rf(bus,0,0xa0,12,0)?;rf(bus,0,0xa0,12,3)?;
        rf(bus,0,0xdd,1<<4,0)?;saved.synlut=false;
        saved.trigger=true;rf(bus,0,0xd3,1<<8,1)?;let value=rf_read(bus,0,0x18,RF_MASK)?;arfc(bus,value,saved)?;
        rf(bus,0,0xd3,1<<8,0)?;saved.trigger=false;
    }
    let actual=rf_read(bus,0,0xc5,1<<15)?;
    if actual==0{return Err(fault(0xc5,actual));}Ok(())
}
fn radio(bus:&mut impl RegisterIo,channel:u8,saved:&mut Cleanup)->io::Result<()>{
    for(path,at)in[(0,0x18),(1,0x18),(0,0x10018),(1,0x10018)]{
        let old=rf_read(bus,path,at,RF_MASK)?;
        let value=(old&!(0x30000|0xe000|0x300|255|0x1000))|u32::from(channel)|0x1000;
        if path==0&&at==0x18{arfc(bus,value,saved)?;lock(bus,saved)?;}else{rf(bus,path,at,RF_MASK,value)?;}
        rf(bus,path,0xcf,1,0)?;rf(bus,path,0xcf,1,1)?;
    }
    for(path,at)in[(0,0x18),(1,0x18),(0,0x10018),(1,0x10018)]{
        let old=rf_read(bus,path,at,RF_MASK)?;rf(bus,path,at,RF_MASK,(old&!(0xc00|0xe000|0x1000))|0x1c00)?;
    }
    for path in 0..2{
        saved.rx_lut[path as usize]=true;rf(bus,path,0xee,4,1)?;rf(bus,path,0x33,31,0x12)?;
        rf(bus,path,0x3f,63,0x1b)?;rf(bus,path,0xee,4,0)?;saved.rx_lut[path as usize]=false;
    }Ok(())
}
fn cleanup(bus:&mut impl RegisterIo,saved:&Cleanup)->io::Result<()>{
    let mut timed=TimedIo::new(bus,2_000);let mut first=None;
    let mut remember=|r:io::Result<()>|{if let Err(e)=r{if first.is_none(){first=Some(e);}}};
    if let Some(value)=saved.ldo{remember(rf(&mut timed,0,0xb1,RF_MASK,value));}
    if saved.trigger{remember(rf(&mut timed,0,0xd3,1<<8,0));}
    if saved.synlut{remember(rf(&mut timed,0,0xdd,1<<4,0));}
    if saved.mmd{remember(rf(&mut timed,0,0xd5,1<<8,0));}
    for path in 0..2{if saved.rx_lut[path as usize]{remember(rf(&mut timed,path,0xee,4,0));}}
    remember(timed.delay_us(0));first.map_or(Ok(()),Err)
}

/// Private initial 2 GHz managed-channel stage; the same-band DM prefix and real platform policy are mandatory.
/// It leaves ordinary DMA TX and the MAC scheduler stopped; RXDCK/IQK/TSSI/DPK and BTC channel work remain.
pub(crate) fn run<B:ChannelIo>(phy:&mut Phy<'_,B>,board:&Board,prefix:&dm::Prefix,configured:&tail::Configured,
    provenance:&DefaultTables,policy:&Policy)->Result<Tuned,Failure>{
    let fail=|cause|Failure{stage:Stage::Guard,cause,scheduler_restore:None,radio_cleanup:None,radio_stop:None};
    let session=phy.bus.command_session();let attempt=phy.attempt();
    if phy.state!=State::PathsConfigured||attempt==0||configured.attempt!=attempt||prefix.attempt()!=attempt
        ||prefix.band!=crate::dack::Band::Ghz2||session==0||configured.session!=session||provenance.session()!=session
        ||board.identity!=phy.identity||policy.country!=board.identity.country||policy.plan!=board.identity.channel_plan
        ||!(1..=3).contains(&configured.rx_path){return Err(fail(fault(0,phy.state as u32)));}
    let cal=calibration::decode(board).map_err(fail)?;
    let image=channel_power::build(policy.group,policy.channel,policy.limits).map_err(|_|fail(fault(0xd200,0)))?;
    let start=phy.bus.now_us();guard(phy.bus,&phy.identity,RegisterPhase::AfterDack).and_then(|()|{
        let value=io::read32(phy.bus,PHY_BASE+0x49c4)?;
        if value&15!=u32::from(configured.rx_path){return Err(fault(PHY_BASE+0x49c4,value));}
        left(phy.bus,start).map(|_|())
    })
        .map_err(|cause|{phy.mark_failed();fail(cause)})?;
    let mut stage=Stage::Pause;let mut pause_complete=false;let mut restore_attempted=false;let mut saved=Cleanup::default();
    let result=(||{
        let timeout=left(phy.bus,start)?.min(1_000_000)as u32;scheduler(phy.bus,timeout)?;pause_complete=true;
        let budget=left(phy.bus,start)?;let mut timed=TimedIo::new(phy.bus,budget);
        stage=Stage::Prepare;field(&mut timed,0xce40,1,0)?;tssi(&mut timed,true)?;bb(&mut timed,0x20fc,0xff000000,15)?;
        timed.delay_us(40)?;reset(&mut timed,false)?;
        stage=Stage::Mac;mac(&mut timed)?;stage=Stage::Baseband;baseband(&mut timed,policy.channel,configured.rx_path,&cal,prefix)?;
        stage=Stage::Radio;radio(&mut timed,policy.channel,&mut saved)?;
        stage=Stage::Power;for w in image.writes(){let old=timed.read32(w.at)?;timed.write32(w.at,(old&!w.mask)|w.bits)?;let value=timed.read32(w.at)?;
            if value&w.mask!=w.bits{return Err(fault(w.at,value));}}
        stage=Stage::Finish;timed.write32(0xce40,0x2b)?;
        let actual=io::read32(&mut timed,0xce40)?;if actual!=0x2b{return Err(fault(0xce40,actual));}
        field(&mut timed,0x9c18,3,1)?;
        bb(&mut timed,0x20fc,0xff000000,0)?;tssi(&mut timed,false)?;reset(&mut timed,true)?;timed.delay_us(0)?;
        stage=Stage::Restore;let timeout=left(phy.bus,start)?.min(1_000_000)as u32;restore_attempted=true;scheduler(phy.bus,timeout)?;
        stage=Stage::Verify;guard(phy.bus,&phy.identity,RegisterPhase::AfterDack)?;left(phy.bus,start)?;Ok(())
    })();
    if let Err(cause)=result{
        phy.mark_failed();let radio_cleanup=cleanup(phy.bus,&saved).err();let radio_stop=disable_bb_rf(phy.bus).err();
        let scheduler_restore=if pause_complete&&!restore_attempted{scheduler(phy.bus,1_000_000).err()}else{None};
        return Err(Failure{stage,cause,scheduler_restore,radio_cleanup,radio_stop});
    }
    phy.state=State::ChannelTuned;Ok(Tuned{attempt,session,channel:policy.channel,policy_version:policy.version,access:policy.access})
}
