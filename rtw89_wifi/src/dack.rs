// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! Bounded RTL8852B DACK engine; actual BTC arbitration and the pre-NCTL DM setup remain mandatory.

use crate::dack_tables::{self as tables, Op};
use crate::io::{self, Fault, FaultKind, RegisterIo, TimedIo};
use crate::phy::{self, PhyIo, PHY_BASE};

const TOTAL_US:u64=400_000;
const RF_MASK:u32=0xfffff;
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
#[repr(u8)]
pub enum Band { Ghz2=0, Ghz5=1 }
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
#[repr(u8)]
pub enum Calibration { Iqk=0, Dpk=2, Dack=4, Rxdck=5 }
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
#[repr(u8)]
pub enum RfkState { Stop=0, Start=1, OneshotStart=2, OneshotStop=3 }
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub struct BtcNotice { pub calibration:Calibration, pub state:RfkState, pub phy_map:u8 }

/// Complete actual BT RFK arbitration, scoreboard/coexistence actions and cancellation within the supplied budget.
/// START success means calibration was allowed; H2C receipt or DMA consumption alone is insufficient.
/// STOP must release a possibly applied START after errors, without enabling any packet TX or dropping DMA memory.
pub trait DackIo:PhyIo { fn btc_rfk(&mut self, notice:BtcNotice, timeout_us:u32)->io::Result<()>; }

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub enum Stage { Guard, BtcStart, Backup, Afe, Drck, AddckA, AddckB, AddckReload, DackA, DackB, Restore, BtcStop }
#[derive(Debug)]
pub struct Failure {
    pub stage:Stage, pub cause:Fault, pub btc_stop:Option<Fault>,
    pub rf_restore:[Option<Fault>;2], pub radio_stop:Option<Fault>,
}
impl Failure {
    pub(crate) fn guard(cause:Fault)->Self { Self{stage:Stage::Guard,cause,btc_stop:None,rf_restore:[None;2],radio_stop:None} }
}
#[derive(Debug,Eq,PartialEq)]
pub struct ResultData {
    pub drck:u8, pub addck:[[u16;2];2], pub msbk:[[[u8;16];2];2], pub bias:[[u16;2];2], pub dadck:[[u8;2];2],
    pub completion:[[u8;2];2], pub diagnostic_dc:[[i32;2];5],
}
impl ResultData {
    fn new()->Self { Self{drck:0,addck:[[0;2];2],msbk:[[[0;16];2];2],bias:[[0;2];2],dadck:[[0;2];2],completion:[[0;2];2],diagnostic_dc:[[0;2];5]} }
}
fn field(bus:&mut impl RegisterIo, at:u32, mask:u32, value:u32)->io::Result<()> {
    if mask==u32::MAX {bus.write32(PHY_BASE+at as usize,value)}
    else {io::modify32(bus,PHY_BASE+at as usize,mask,(value<<mask.trailing_zeros())&mask)}
}
fn read(bus:&mut impl RegisterIo, at:u32, mask:u32)->io::Result<u32> {
    Ok((io::read32(bus,PHY_BASE+at as usize)?&mask)>>mask.trailing_zeros())
}
fn table(bus:&mut impl RegisterIo, table:&[Op])->io::Result<()> {
    for op in table {
        match *op {
            Op::Bb(at,mask,value)=>field(bus,at,mask,value)?,
            Op::Rf(path,at,mask,value)=>phy::rf_write_masked(bus,path,at,mask,value)?,
            Op::Delay(us)=>bus.delay_us(us)?,
        }
    }
    bus.delay_us(0)
}
fn poll(bus:&mut impl RegisterIo, at:u32, mask:u32)->io::Result<()> {
    io::poll32(bus,PHY_BASE+at as usize,mask,mask,1,10_000).map(|_|())
}
fn diagnose(bus:&mut impl RegisterIo, path:usize)->io::Result<[i32;2]> {
    table(bus,if path==0 {tables::CHECK_ADDC_DEFS_A} else {tables::CHECK_ADDC_DEFS_B})?;
    let mut sum=[0;2];
    for _ in 0..100 {
        let value=read(bus,0x1730,u32::MAX)?;
        sum[0]+=((value<<8) as i32)>>20; sum[1]+=((value<<20) as i32)>>20;
    }
    bus.delay_us(0)?; Ok([sum[0]/100,sum[1]/100])
}
fn drck(bus:&mut impl RegisterIo)->io::Result<u8> {
    field(bus,0xc0cc,1<<6,1)?; poll(bus,0xc0d0,8)?;
    field(bus,0xc0cc,1<<6,0)?; field(bus,0xc094,1<<9,1)?; bus.delay_us(1)?; field(bus,0xc094,1<<9,0)?;
    let value=read(bus,0xc0d0,0xf8000)?; field(bus,0xc0cc,1<<9,0)?; field(bus,0xc0cc,31,value)?; Ok(value as u8)
}
fn addck(bus:&mut impl RegisterIo,path:usize,diagnostics:&mut [[i32;2]])->io::Result<()> {
    let base=0xc000+(path as u32)*0x100; let debug=if path==0 {0x12b8} else {0x32b8};
    if path==0 {field(bus,0xc0f4,0x30,0)?;field(bus,0xc1d4,0x30,0)?;}
    field(bus,debug,1<<30,1)?; field(bus,0x032c,1<<30,0)?;
    field(bus,0x032c,1<<22,0)?; field(bus,0x032c,1<<22,1)?; field(bus,0x030c,0xf000000,15)?;
    field(bus,0x032c,1<<16,0)?; field(bus,base+0xd4,2,1)?; field(bus,0x030c,0xf000000,3)?;
    diagnostics[0]=diagnose(bus,path)?;
    field(bus,base+0xf4,1<<11,1)?; field(bus,base+0xf4,1<<11,0)?; bus.delay_us(1)?; field(bus,base+0xf4,0x300,1)?;
    poll(bus,base+0xfc,1)?; diagnostics[1]=diagnose(bus,path)?;
    field(bus,base+0xd4,2,0)?; field(bus,0x032c,1<<16,1)?; field(bus,0x030c,0xf000000,12)?;
    field(bus,0x032c,1<<30,1)?; field(bus,debug,1<<30,0)
}
fn reload_addck(bus:&mut impl RegisterIo,data:&mut ResultData)->io::Result<()> {
    for path in 0..2 {
        let base=0xc000+path as u32*0x100;
        field(bus,base+0xf4,0x300,0)?;
        data.addck[path][0]=read(bus,base+0xfc,0xffc00)? as u16;
        data.addck[path][1]=read(bus,base+0xfc,0x3ff)? as u16;
    }
    for path in 0..2 {
        let base=0xc000+path as u32*0x100; let a=data.addck[path];
        field(bus,base+0xf0,0x3ff0000,a[0] as u32)?; field(bus,base+0xf4,15,(a[1]>>6) as u32)?;
        field(bus,base+0xf0,0xfc000000,(a[1]&63) as u32)?; field(bus,base+0xf4,0x30,3)?;
    }
    Ok(())
}
fn poll_dack(bus:&mut impl RegisterIo,path:usize,part:usize)->io::Result<u8> {
    let base=0xc000+path as u32*0x100;
    let (a,b,mask)=if part==0 {(base+0x40,base+0x64,1<<31)} else {(base+0x5c,base+0x80,4)};
    let mut timed=TimedIo::new(bus,10_000);
    for _ in 0..=10_000 {
        let av=read(&mut timed,a,mask)?!=0; let bv=read(&mut timed,b,mask)?!=0;
        timed.delay_us(0)?;
        // The pinned RTL8852B source requires both flags on S0 and either flag on S1.
        if if path==0 {av&&bv} else {av||bv} {return Ok(av as u8|((bv as u8)<<1));}
        timed.delay_us(1)?;
    }
    Err(Fault::new(FaultKind::Timeout,PHY_BASE+a as usize,0))
}
fn backup_dack(bus:&mut impl RegisterIo,path:usize,data:&mut ResultData)->io::Result<()> {
    let base=0xc000+path as u32*0x100;
    field(bus,if path==0 {0x12b8} else {0x32b8},1<<30,1)?;
    for i in 0..16 {
        field(bus,base,30,i as u32)?; data.msbk[path][0][i]=read(bus,base+0x5c,0xff000000)? as u8;
        field(bus,base+0x20,30,i as u32)?; data.msbk[path][1][i]=read(bus,base+0x80,0xff000000)? as u8;
    }
    for (index,at) in [base+0x48,base+0x6c].into_iter().enumerate() {data.bias[path][index]=read(bus,at,0xffc)? as u16;}
    for (index,at) in [base+0x60,base+0x84].into_iter().enumerate() {data.dadck[path][index]=read(bus,at,0xff000000)? as u8;}
    Ok(())
}
fn dack_path(bus:&mut impl RegisterIo,path:usize,data:&mut ResultData)->io::Result<()> {
    let tables=if path==0 {[tables::DACK_S0_1_DEFS,tables::DACK_S0_2_DEFS,tables::DACK_S0_3_DEFS]}
        else {[tables::DACK_S1_1_DEFS,tables::DACK_S1_2_DEFS,tables::DACK_S1_3_DEFS]};
    table(bus,tables[0])?; data.completion[path][0]=poll_dack(bus,path,0)?;
    table(bus,tables[1])?; data.completion[path][1]=poll_dack(bus,path,1)?; table(bus,tables[2])?;
    if path==1 {
        table(bus,tables::CHECK_DADC_EN_DEFS_B)?; data.diagnostic_dc[4]=diagnose(bus,1)?; table(bus,tables::CHECK_DADC_DIS_DEFS_B)?;
    }
    backup_dack(bus,path,data)?; field(bus,if path==0 {0x12b8} else {0x32b8},1<<30,0)
}
fn remaining(bus:&mut impl RegisterIo,start:u64)->io::Result<u32> {
    let elapsed=bus.now_us().checked_sub(start).ok_or(Fault::new(FaultKind::Clock,0,0))?;
    TOTAL_US.checked_sub(elapsed).filter(|v|*v!=0).map(|v|v as u32).ok_or(Fault::new(FaultKind::Timeout,0,0))
}
pub(crate) fn notify(bus:&mut impl DackIo,notice:BtcNotice,timeout_us:u32)->io::Result<()> {
    let start=bus.now_us(); let result=bus.btc_rfk(notice,timeout_us);
    let elapsed=bus.now_us().checked_sub(start).ok_or(Fault::new(FaultKind::Clock,0,0))?;
    result?;
    if elapsed>=u64::from(timeout_us) {return Err(Fault::new(FaultKind::Timeout,0,timeout_us));} Ok(())
}
fn notice(band:Band,state:RfkState)->BtcNotice {BtcNotice{calibration:Calibration::Dack,state,phy_map:0x10|((band as u8)<<6)}}
fn restore(bus:&mut impl RegisterIo,path:u8,at:u32,value:u32)->io::Result<()> {
    phy::rf_write_masked(bus,path,at,RF_MASK,value)?;
    let actual=phy::rf_read(bus,path,at,RF_MASK)?;
    if actual!=value {return Err(Fault::new(FaultKind::PowerState,at as usize,actual));} bus.delay_us(0)
}

pub(crate) fn run(bus:&mut impl DackIo,band:Band,start:u64)->core::result::Result<ResultData,Failure> {
    let mut stage=Stage::BtcStart; let mut start_attempted=false; let mut stop_attempted=false; let mut stop_error=None;
    let mut saved=[[None;2];2]; let mut modified=false;
    let result=(|| {
        let left=remaining(bus,start)?; start_attempted=true;
        notify(bus,notice(band,RfkState::Start),left.min(100_000))?;
        let left=remaining(bus,start)?;
        let mut io=TimedIo::new(bus,left as u64);
        stage=Stage::Backup;
        for path in 0..2 {
            saved[path][0]=Some(phy::rf_read(&mut io,path as u8,0,RF_MASK)?);
            saved[path][1]=Some(phy::rf_read(&mut io,path as u8,5,RF_MASK)?);
        }
        let mut data=ResultData::new();
        stage=Stage::Afe; modified=true;
        io.write32(0x8040,15)?; table(&mut io,tables::AFE_INIT_DEFS)?;
        stage=Stage::Drck; data.drck=drck(&mut io)?;
        for path in 0..2 {phy::rf_write_masked(&mut io,path,5,1,0)?;}
        for path in 0..2 {phy::rf_write_masked(&mut io,path,0,RF_MASK,0x337e1)?;}
        stage=Stage::AddckA; addck(&mut io,0,&mut data.diagnostic_dc[..2])?;
        stage=Stage::AddckB; addck(&mut io,1,&mut data.diagnostic_dc[2..4])?;
        stage=Stage::AddckReload; reload_addck(&mut io,&mut data)?;
        for path in 0..2 {phy::rf_write_masked(&mut io,path,1,RF_MASK,0)?;}
        stage=Stage::DackA; dack_path(&mut io,0,&mut data)?;
        stage=Stage::DackB; dack_path(&mut io,1,&mut data)?;
        stage=Stage::Restore;
        for path in 0..2 {restore(&mut io,path as u8,0,saved[path][0].unwrap())?;}
        for path in 0..2 {
            phy::rf_write_masked(&mut io,path,5,1,1)?;
            let actual=phy::rf_read(&mut io,path,5,RF_MASK)?;
            if actual!=(saved[path as usize][1].unwrap()|1) {return Err(Fault::new(FaultKind::PowerState,5,actual));}
        }
        io.delay_us(0)?;
        stage=Stage::BtcStop;
        let left=remaining(bus,start)?; stop_attempted=true;
        if let Err(error)=notify(bus,notice(band,RfkState::Stop),left.min(100_000)) {stop_error=Some(error);return Err(error);}
        remaining(bus,start)?;
        Ok(data)
    })();
    match result {
        Ok(data)=>Ok(data),
        Err(cause)=>{
            let mut rf_restore=[None;2];
            if modified {
                let mut cleanup=TimedIo::new(bus,1_000);
                for path in 0..2 {
                    for (index,at) in [0u32,5].into_iter().enumerate() {
                        if let Some(value)=saved[path][index] {
                            if let Err(error)=restore(&mut cleanup,path as u8,at,value) {
                                if rf_restore[path].is_none() {rf_restore[path]=Some(error);}
                            }
                        }
                    }
                }
            }
            let radio_stop=if start_attempted {phy::disable_bb_rf(bus).err()} else {None};
            let btc_stop=if start_attempted&&!stop_attempted {notify(bus,notice(band,RfkState::Stop),100_000).err()} else {stop_error};
            Err(Failure{stage,cause,btc_stop,rf_restore,radio_stop})
        }
    }
}
