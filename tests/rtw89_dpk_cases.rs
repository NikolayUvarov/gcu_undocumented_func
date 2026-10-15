use super::*;
use crate::{dack::{DackIo,BtcNotice,RfkState},phy::PhyIo};

struct Device{
    memory:Vec<u8>,rf:[[u32;256];2],xtal:[u8;256],selected:(usize,usize),now:u64,frozen:bool,
    writes:Vec<(usize,u32)>,commands:Vec<(u8,u8)>,btc:Vec<(BtcNotice,usize)>,schedulers:Vec<usize>,
    fail_write:Option<usize>,fail_cleanup:Option<usize>,missing_first:Option<usize>,missing_second:Option<usize>,
    loss:u8,dgain:u16,correlation:u8,dc:[u16;2],pas:[(u16,u16);2],
    start_fail:bool,stop_fail:bool,scheduler_fail:Option<usize>,scheduler_delay:u64,late_delay:Option<u32>,late_write:Option<usize>,
}
impl Device{
    fn new()->Self{
        let mut d=Self{memory:vec![0;0x20000],rf:[[0;256];2],xtal:[0;256],selected:(0,0),now:0,frozen:false,writes:Vec::new(),commands:Vec::new(),btc:Vec::new(),schedulers:Vec::new(),
            fail_write:None,fail_cleanup:None,missing_first:None,missing_second:None,loss:3,dgain:0x600,correlation:200,dc:[1,0xffe],pas:[(100,0),(100,0)],
            start_fail:false,stop_fail:false,scheduler_fail:None,scheduler_delay:0,late_delay:None,late_write:None};
        d.set(2,3);d.set(0x2f0,1<<17);d.xtal[0x80]=0xc7;d.xtal[0x81]=0xc7;
        for(path,values)in d.rf.iter_mut().enumerate(){for(at,v)in values.iter_mut().enumerate(){*v=((path+1)*0x1000+at)as u32;}values[0]=0x31234;values[0x42]=42<<1;}
        for(path,base)in [0x1e000,0x1f000].into_iter().enumerate(){d.set(base+20,0x54321+path as u32);}
        for(at,value)in[(0x12344,0x12345678),(0x15800,0x23456789),(0x17800,0x34567890),(0x1813c,0x22220000),(0x1823c,0x33330000),(0x18124,0x33331111),(0x18224,0x44442222),(0x18120,0x55554444),(0x18220,0x66665555),(0x15818,0x1234),(0x17818,0x4321)]{d.set(at,value);}
        d
    }
    fn set(&mut self,at:usize,v:u32){self.memory[at..at+4].copy_from_slice(&v.to_le_bytes());}
    fn get(&self,at:usize)->u32{u32::from_le_bytes(self.memory[at..at+4].try_into().unwrap())}
    fn tick(&mut self,us:u64){if !self.frozen{self.now+=us;}}
    fn read(&mut self,at:usize,width:usize)->io::Result<u32>{
        io::check_offset(at,width,self.memory.len())?;self.tick(1);
        if width==4&&at==0x1174c{return Ok(1<<26|self.rf[self.selected.0][self.selected.1]);}
        if width==4&&at==0x1bff8{return Ok(if self.missing_first==Some(self.commands.len()){0}else{0x55});}
        if width==4&&at==0x180fc{
            let selector=(self.get(0x180d4)>>16)&0x3f;
            return Ok(match selector{
                3=>if self.missing_second==Some(self.commands.len()){0}else{0x8000},
                0=>(u32::from(self.dgain)<<16)|(u32::from(self.correlation)<<8)|7,
                9=>(u32::from(self.dc[0])<<16)|u32::from(self.dc[1]),
                6=>if self.get(0x180bc)&(1<<14)!=0{u32::from(if self.loss==255{if self.commands.iter().filter(|(_,id)|*id==0x13).count()%2==1{7}else{0}}else{self.loss})<<4}else{let index=usize::from(self.get(0x180c0)>>24==31);u32::from(self.pas[index].0)<<16|u32::from(self.pas[index].1)},
                _=>0,
            });
        }
        let mut bytes=[0;4];bytes[..width].copy_from_slice(&self.memory[at..at+width]);Ok(u32::from_le_bytes(bytes))
    }
    fn write(&mut self,at:usize,width:usize,value:u32)->io::Result<()>{
        io::check_offset(at,width,self.memory.len())?;self.tick(1);self.writes.push((at,value));self.memory[at..at+width].copy_from_slice(&value.to_le_bytes()[..width]);
        if at==0x10378{self.selected=((value>>8&1)as usize,(value&255)as usize);}
        if at==0x10370{let path=(value>>28&1)as usize;let reg=(value>>20&255)as usize;let mask=if value&(1<<31)!=0{self.get(0x10374)&0xfffff}else{0xfffff};self.rf[path][reg]=(self.rf[path][reg]&!mask)|(value&mask);}
        if at==0x18000{let id=(value>>8)as u8;let path=match value&255{0x19=>0,0x29=>1,_=>panic!("bad command {value:x}")};self.commands.push((path,id));}
        if at==0x270{let offset=(value&255)as usize;if (value>>24)&7==1{self.memory[at+1]=self.xtal[offset];}else{let mask=(value>>16)as u8;self.xtal[offset]=(self.xtal[offset]&!mask)|((value>>8)as u8&mask);}self.memory[at+3]&=!0x80;}
        if self.late_write==Some(self.writes.len()){self.now+=TOTAL_US;}
        if self.fail_write==Some(self.writes.len())||self.fail_cleanup==Some(at){return Err(Fault::new(FaultKind::Access,at,value));}Ok(())
    }
}
impl RegisterIo for Device{
    fn read8(&mut self,a:usize)->io::Result<u8>{self.read(a,1).map(|v|v as u8)}fn read16(&mut self,a:usize)->io::Result<u16>{self.read(a,2).map(|v|v as u16)}fn read32(&mut self,a:usize)->io::Result<u32>{self.read(a,4)}
    fn write8(&mut self,a:usize,v:u8)->io::Result<()>{self.write(a,1,u32::from(v))}fn write16(&mut self,a:usize,v:u16)->io::Result<()>{self.write(a,2,u32::from(v))}fn write32(&mut self,a:usize,v:u32)->io::Result<()>{self.write(a,4,v)}
    fn now_us(&mut self)->u64{self.now}fn delay_us(&mut self,us:u32)->io::Result<()>{self.tick(u64::from(us));if self.late_delay==Some(us){self.now+=TOTAL_US;self.late_delay=None;}Ok(())}
}
impl PhyIo for Device{
    fn pci_id(&mut self)->io::Result<u32>{Ok(0xb85210ec)}fn pci_command(&mut self)->io::Result<u16>{Ok(6)}fn bar_len(&self)->usize{self.memory.len()}
    fn send_rf_page(&mut self,_:u8,_:u8,_:&[u8],_:u32)->io::Result<()>{panic!("DPK has no RF H2C upload")}
}
impl DackIo for Device{fn btc_rfk(&mut self,notice:BtcNotice,timeout:u32)->io::Result<()>{assert!((1..=100_000).contains(&timeout));self.btc.push((notice,self.writes.len()));
    if (notice.state==RfkState::Start&&self.start_fail)||(notice.state==RfkState::Stop&&self.stop_fail){return Err(Fault::new(FaultKind::PowerState,0xac,0));}Ok(())}}
impl RxdckIo for Device{fn scheduler_zero(&mut self,timeout:u32)->io::Result<()>{assert!((1..=1_000_000).contains(&timeout));self.schedulers.push(self.writes.len());self.now+=self.scheduler_delay;
    if self.scheduler_fail==Some(self.schedulers.len()){return Err(Fault::new(FaultKind::Access,0xc348,0));}Ok(())}}
fn run(bus:&mut Device,admission:Admission)->Result<ResultData,Failure>{let start=bus.now;engine(bus,admission,start)}
fn admission()->Admission{Admission{channel:6,tssi:[true;2]}}
fn paired(d:&Device){assert_eq!(d.btc.len(),2);for((notice,_),state)in d.btc.iter().zip([RfkState::Start,RfkState::Stop]){assert_eq!(*notice,BtcNotice{calibration:Calibration::Dpk,state,phy_map:0x10});}assert_eq!(d.btc[1].1,d.writes.len());}
fn radio_off(d:&Device){assert_eq!(d.get(2)&3,0);assert_eq!(d.get(0x2f0)&(1<<17),0);assert_eq!(d.xtal[0x80]&7,0);assert_eq!(d.xtal[0x81]&7,0);}
fn off(d:&Device){radio_off(d);for at in [0x181bc,0x182bc]{assert_eq!(d.get(at)&(1<<24),0);}}
fn restored(d:&Device,original:&Device){for at in BB{assert_eq!(d.get(PHY_BASE+at as usize),original.get(PHY_BASE+at as usize));}for path in 0..2{for at in KIP{let at=PHY_BASE+at as usize+(path<<8);assert_eq!(d.get(at),original.get(at));}for at in RF{if at<0x10000{assert_eq!(d.rf[path][at as usize],original.rf[path][at as usize]);}else{let address=0x1e000+(path<<12)+((at as usize&255)<<2);assert_eq!(d.get(address),original.get(address));}}assert_eq!(d.get(0x15818+(path<<13))&(1<<30),original.get(0x15818+(path<<13))&(1<<30));}}

#[test]
fn initial_dpk_follows_both_paths_with_saved_rf_kip_and_tssi_restoration(){
    let mut d=Device::new();let original=Device::new();let data=run(&mut d,admission()).unwrap();assert_eq!(data.channel,6);assert_eq!(data.gain_scale,0x5b);
    for p in 0..2{assert_eq!(data.paths[p].txagc,0x35);assert_eq!(data.paths[p].thermal,42);assert_eq!(data.paths[p].dc,[1,2]);assert_eq!(data.paths[p].correlation,200);
        assert_eq!(d.commands.iter().filter(|(path,_)|*path==p as u8).map(|(_,id)|*id).collect::<Vec<_>>(),[0x19,0x16,0x15,0x10,0x13,0x19,0x11]);
        assert_eq!(d.get(0x181bc+(p<<8)),0x075b5b5b);assert_eq!(d.get(0x181c4+(p<<8))&0x3f00,0x3500);assert_eq!(d.get(0x181b4+(p<<8))&0x1ff0000,0x780000);
    }restored(&d,&original);paired(&d);assert_eq!(d.schedulers.len(),2);assert_eq!(d.schedulers[0],0);assert_eq!(d.schedulers[1],d.writes.len());assert!(d.now<100_000);
}
#[test]
fn source_backoff_selection_requires_actual_early_dpk_initialization(){
    let mut d=Device::new();d.set(0x144a0,44);assert_eq!(run(&mut d,admission()).unwrap_err().stage,Stage::Guard);assert!(d.btc.is_empty()&&d.writes.is_empty());
    let mut d=Device::new();d.set(0x144a0,44);d.set(0x181bc,0x7f7f7f);d.set(0x182bc,0x7f7f7f);
    assert_eq!(run(&mut d,admission()).unwrap().gain_scale,0x7f);assert_eq!(d.get(0x181bc),0x077f7f7f);assert_eq!(d.get(0x182bc),0x077f7f7f);
}
#[test]
fn command_timeout_in_either_poll_never_becomes_success(){for second in [false,true]{for command in [1,7,8,14]{
    let mut d=Device::new();if second{d.missing_second=Some(command)}else{d.missing_first=Some(command)};
    let e=run(&mut d,admission()).unwrap_err();assert_eq!(e.stage,if command<=7{Stage::PathA}else{Stage::PathB});assert_eq!(e.cause.kind,FaultKind::Timeout);
    assert_eq!(d.commands.len(),command);assert!(d.now<30_000);assert!(e.nctl_uncertain&&e.restore_skipped);assert_eq!(e.rf_restore,[None;2]);paired(&d);radio_off(&d);
    let trigger=d.writes.iter().rposition(|(at,_)|*at==0x18000).unwrap();
    assert!(d.writes[trigger+1..].iter().all(|(at,_)|matches!(*at,0x180d4|0x2f0|2|0x270)));
}}}
#[test]
fn sync_quality_rejects_bad_correlation_or_signed_dc(){for(corr,dc)in[(169,[0,0]),(200,[201,0]),(200,[0,0xf37])]{
    let mut d=Device::new();d.correlation=corr;d.dc=dc;let e=run(&mut d,admission()).unwrap_err();assert_eq!(e.stage,Stage::PathA);assert_eq!(e.cause.kind,FaultKind::PowerState);
    assert_eq!(d.commands.last(),Some(&(0,0x10)));off(&d);paired(&d);
}}
#[test]
fn agc_has_source_thresholds_clamps_and_no_iteration_limit_success(){
    assert_eq!([0xffff,0xaa5,0xaa4,0x556,0x555,0x220,0x21f].map(dgain_offset),[6,6,5,0,-1,-7,-8]);assert_eq!(signed12(0x800),2048);
    let mut d=Device::new();d.loss=7;let data=run(&mut d,admission()).unwrap();assert_eq!(data.paths.map(|p|p.txagc),[0x2e;2]);
    let mut d=Device::new();d.loss=0;let data=run(&mut d,admission()).unwrap();assert_eq!(data.paths.map(|p|p.txagc),[0x3f;2]);
    let mut d=Device::new();d.loss=255;let e=run(&mut d,admission()).unwrap_err();assert_eq!(e.stage,Stage::PathA);assert_eq!(e.cause.value,6);off(&d);
}
#[test]
fn pas_uses_signed_twelve_bit_energy_without_division_by_zero(){
    let mut d=Device::new();d.pas=[(0x800,0x800),(0,0)];assert!(pas_high(&mut d).unwrap());d.pas=[(0,0),(0x800,0x800)];assert!(!pas_high(&mut d).unwrap());
}
#[test]
fn btc_denial_and_uncertain_scheduler_do_not_start_dpk(){for scheduler_error in [false,true]{
    let mut d=Device::new();if scheduler_error{d.scheduler_fail=Some(1)}else{d.start_fail=true};let e=run(&mut d,admission()).unwrap_err();
    assert_eq!(e.stage,if scheduler_error{Stage::SchedulerPause}else{Stage::BtcStart});assert_eq!(d.schedulers.len(),usize::from(scheduler_error));assert!(d.commands.is_empty());off(&d);paired(&d);
}}
#[test]
fn stuck_rx_mode_and_expired_scheduler_never_begin_calibration(){for timedout in [false,true]{
    let mut d=Device::new();if timedout{d.scheduler_delay=1_000_000;}else{d.rf[1][0]=0x20000;}
    let e=run(&mut d,admission()).unwrap_err();assert_eq!(e.stage,if timedout{Stage::SchedulerPause}else{Stage::WaitB});assert!(d.commands.is_empty());off(&d);paired(&d);
}}
#[test]
fn every_possibly_applied_write_failure_restores_known_state_and_never_completes(){
    let count={let mut d=Device::new();run(&mut d,admission()).unwrap();d.writes.len()};
    for index in 1..=count{let mut d=Device::new();d.fail_write=Some(index);let e=run(&mut d,admission()).unwrap_err();assert_eq!(e.cause.kind,FaultKind::Access,"write {index}");assert_eq!(e.disable,[None;2]);if e.nctl_uncertain{radio_off(&d)}else{off(&d)}paired(&d);}
}
#[test]
fn failure_cleanup_errors_are_independent_and_stop_is_last(){
    let mut d=Device::new();d.correlation=100;d.fail_cleanup=Some(0x181bc);d.scheduler_fail=Some(2);d.stop_fail=true;
    let e=run(&mut d,admission()).unwrap_err();assert_eq!(e.cause.kind,FaultKind::PowerState);assert!(e.disable[0].is_some());assert!(e.scheduler_restore.is_some());assert!(e.btc_stop.is_some());assert_eq!(e.rf_restore,[None;2]);assert!(e.radio_stop.is_none());paired(&d);off(&d);
}
#[test]
fn late_delay_or_restore_cannot_publish_completed_calibration(){for restore_late in [false,true]{
    let mut d=Device::new();if restore_late{let mut good=Device::new();run(&mut good,admission()).unwrap();d.late_write=Some(good.writes.len());}else{d.late_delay=Some(1000);}
    let e=run(&mut d,admission()).unwrap_err();assert_eq!(e.cause.kind,FaultKind::Timeout);off(&d);paired(&d);
}}
#[test]
fn failed_final_scheduler_or_btc_stop_disables_both_dpd_paths(){for stop in [false,true]{
    let mut d=Device::new();if stop{d.stop_fail=true}else{d.scheduler_fail=Some(2)};let e=run(&mut d,admission()).unwrap_err();assert_eq!(e.stage,if stop{Stage::BtcStop}else{Stage::SchedulerRestore});assert_eq!(d.schedulers.len(),2);paired(&d);off(&d);
}}
#[test]
fn invalid_initial_input_is_rejected_before_register_access(){for input in [Admission{channel:14,tssi:[true;2]},Admission{channel:6,tssi:[true,false]}]{let mut d=Device::new();assert_eq!(run(&mut d,input).unwrap_err().stage,Stage::Guard);assert!(d.writes.is_empty()&&d.btc.is_empty());assert_eq!(d.now,0);}}
#[test]
fn frozen_clock_still_bounds_missing_completion_poll(){let mut d=Device::new();d.frozen=true;d.missing_first=Some(1);let e=run(&mut d,admission()).unwrap_err();assert_eq!(e.cause.kind,FaultKind::Timeout);assert!(d.writes.len()<300);off(&d);paired(&d);}
