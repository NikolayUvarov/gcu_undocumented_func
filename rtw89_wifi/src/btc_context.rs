// SPDX-License-Identifier: BSD-3-Clause
// Copyright(c) 2019-2022 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.
//! Session-bound normal/no-link coexistence; completion is distinct from calibrated-radio readiness.

use crate::{board::Board,btc_codec::{Profile,Policy,Encoded},btc_control::{self,WlanShadow,BtState,SavedAntenna,Antenna},
    btc_init,btc_reports::{self,Event,Report},btc_session::{self,Session,DeviceIo},io::{self,RegisterIo,Fault,FaultKind,TimedIo},phy::{self,Phy,PhyIo}};
const INIT_US:u64=1_000_000;
const SCOREBOARD:usize=0xac;
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub enum Error{Session(btc_session::Error),Register(Fault),Profile,Order,Clock,Timeout,Events,Reports(btc_reports::Error),Whql,Busy,PolicyMismatch}
impl From<btc_session::Error> for Error{fn from(e:btc_session::Error)->Self{Self::Session(e)}}
impl From<Fault> for Error{fn from(e:Fault)->Self{Self::Register(e)}}
#[derive(Debug)]
pub struct Failure{pub cause:Error,pub initcfg:Option<btc_init::Failure>,pub restore_antenna:Option<Fault>,pub close_lut:Option<Fault>,pub disable_bb_rf:Option<Fault>}

/// Constructed only after actual initcfg, source commands/common actions and final owned-session validation.
/// It records one PHY attempt and command session, not WLAN authorization or a grant for later RF calibration.
#[derive(Debug)]
pub struct Initialized{attempt:u64,session:u64}
impl Initialized{
    pub(crate) fn matches(&self,attempt:u64,session:u64)->bool{attempt!=0&&session!=0&&self.attempt==attempt&&self.session==session}
    pub fn validate<B:DeviceIo>(&self,phy:&mut Phy<'_,Session<'_,'_,'_,B>>,timeout_us:u32)->Result<(),Error>{
        let attempt=phy.attempt();if phy.state()==phy::State::Failed{return Err(Error::Order)}
        let s=phy.bus_between_stages();let session=PhyIo::command_session(s);
        let context=s.btc.as_ref().ok_or(Error::Order)?;
        if !self.matches(attempt,session)||context.attempt!=attempt||context.session!=session{return Err(Error::Order)}
        s.receive_permit(timeout_us)?;Ok(())
    }
}

#[derive(Clone,Copy)]
struct ObservedBt{raw:u32,info:[u8;8],info_count:u32,version:u32}
impl ObservedBt{
    fn new()->Self{Self{raw:0,info:[0;8],info_count:0,version:0}}
    fn state(&self)->BtState{BtState{enabled:self.raw&2!=0,patched:self.raw&1<<30!=0,running_rfk:self.raw&32!=0,requests_rfk:self.raw&64!=0}}
    fn profiles(&self)->bool{self.info[2]&0xf0!=0}
    fn lna(&self)->u8{4+u8::from(self.raw&1<<8!=0)+2*u8::from(self.raw&1<<10!=0)}
    fn update_score(&mut self,raw:u32)->bool{
        if self.raw&2==0&&raw&2!=0{self.info=[0;8];self.info_count=0;self.version=0;}
        let changed=(self.raw^raw)&((1<<30)|2|4|32|64|1<<8|1<<10|1<<13)!=0;self.raw=raw;changed
    }
}

pub(crate) struct Context{
    attempt:u64,session:u64,profile:Profile,shadow:WlanShadow,policy:Policy,reports:u32,bt:ObservedBt,
    published:Option<u32>,btg:bool,rx_gain:bool,bt_lna:u8,afh_profiles:Option<u8>,ignore_pending:bool,dirty:bool,
    observed_tdma:Option<[u8;8]>,observed_slots:Option<[btc_reports::Slot;18]>,other_events:u32,
}
impl Context{
    fn new(profile:Profile,attempt:u64,session:u64)->Self{let mut shadow=WlanShadow::new();shadow.initialize();Self{
        attempt,session,profile,shadow,policy:Policy::ResetDefaults,reports:0,bt:ObservedBt::new(),published:None,btg:false,rx_gain:false,bt_lna:0,
        afh_profiles:None,ignore_pending:false,dirty:true,observed_tdma:None,observed_slots:None,other_events:0}}
    fn events<B:DeviceIo>(&mut self,s:&mut Session<'_,'_,'_,B>)->Result<(),Error>{
        while let Some(event)=s.event(){
            match btc_reports::parse_105(event.bytes).map_err(Error::Reports)?{
                Event::BtInfo(bytes)=>{self.bt.info_count=self.bt.info_count.saturating_add(1);
                    if self.bt.info!=*bytes{self.bt.info=*bytes;self.ignore_pending|=bytes[5]&8!=0;self.dirty=true;}},
                Event::Scoreboard(_)=>{let raw=io::read32(s,SCOREBOARD)?;self.dirty|=self.bt.update_score(raw);},
                Event::Reports(reports)=>for report in reports.iter(){match report{
                    Report::Tdma(t)=>self.observed_tdma=Some(t.fields),
                    Report::Slots(v)=>self.observed_slots=Some(core::array::from_fn(|i|v.get(i).unwrap())),
                    Report::BtVersion(v)=>{self.dirty|=self.bt.version!=v.firmware_version;self.bt.version=v.firmware_version;},
                    Report::Other{..}=>self.other_events=self.other_events.saturating_add(1),
                }},
                Event::Overflow(_)=>return Err(Error::Events),
                Event::Other{..}|Event::BtRegister(_)|Event::RunInfo(_)|Event::Psd(_)|Event::DeviceInfo(_)|Event::Loopback(_)=>self.other_events=self.other_events.saturating_add(1),
            }
            s.consume_event();
        }Ok(())
    }
    fn send<B:DeviceIo>(&mut self,s:&mut Session<'_,'_,'_,B>,message:Encoded,deadline:u64)->Result<(),Error>{
        s.command_before(message.command(),message.bytes(),message.acks(),deadline)?;self.events(s)
    }
    fn report<B:DeviceIo>(&mut self,s:&mut Session<'_,'_,'_,B>,mask:u32,enabled:bool,deadline:u64)->Result<(),Error>{
        let next=if enabled{self.reports|mask}else{self.reports&!mask};if next==self.reports{return Ok(())}
        self.send(s,self.profile.reports(next,enabled).map_err(|_|Error::Profile)?,deadline)?;self.reports=next;Ok(())
    }
    fn policy<B:DeviceIo>(&mut self,s:&mut Session<'_,'_,'_,B>,next:Policy,deadline:u64)->Result<(),Error>{
        self.shadow.clear(1<<9);
        if let Some(message)=self.profile.policy(self.policy,next){
            self.drain(s,deadline)?;self.send(s,message,deadline)?;
            // Reports preceding command completion are transition snapshots, not observations of the new policy.
            self.observed_tdma=None;self.observed_slots=None;
        }
        self.policy=next;Ok(())
    }
    fn policy_consistent(&self)->Result<(),Error>{
        if self.observed_tdma.is_some_and(|v|v!=self.policy.tdma()){return Err(Error::PolicyMismatch)}
        if let Some(slots)=self.observed_slots{for (index,slot)in slots.iter().enumerate(){
            if (slot.duration,slot.table,slot.kind)!=self.policy.slot(index){return Err(Error::PolicyMismatch)}
        }}Ok(())
    }
    fn common<B:DeviceIo>(&mut self,s:&mut Session<'_,'_,'_,B>,initial:bool,deadline:u64)->Result<(),Error>{
        let bt=self.bt.state();let btg=bt.enabled&&bt.patched;
        if self.btg!=btg{s.registers(deadline,|b|btc_control::btg(b,btg))?;self.btg=btg;}
        if initial{s.registers(deadline,|b|btc_control::preagc_off(b))?;}
        let profiles=(self.bt.info[2]&0xf0).count_ones()as u8;
        if self.afh_profiles!=Some(profiles){self.send(s,self.profile.afh_no_link(),deadline)?;self.afh_profiles=Some(profiles);}
        if self.bt.raw&1<<13!=0{self.shadow.clear(1<<13);}
        let constrained=btg&&self.bt.profiles();
        if constrained!=self.rx_gain{s.registers(deadline,|b|receive_gain(b,constrained))?;self.rx_gain=constrained;}
        if self.ignore_pending{self.ignore_pending=false;self.send(s,self.profile.bt_ignore_wlan(),deadline)?;}
        if self.bt.info_count>0&&(self.bt_lna!=7||self.bt.lna()!=7){
            self.shadow.clear(1<<4);self.send(s,self.profile.bt_lna_normal(),deadline)?;self.bt_lna=7;
        }
        self.shadow.clear(1<<12);
        let ver=self.bt.version&0xffff;
        self.report(s,1<<3,bt.enabled&&(ver==0||ver==0x8852&&bt.patched),deadline)?;
        if initial{self.send(s,self.profile.driver_no_link(),deadline)?;self.report(s,1<<2,true,deadline)?;}
        if self.published!=Some(self.shadow.value()){
            let remaining=left(s,deadline)?.min(100_000);
            s.registers(deadline,|b|self.shadow.publish(b,remaining))?;self.published=Some(self.shadow.value());
        }Ok(())
    }
    fn drain<B:DeviceIo>(&mut self,s:&mut Session<'_,'_,'_,B>,deadline:u64)->Result<(),Error>{
        for _ in 0..1024{self.events(s)?;let progress=s.poll_before(deadline)?;self.events(s)?;if progress==[false,false]{return Ok(())}}
        Err(Error::Events)
    }
    fn init<B:DeviceIo>(&mut self,s:&mut Session<'_,'_,'_,B>,deadline:u64)->Result<(),Error>{
        let raw=io::read32(s,SCOREBOARD)?;self.bt.update_score(raw);self.drain(s,deadline)?;
        if self.bt.raw&4!=0{return Err(Error::Whql)}
        s.registers(deadline,|b|btc_control::default_power(b,100_000))?;
        self.send(s,self.profile.monitor_table(),deadline)?;self.report(s,1<<2,true,deadline)?;
        self.send(s,self.profile.slot_defaults(),deadline)?;self.send(s,self.profile.driver_init(),deadline)?;
        self.send(s,self.profile.driver_control(true),deadline)?;
        s.registers(deadline,|b|btc_control::antenna(b,Antenna::Initial,100_000).map(|_|()))?;
        self.policy(s,Policy::NoLink,deadline)?;
        for turn in 0..8{
            self.dirty=false;
            if turn!=0{let desired=if self.bt.state().enabled{Antenna::NoLink}else{Antenna::WlanOnly};
                s.registers(deadline,|b|btc_control::antenna(b,desired,100_000).map(|_|()))?;self.policy(s,Policy::NoLink,deadline)?;}
            self.common(s,turn==0,deadline)?;self.drain(s,deadline)?;
            let raw=io::read32(s,SCOREBOARD)?;self.dirty|=self.bt.update_score(raw);
            if self.bt.raw&4!=0{return Err(Error::Whql)}
            if !self.dirty{return self.policy_consistent()}
        }Err(Error::Events)
    }
}
fn left<B:DeviceIo>(s:&mut Session<'_,'_,'_,B>,deadline:u64)->Result<u32,Error>{
    deadline.checked_sub(s.clock()?).filter(|v|*v>0).map(|v|v.min(u32::MAX as u64)as u32).ok_or(Error::Timeout)
}

/// Normal, no-link, no-LPS startup only; no policy context may already own this firmware session.
pub fn initialize<B:DeviceIo>(phy:&mut Phy<'_,Session<'_,'_,'_,B>>,board:&Board)->Result<Initialized,Failure>{
    let bare=|cause|Failure{cause,initcfg:None,restore_antenna:None,close_lut:None,disable_bb_rf:None};
    let attempt=phy.attempt();let s=phy.bus_between_stages();
    if s.failed().is_some()||s.btc.is_some(){return Err(bare(Error::Order))}
    let session=PhyIo::command_session(s);let version=s.firmware_version();
    let profile=Profile::select_version(board,version).map_err(|_|bare(Error::Profile))?;
    let config=match btc_init::prepare_inner(phy,board,version,session){
        Ok(config)=>config,Err(failure)=>{phy.bus_between_stages().abort_coexistence();phy.mark_failed();return Err(Failure{cause:Error::Order,initcfg:Some(failure),restore_antenna:None,close_lut:None,disable_bb_rf:None})}
    };
    if !config.matches(attempt,session){phy.bus_between_stages().abort_coexistence();phy.mark_failed();return Err(bare(Error::Order))}
    let s=phy.bus_between_stages();let mut context=Context::new(profile,attempt,session);let mut saved=None;
    let result=(||{let deadline=s.clock()?.checked_add(INIT_US).ok_or(Error::Clock)?;
        saved=Some(s.registers(deadline,|b|SavedAntenna::read(b,200_000))?);
        context.init(s,deadline)?;let remaining=left(s,deadline)?.min(10_000);s.receive_permit(remaining)?;left(s,deadline)?;Ok::<_,Error>(())})();
    if let Err(cause)=result{
        let restore_antenna=saved.as_ref().and_then(|v|v.restore(s,200_000).err());
        let close_lut=close_receive_lut(&mut TimedIo::new(s,1000)).err();let disable_bb_rf=phy::disable_bb_rf(s).err();
        s.abort_coexistence();phy.mark_failed();return Err(Failure{cause,initcfg:None,restore_antenna,close_lut,disable_bb_rf});
    }
    s.btc=Some(context);Ok(Initialized{attempt,session})
}

// Shared-antenna RF levels zero and one use WLAN gain zero and two, respectively, with pre-AGC disabled.
fn receive_gain(bus:&mut impl RegisterIo,constrained:bool)->io::Result<()> {
    btc_control::preagc_off(bus)?;
    phy::rf_write_masked(bus,1,0xef,0xfffff,0x1000)?;
    for (at,value)in[(0,0x15),(1,if constrained{5}else{0x17}),(2,0x15),(3,if constrained{5}else{0x17})]{
        phy::rf_write_masked(bus,1,0x33,0xfffff,at)?;phy::rf_write_masked(bus,1,0x3f,0xfffff,value)?;
    }close_receive_lut(bus)
}
fn close_receive_lut(bus:&mut impl RegisterIo)->io::Result<()> {
    phy::rf_write_masked(bus,1,0xef,0xfffff,0)?;let value=phy::rf_read(bus,1,0xef,0xfffff)?;
    if value!=0{return Err(Fault::new(FaultKind::PowerState,0xef,value))}Ok(())
}
