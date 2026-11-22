// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2024 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! TSSI alignment with actual PMAC packets, 2.4 and 5 GHz (213-DRV-0087); native admission is intentionally absent.

use super::{Phy,State,RegisterPhase,PHY_BASE,guard,rf_read,rf_write_masked,disable_bb_rf,dm,tail,channel};
use crate::{band::{self,Subband},board::Board,calibration::{self,Calibration},dack::{self,Band,BtcNotice,RfkState,Calibration as Rfk},
    io::{self,RegisterIo,TimedIo,Fault,FaultKind},rxdck::RxdckIo};
#[path="../tssi_tables.rs"]pub(crate) mod tables;
const TOTAL_US:u64=4_500_000;
const RESTORE_US:u64=20_000;
const BACKUP:[usize;8]=[0x5820,0x7820,0x4978,0x58e4,0x78e4,0x49c0,0xd18,0xd80];

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) enum Stage{Guard,OuterStart,OneshotStart,Disable,ConfigureA,ConfigureB,Pause,WaitRx,AlignmentA,AlignmentB,Restore,Enable,Efuse,OneshotStop,OuterStop,Verify}
#[derive(Debug)]
pub(crate) struct Failure{pub stage:Stage,pub cause:Fault,pub pmac_stop:Option<Fault>,pub restore:Option<Fault>,
    pub scheduler_restore:Option<Fault>,pub restore_skipped:bool,pub disable_tssi:Option<Fault>,pub radio_stop:Option<Fault>,
    pub oneshot_stop:Option<Fault>,pub outer_stop:Option<Fault>}
impl Failure{pub(crate) fn guard(cause:Fault)->Self{Self{stage:Stage::Guard,cause,pmac_stop:None,restore:None,scheduler_restore:None,restore_skipped:false,
    disable_tssi:None,radio_stop:None,oneshot_stop:None,outer_stop:None}}}
#[derive(Debug)]
pub(crate) struct Completed{attempt:u64,session:u64,channel:u8,policy_version:u64,pub alignment:[[u32;4];2],pub reports:[[u16;2];2]}
impl Completed{
    pub(crate) fn matches(&self,tuned:&channel::Tuned)->bool{self.attempt==tuned.attempt&&self.session==tuned.session
        &&self.channel==tuned.channel&&self.policy_version==tuned.policy_version&&tuned.access==channel::Access::InitiateRadiation}
    pub(crate) fn is_tssi_mode(&self)->[bool;2]{[true;2]}
}
fn fault(at:usize,value:u32)->Fault{Fault::new(FaultKind::PowerState,at,value)}
fn left(bus:&mut impl RegisterIo,start:u64)->io::Result<u32>{
    let elapsed=bus.now_us().checked_sub(start).ok_or(Fault::new(FaultKind::Clock,0,0))?;
    TOTAL_US.checked_sub(elapsed).filter(|v|*v>0).map(|v|v as u32).ok_or(Fault::new(FaultKind::Timeout,0,0))
}
fn read(bus:&mut impl RegisterIo,at:usize,mask:u32)->io::Result<u32>{Ok((io::read32(bus,PHY_BASE+at)?&mask)>>mask.trailing_zeros())}
fn bb(bus:&mut impl RegisterIo,at:usize,mask:u32,value:u32)->io::Result<()>{
    let bits=(value<<mask.trailing_zeros())&mask;
    let before=bus.read32(PHY_BASE+at)?;bus.write32(PHY_BASE+at,(before&!mask)|bits)?;
    let actual=bus.read32(PHY_BASE+at)?;if actual&mask!=bits{return Err(fault(PHY_BASE+at,actual));}Ok(())
}
fn table(bus:&mut impl RegisterIo,table:&[tables::Write])->io::Result<()>{for &(at,mask,value)in table{bb(bus,at,mask,value)?;}Ok(())}
// Written unread, as Linux's rtw89_rfk_parser.
fn table_unread(bus:&mut impl RegisterIo,table:&[tables::Write])->io::Result<()>{
    for &(at,mask,value)in table{let before=bus.read32(PHY_BASE+at)?;bus.write32(PHY_BASE+at,(before&!mask)|((value<<mask.trailing_zeros())&mask))?;}Ok(())
}
fn disable(bus:&mut impl RegisterIo)->io::Result<()>{
    for path in 0..2{let p=path*0x2000;bb(bus,0x5820+p,1<<31,0)?;bb(bus,0x5818+p,0x18000000,1)?;bb(bus,0x58e4+p,1<<14,1)?;}Ok(())
}
fn enable(bus:&mut impl RegisterIo)->io::Result<()>{
    for path in 0..2{let p=path*0x2000;
        bb(bus,0x5814+p,1<<11,0)?;bb(bus,0x58e4+p,0xff800,0x10)?;bb(bus,0x58e4+p,1<<14,0)?;
        bb(bus,0x5820+p,1<<31,0)?;bb(bus,0x5820+p,1<<31,1)?;
        rf_write_masked(bus,path as u8,0x10055,1<<7,1)?;
        bb(bus,0x5818+p,0x18000000,3)?;bb(bus,0x5818+p,255,0xc0)?;
        bb(bus,0x5818+p,1<<28,0)?;bb(bus,0x5818+p,1<<28,1)?;
    }Ok(())
}
// _tssi_set_tmeter_tbl: the sub-band's thermal swing tables (rtw89_8852b_trk_cfg), 2.4 GHz or one of 5 GHz's three.
fn thermal_words(path:usize,thermal:u8,subband:Subband)->[u32;16]{
    let mut bytes=[0u8;64];
    if thermal!=255{
        let row=match subband{Subband::Ghz5Low=>0,Subband::Ghz5Middle=>1,_=>2};
        let (down,up)=match(subband,path){(Subband::Ghz2,0)=>(&tables::THERMAL_A_N,&tables::THERMAL_A_P),
            (Subband::Ghz2,_)=>(&tables::THERMAL_B_N,&tables::THERMAL_B_P),
            (_,0)=>(&tables::THERMAL_5G_A_N[row],&tables::THERMAL_5G_A_P[row]),(_,_)=>(&tables::THERMAL_5G_B_N[row],&tables::THERMAL_5G_B_P[row])};
        for j in 0..32{bytes[j]=(-down[j.min(29)])as u8;bytes[63-j]=up[(j+1).min(29)]as u8;}
    }
    core::array::from_fn(|j|u32::from_le_bytes(bytes[j*4..j*4+4].try_into().unwrap()))
}
fn thermal(bus:&mut impl RegisterIo,path:usize,value:u8,subband:Subband)->io::Result<()>{
    let p=path*0x2000;let reference=if value==255{32}else{u32::from(value)&63};
    bb(bus,0x5810+p,1<<16,0)?;bb(bus,0x5810+p,1<<24,1)?;
    bb(bus,0x5810+p,0xfc00,reference)?;bb(bus,0x5864+p,0x3f00000,reference)?;
    // The offset table is written unread, as Linux's write32: path B's read back changed (the Ryzen, 2026-11-06).
    for(i,word)in thermal_words(path,value,subband).into_iter().enumerate(){bus.write32(PHY_BASE+0x5c00+p+i*4,word)?;}
    bb(bus,0x5864+p,1<<26,1)?;bb(bus,0x5864+p,1<<26,0)
}
// rtw8852b_tssi's per-path setup: RF's TX power select (TXG on 2.4 GHz, TXA on 5 GHz), then the band's system, slope and
// alignment defaults, the sub-band's thermal table.
fn configure(bus:&mut impl RegisterIo,cal:&Calibration,path:usize,subband:Subband)->io::Result<()>{
    let five=subband!=Subband::Ghz2;
    rf_write_masked(bus,path as u8,0x7f,if five{1<<8}else{2},1)?;table(bus,tables::SYS_DEFS)?;
    let sys=match(five,path){(false,0)=>tables::SYS_A_DEFS_2G,(false,_)=>tables::SYS_B_DEFS_2G,(true,0)=>tables::SYS_A_DEFS_5G,(true,_)=>tables::SYS_B_DEFS_5G};
    let blocks=if path==0{[sys,tables::INIT_TXPWR_DEFS_A,tables::INIT_TXPWR_HE_TB_DEFS_A,tables::DCK_DEFS_A]}
        else{[sys,tables::INIT_TXPWR_DEFS_B,tables::INIT_TXPWR_HE_TB_DEFS_B,tables::DCK_DEFS_B]};
    for block in blocks{table(bus,block)?;}thermal(bus,path,cal.thermal_reference[path],subband)?;
    let slope=match(five,path){(false,0)=>tables::SLOPE_A_DEFS_2G,(false,_)=>tables::SLOPE_B_DEFS_2G,(true,0)=>tables::SLOPE_A_DEFS_5G,(true,_)=>tables::SLOPE_B_DEFS_5G};
    let align=match(subband,path){(Subband::Ghz2,0)=>tables::ALIGN_A_2G_ALL_DEFS,(Subband::Ghz2,_)=>tables::ALIGN_B_2G_ALL_DEFS,
        (Subband::Ghz5Low,0)=>tables::ALIGN_A_5G1_ALL_DEFS,(Subband::Ghz5Low,_)=>tables::ALIGN_B_5G1_ALL_DEFS,
        (Subband::Ghz5Middle,0)=>tables::ALIGN_A_5G2_ALL_DEFS,(Subband::Ghz5Middle,_)=>tables::ALIGN_B_5G2_ALL_DEFS,
        (Subband::Ghz5High,0)=>tables::ALIGN_A_5G3_ALL_DEFS,(Subband::Ghz5High,_)=>tables::ALIGN_B_5G3_ALL_DEFS};
    let blocks=if path==0{[tables::DAC_GAIN_DEFS_A,slope,align,tables::SLOPE_DEFS_A]}
        else{[tables::DAC_GAIN_DEFS_B,slope,align,tables::SLOPE_DEFS_B]};
    // Path B's DAC gain table read back changed at 0x7a10, 0x7a1c and 0x7a2c on the Ryzen (2026-11-08), as its thermal
    // table and RF 0x85 did (2026-11-06): path B is shared with Bluetooth.
    if path==1{table_unread(bus,blocks[0])?;}else{table(bus,blocks[0])?;}
    for block in &blocks[1..]{table(bus,block)?;}Ok(())
}
// _tssi_get_ofdm_group: the board's OFDM DE group, or between two (TSSI_EXTRA_GROUP: their average).
pub(crate) fn ofdm_group(channel:u8)->Option<(usize,bool)>{
    Some(match channel{1..=2=>(0,false),3..=5=>(1,false),6..=8=>(2,false),9..=11=>(3,false),12..=14=>(4,false),
        36..=40=>(5,false),41..=43=>(5,true),44..=48=>(6,false),49..=51=>(6,true),52..=56=>(7,false),57..=59=>(7,true),60..=64=>(8,false),
        100..=104=>(9,false),105..=107=>(9,true),108..=112=>(10,false),113..=115=>(10,true),116..=120=>(11,false),121..=123=>(11,true),
        124..=128=>(12,false),129..=131=>(12,true),132..=136=>(13,false),137..=139=>(13,true),140..=144=>(14,false),
        149..=153=>(15,false),154..=156=>(15,true),157..=161=>(16,false),162..=164=>(16,true),165..=169=>(17,false),170..=172=>(17,true),
        173..=177=>(18,false),_=>return None})
}
// _tssi_get_trim_group and _tssi_get_cck_group (0 off 2.4 GHz, as Linux's switch falls through to it).
pub(crate) fn trim_group(channel:u8)->Option<usize>{
    Some(match channel{1..=8=>0,9..=14=>1,36..=48=>2,52..=64=>3,100..=112=>4,116..=128=>5,132..=144=>6,149..=177=>7,_=>return None})
}
pub(crate) fn cck_group(channel:u8)->usize{match channel{3..=5=>1,6..=8=>2,9..=11=>3,12..=13=>4,14=>5,_=>0}}
// _tssi_set_efuse_to_de: CCK's DE from the CCK group, every MCS bandwidth's from the OFDM group, both with the trim.
fn efuse(bus:&mut impl RegisterIo,cal:&Calibration,channel:u8)->io::Result<()>{
    let (Some((group,between)),Some(trim_at))=(ofdm_group(channel),trim_group(channel))else{return Err(fault(0,u32::from(channel)))};
    for path in 0..2{let p=path*0x2000;let trim=i32::from(cal.tssi_trim[path][trim_at]);
        let ofdm=if between{(i32::from(cal.tssi_mcs[path][group])+i32::from(cal.tssi_mcs[path][group+1]))/2}else{i32::from(cal.tssi_mcs[path][group])};
        let cck=i32::from(cal.tssi_cck[path][cck_group(channel)])+trim;let mcs=ofdm+trim;
        for at in [0x5858,0x5860]{bb(bus,at+p,0x3ff000,cck as u32)?;}
        for at in [0x5838,0x5840,0x5848,0x5850,0x5828,0x5830]{bb(bus,at+p,0x3ff000,mcs as u32)?;}
    }Ok(())
}
fn wait_rx(bus:&mut impl RegisterIo,path:u8)->io::Result<()>{
    let mut timed=TimedIo::new(bus,5_000);
    for _ in 0..=2500{let value=rf_read(&mut timed,path,0,0xf0000)?;timed.delay_us(0)?;
        if value!=2{return Ok(());}timed.delay_us(2)?;}
    Err(Fault::new(FaultKind::Timeout,0,2))
}
fn scheduler(bus:&mut impl RxdckIo,timeout:u32)->io::Result<()>{
    let start=bus.now_us();bus.scheduler_zero(timeout)?;
    let elapsed=bus.now_us().checked_sub(start).ok_or(Fault::new(FaultKind::Clock,0,0))?;
    if elapsed>=u64::from(timeout){return Err(Fault::new(FaultKind::Timeout,0xc348,timeout));}
    let mut timed=TimedIo::new(bus,u64::from(timeout)-elapsed);
    let value=io::read32(&mut timed,0xc348)?;if value&0xffff!=0{return Err(fault(0xc348,value));}
    io::poll32(&mut timed,0xc6ec,1<<7,0,8,80_000)?;timed.delay_us(0)
}
// rtw8852bx_bb_set_pmac_tx's stop: CCK's CCA comes back on 2.4 GHz alone.
fn stop_pmac(bus:&mut impl RegisterIo,band:Band)->io::Result<()>{
    bb(bus,0x9c4,1<<4,0)?;bb(bus,0xc3c,1<<9,0)?;if band==Band::Ghz2{bb(bus,0x2344,1<<31,0)?;}Ok(())
}
fn normal_mode(bus:&mut impl RegisterIo)->io::Result<()>{
    for(at,mask)in[(0x980,1),(0x980,1<<16),(0x988,0xfff),(0x994,0xf0),(0x9a4,1<<10),(0x9a4,0x1c),(0x9a4,1<<16)]{bb(bus,at,mask,0)?;}Ok(())
}
fn pmac(bus:&mut impl RegisterIo,cal:&Calibration,prefix:&dm::Prefix,subband:Subband,original_rx:u8,path:Option<usize>,raw_power:u32)->io::Result<()>{
    table(bus,tables::PMAC_HT20_MCS7)?;bb(bus,0x9a4,0x1c,7)?;
    if let Some(path)=path{let rx=1<<path;bb(bus,0x458c,0xf0000000,rx)?;bb(bus,0x45b4,0x001e0000,0)?;
        tail::receive_path(bus,rx as u8)?;}
    tail::gain_offset(bus,cal,prefix,subband,original_rx)?;
    // RF_ABCD in the second source sample keeps the selected path but disables BTG and resets path B; no BTG on 5 GHz.
    tail::btg(bus,subband==Subband::Ghz2&&path==Some(1))?;let at=if path==Some(0){0x58dc}else{0x78dc};
    bb(bus,at,0xc0000000,1)?;bb(bus,at,0xc0000000,3)?;
    bb(bus,0x9a4,1<<16,1)?;bb(bus,0x4594,0x7fc00000,raw_power)?;
    bb(bus,0x980,1,1)?;bb(bus,0x980,1<<16,1)?;bb(bus,0x988,0xfff,0x3f)?;
    bb(bus,0x704,2,0)?;bb(bus,0xc3c,1<<9,1)?;bb(bus,0x2344,1<<31,1)?;bb(bus,0x704,2,1)?;
    bb(bus,0x9c4,1<<4,1)?;bb(bus,0x9c4,0xffffff00,5000)?;bb(bus,0x9c8,u32::MAX,100)?;
    bb(bus,0x9c0,1,1)?;bb(bus,0x9c0,1,0)
}
#[derive(Clone,Copy)]
struct Saved{bb:[u32;8],tx_path:u32,rx_path:u32,mode:[u32;4],tx_power:u32}
fn save(bus:&mut impl RegisterIo)->io::Result<Saved>{
    let tx_path=read(bus,0x458c,0xf0000000)?;let rx_path=read(bus,0x49c4,15)?;
    let mut mode=[0;4];for(i,at)in[0x12ac,0x12b0,0x32ac,0x32b0].into_iter().enumerate(){mode[i]=read(bus,at,u32::MAX)?;}
    let tx_power=read(bus,0x4594,0x7fc00000)?;let mut values=[0;8];for(i,at)in BACKUP.into_iter().enumerate(){values[i]=read(bus,at,u32::MAX)?;}
    Ok(Saved{bb:values,tx_path,rx_path,mode,tx_power})
}
fn restore(bus:&mut impl RegisterIo,saved:Saved)->io::Result<()>{
    for(at,value)in BACKUP.into_iter().zip(saved.bb){bb(bus,at,u32::MAX,value)?;}
    bb(bus,0x458c,0xf0000000,saved.tx_path)?;bb(bus,0x45b4,0x001e0000,if saved.tx_path==3{4}else{0})?;
    bb(bus,0x49c4,15,saved.rx_path)?;bb(bus,0x9a4,1<<16,1)?;
    for(at,value)in[0x12ac,0x12b0,0x32ac,0x32b0].into_iter().zip(saved.mode){bb(bus,at,u32::MAX,value)?;}
    bb(bus,0x4594,0x7fc00000,saved.tx_power)?;normal_mode(bus)
}
fn align(bus:&mut impl RegisterIo,cal:&Calibration,prefix:&dm::Prefix,subband:Subband,original_rx:u8,path:usize)->io::Result<([u32;4],[u16;2])>{
    for p in [0,0x2000]{bb(bus,0x5820+p,0xf000,8)?;bb(bus,0x58e4+p,0x3800,2)?;}
    let p=path*0x2000;let mut reports=[0u16;2];
    for (index,power)in[48,20].into_iter().enumerate(){
        bb(bus,0x5820+p,1<<31,0)?;bb(bus,0x5820+p,1<<31,1)?;
        // Completion is a fresh ready indication after the trigger, not a prior calibration's stale report.
        io::poll32(bus,PHY_BASE+0x1c18+p,1<<16,0,1,30)?;
        let _before=read(bus,0x1a40,65535)?;
        // power_2g and power_5g are the same: 48 and 20 quarter-dBm.
        pmac(bus,cal,prefix,subband,original_rx,(index==0).then_some(path),power)?;
        let value=io::poll32(bus,PHY_BASE+0x1c18+p,1<<16,1<<16,30,3_000)?;
        reports[index]=(value&511)as u16;stop_pmac(bus,subband.band())?;
    }
    let raw=read(bus,0x5630+p,u32::MAX)?;
    let signed=|v:u32|((v as i32)<<23)>>23;
    let diff=i32::from(reports[0])-56-i32::from(reports[1]);
    let values=[signed(raw>>20&1023)+diff,signed(raw>>10&1023)+diff,signed(raw&1023)+diff];
    let packed=((values[0]as u32&1023)<<20)|((values[1]as u32&1023)<<10)|(values[2]as u32&1023);
    bb(bus,0x5630+p,0x3fffffff,packed)?;bb(bus,0x563c+p,0x3fffffff,packed)?;
    let mut result=[0;4];for(i,at)in[0x5630,0x5634,0x563c,0x5640].into_iter().enumerate(){result[i]=read(bus,at+p,u32::MAX)?;}
    Ok((result,reports))
}
fn notice(band:Band,state:RfkState)->BtcNotice{BtcNotice{calibration:Rfk::Iqk,state,phy_map:match state{RfkState::Start|RfkState::Stop=>0x10,_=>0x13}|(band as u8)<<6}}

// The final caller must also supply the actual IQK completion and initialized BTC context; no native caller exists.
pub(crate) fn engine<B:RxdckIo>(phy:&mut Phy<'_,B>,board:&Board,prefix:&dm::Prefix,tuned:&channel::Tuned,
    admission:&channel::CalibrationTransmit)->Result<Completed,Failure>{
    if phy.state!=State::ChannelTuned||phy.attempt()!=tuned.attempt||prefix.attempt()!=tuned.attempt
        ||prefix.band!=Band::Ghz2||board.identity!=phy.identity||phy.bus.command_session()!=tuned.session
        ||!admission.matches(tuned,channel::CalibrationEngine::Tssi){return Err(Failure::guard(fault(0,phy.state as u32)));}
    let subband=band::subband(tuned.channel).ok_or(Failure::guard(fault(0,u32::from(tuned.channel))))?;let band=subband.band();
    let cal=calibration::decode(board).map_err(Failure::guard)?;let start=phy.bus.now_us();
    guard(phy.bus,&phy.identity,RegisterPhase::AfterDack).and_then(|()|left(phy.bus,start).map(|_|())).map_err(Failure::guard)?;
    let original_rx=read(phy.bus,0x49c4,15).map_err(Failure::guard)?as u8;
    if !(1..=3).contains(&original_rx){return Err(Failure::guard(fault(0x149c4,u32::from(original_rx))));}
    let bus=&mut *phy.bus;let mut stage=Stage::OuterStart;let mut outer=false;let mut outer_stop=false;
    let mut inner=false;let mut inner_stop=false;let mut paused=false;let mut restore_attempted=false;let mut saved=None;
    let mut result_values=[[0;4];2];let mut reports=[[0;2];2];
    let result=(||{
        let budget=left(bus,start)?.min(100_000);outer=true;dack::notify(bus,notice(band,RfkState::Start),budget)?;
        stage=Stage::OneshotStart;let budget=left(bus,start)?.min(100_000);inner=true;dack::notify(bus,notice(band,RfkState::OneshotStart),budget)?;
        stage=Stage::Disable;{let budget=left(bus,start)?;disable(&mut TimedIo::new(bus,u64::from(budget)))?;}
        for path in 0..2{
            stage=if path==0{Stage::ConfigureA}else{Stage::ConfigureB};let budget=left(bus,start)?;
            configure(&mut TimedIo::new(bus,u64::from(budget)),&cal,path,subband)?;
            stage=Stage::Pause;let budget=left(bus,start)?.min(1_000_000);scheduler(bus,budget)?;paused=true;restore_attempted=false;
            let budget=left(bus,start)?;let mut timed=TimedIo::new(bus,u64::from(budget));stage=Stage::WaitRx;
            wait_rx(&mut timed,0)?;wait_rx(&mut timed,1)?;saved=Some(save(&mut timed)?);
            stage=if path==0{Stage::AlignmentA}else{Stage::AlignmentB};
            let(values,rpt)=align(&mut timed,&cal,prefix,subband,original_rx,path)?;result_values[path]=values;reports[path]=rpt;
            stage=Stage::Restore;restore(&mut timed,saved.unwrap())?;saved=None;timed.delay_us(0)?;
            let budget=left(bus,start)?.min(1_000_000);restore_attempted=true;scheduler(bus,budget)?;paused=false;
        }
        let budget=left(bus,start)?;let mut timed=TimedIo::new(bus,u64::from(budget));stage=Stage::Enable;enable(&mut timed)?;
        stage=Stage::Efuse;efuse(&mut timed,&cal,tuned.channel)?;timed.delay_us(0)?;
        stage=Stage::OneshotStop;let budget=left(bus,start)?.min(100_000);inner_stop=true;dack::notify(bus,notice(band,RfkState::OneshotStop),budget)?;
        stage=Stage::OuterStop;let budget=left(bus,start)?.min(100_000);outer_stop=true;dack::notify(bus,notice(band,RfkState::Stop),budget)?;
        stage=Stage::Verify;guard(bus,&board.identity,RegisterPhase::AfterDack)?;left(bus,start)?;Ok(())
    })();
    if let Err(cause)=result{
        let pmac_stop={let mut timed=TimedIo::new(bus,RESTORE_US);stop_pmac(&mut timed,band).and_then(|()|normal_mode(&mut timed)).and_then(|()|timed.delay_us(0))}.err();
        // An unconfirmed PMAC stop forbids changing saved TX power/path before independently turning RF off.
        let restore_skipped=pmac_stop.is_some()&&saved.is_some();
        let early_radio=pmac_stop.is_some().then(||disable_bb_rf(bus).err());
        let restoration=if restore_skipped{None}else{saved.and_then(|saved|{let mut timed=TimedIo::new(bus,RESTORE_US);restore(&mut timed,saved).and_then(|()|timed.delay_us(0)).err()})};
        let disable_tssi={let mut timed=TimedIo::new(bus,RESTORE_US);disable(&mut timed).and_then(|()|timed.delay_us(0))}.err();
        let radio_stop=early_radio.unwrap_or_else(||disable_bb_rf(bus).err());
        let scheduler_restore=if paused&&!restore_attempted{scheduler(bus,1_000_000).err()}else{None};
        let oneshot_stop=if inner&&!inner_stop{dack::notify(bus,notice(band,RfkState::OneshotStop),100_000).err()}else{None};
        let outer_stop=if outer&&!outer_stop{dack::notify(bus,notice(band,RfkState::Stop),100_000).err()}else{None};
        phy.mark_failed();return Err(Failure{stage,cause,pmac_stop,restore:restoration,scheduler_restore,restore_skipped,disable_tssi,radio_stop,oneshot_stop,outer_stop});
    }
    Ok(Completed{attempt:tuned.attempt,session:tuned.session,channel:tuned.channel,policy_version:tuned.policy_version,alignment:result_values,reports})
}

#[cfg(test)]
pub(crate) fn modeled_engine<B:RxdckIo>(phy:&mut Phy<'_,B>,board:&Board,prefix:&dm::Prefix,tuned:&channel::Tuned,
    admission:&channel::CalibrationTransmit)->Result<Completed,Failure>{engine(phy,board,prefix,tuned,admission)}
#[cfg(test)]
pub(crate) fn modeled_thermal(path:usize,value:u8)->[u32;16]{thermal_words(path,value,Subband::Ghz2)}
#[cfg(test)]
pub(crate) fn modeled_thermal_in(path:usize,value:u8,subband:Subband)->[u32;16]{thermal_words(path,value,subband)}
