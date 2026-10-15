use super::*;
use crate::phy::PhyIo;

#[derive(Clone,Debug)]
enum Event{Write(usize,u32),Rf(u8,u32,u32,u32),Notice(RfkState),Scheduler}
struct Device{
    memory:Vec<u8>,rf:[[u32;256];2],xtal:[u8;256],readback:u32,now:u64,frozen:bool,
    events:Vec<Event>,commands:Vec<u32>,notices:Vec<RfkState>,scheduler:usize,
    stuck:Option<usize>,fail_command:Option<usize>,flag_command:Option<usize>,bad_lok:usize,
    writes:usize,fail_write:Option<usize>,fail_notice:Option<usize>,fail_scheduler:Option<usize>,
    late_notice:Option<usize>,late_read:Option<usize>,late_read_after:usize,reads:usize,fail_cleanup:bool,
}
impl Device{
    fn new()->Self{
        let mut d=Self{memory:vec![0;0x20000],rf:[[0;256];2],xtal:[0;256],readback:0,now:1000,frozen:false,
            events:Vec::new(),commands:Vec::new(),notices:Vec::new(),scheduler:0,stuck:None,fail_command:None,flag_command:None,bad_lok:0,
            writes:0,fail_write:None,fail_notice:None,fail_scheduler:None,late_notice:None,late_read:None,late_read_after:0,reads:0,fail_cleanup:false};
        d.put(2,1,3);d.put(0x2f0,4,1<<17);d.put(0x8040,1,15);d.xtal[0x80]=0xc7;d.xtal[0x81]=0xc7;
        for path in 0..2{for(i,at)in RF_BACKUP.iter().enumerate(){if at&0x10000==0{d.rf[path][*at as usize]=0x30000+(i as u32)*4+1;}
            else{d.put(0x1e000+path*0x1000+((*at&255)as usize)*4,4,0x12341);}}
            d.rf[path][0x18]=0x1006;d.rf[path][0x58]=(12<<15)|(13<<10);d.rf[path][0x0a]=(20<<14)|(21<<4);
        }
        for(at,v)in BB_BACKUP.iter().zip([0x12345678,0x23456789,0x3456789a]){d.put(BASE+*at,4,v);}d
    }
    fn tick(&mut self,us:u64){if !self.frozen{self.now+=us;}}
    fn get(&self,at:usize,width:usize)->u32{let mut a=[0;4];a[..width].copy_from_slice(&self.memory[at..at+width]);u32::from_le_bytes(a)}
    fn put(&mut self,at:usize,width:usize,value:u32){self.memory[at..at+width].copy_from_slice(&value.to_le_bytes()[..width]);}
    fn read(&mut self,at:usize,width:usize)->io::Result<u32>{
        io::check_offset(at,width,self.memory.len())?;self.tick(1);self.reads+=1;
        if self.late_read==Some(at){if self.late_read_after>0{self.late_read_after-=1;}else{self.now+=TOTAL_US;self.late_read=None;}}
        if at==0x1174c{return Ok(self.readback);}
        Ok(self.get(at,width))
    }
    fn write(&mut self,at:usize,width:usize,value:u32)->io::Result<()>{
        io::check_offset(at,width,self.memory.len())?;self.tick(1);self.writes+=1;self.events.push(Event::Write(at,value));self.put(at,width,value);
        if at==0x10378{let path=((value>>8)&7)as usize;let reg=(value&255)as usize;
            self.readback=(1<<26)|self.rf[path][reg];}
        if at==0x10370{let path=((value>>28)&7)as usize;let reg=((value>>20)&255)as usize;
            let mask=if value>>31!=0{self.get(0x10374,4)&RF_MASK}else{RF_MASK};
            self.rf[path][reg]=(self.rf[path][reg]&!mask)|(value&mask);self.events.push(Event::Rf(path as u8,reg as u32,mask,value&mask));}
        if at==0x18000{
            self.commands.push(value);let index=self.commands.len();self.put(0x1bff8,4,if self.stuck==Some(index){0}else{0x55});
            let old=self.get(0x18008,4)&!(1<<26);self.put(0x18008,4,old|if self.flag_command==Some(index){1<<26}else{0});
            if value&0xf00==0x300{
                let path=usize::from(value&0x20!=0);let invalid=self.bad_lok>0;
                if index%4==0{self.bad_lok=self.bad_lok.saturating_sub(1);}
                self.rf[path][0x58]=if invalid{0}else{(12<<15)|(13<<10)};
                self.rf[path][0x0a]=if invalid{0}else{(20<<14)|(21<<4)};
            }
            if self.fail_command==Some(index){return Err(Fault::new(FaultKind::Access,at,value));}
        }
        if at==0x270{let reg=(value&255)as usize;
            if value&(1<<24)!=0{self.put(at,4,reg as u32|u32::from(self.xtal[reg])<<8);}
            else{let mask=(value>>16)as u8;self.xtal[reg]=(self.xtal[reg]&!mask)|((value>>8)as u8&mask);self.put(at,4,value&!(1<<31));}
        }
        if self.fail_write==Some(self.writes)||(self.fail_cleanup&&at==0x2f0){return Err(Fault::new(FaultKind::Access,at,value));}Ok(())
    }
    fn off(&self){assert_eq!(self.get(2,1)&3,0);assert_eq!(self.get(0x2f0,4)&(1<<17),0);assert_eq!(self.xtal[0x80]&7,0);assert_eq!(self.xtal[0x81]&7,0);}
}
impl RegisterIo for Device{
    fn read8(&mut self,at:usize)->io::Result<u8>{self.read(at,1).map(|v|v as u8)}
    fn read16(&mut self,at:usize)->io::Result<u16>{self.read(at,2).map(|v|v as u16)}
    fn read32(&mut self,at:usize)->io::Result<u32>{self.read(at,4)}
    fn write8(&mut self,at:usize,v:u8)->io::Result<()>{self.write(at,1,u32::from(v))}
    fn write16(&mut self,at:usize,v:u16)->io::Result<()>{self.write(at,2,u32::from(v))}
    fn write32(&mut self,at:usize,v:u32)->io::Result<()>{self.write(at,4,v)}
    fn now_us(&mut self)->u64{self.now}
    fn delay_us(&mut self,us:u32)->io::Result<()>{self.tick(u64::from(us));Ok(())}
}
impl PhyIo for Device{
    fn pci_id(&mut self)->io::Result<u32>{Ok(0xb85210ec)}
    fn pci_command(&mut self)->io::Result<u16>{Ok(6)}
    fn bar_len(&self)->usize{self.memory.len()}
    fn send_rf_page(&mut self,_path:u8,_page:u8,_bytes:&[u8],_timeout:u32)->io::Result<()>{panic!("IQK cannot invent H2C RF pages")}
}
impl dack::DackIo for Device{
    fn btc_rfk(&mut self,notice:BtcNotice,timeout:u32)->io::Result<()>{
        assert_eq!(notice.calibration,Calibration::Iqk);assert_eq!(notice,super::notice(notice.state));
        self.notices.push(notice.state);self.events.push(Event::Notice(notice.state));let index=self.notices.len();
        if self.late_notice==Some(index){self.now+=u64::from(timeout);}
        if self.fail_notice==Some(index){return Err(bad(0x30,index as u32));}Ok(())
    }
}
impl RxdckIo for Device{
    fn scheduler_zero(&mut self,_timeout:u32)->io::Result<()>{self.scheduler+=1;self.events.push(Event::Scheduler);
        if self.fail_scheduler==Some(self.scheduler){return Err(Fault::new(FaultKind::Access,0xc348,0));}Ok(())}
}
fn execute(d:&mut Device)->core::result::Result<[PathResult;2],Failure>{let start=d.now;engine(d,6,start)}
#[test]
fn initial_wideband_iqk_runs_both_paths_exact_commands_and_restores_source_backups(){
    let mut d=Device::new();let before=d.rf;let bb=BB_BACKUP.map(|at|d.get(BASE+at,4));let results=execute(&mut d).unwrap();
    assert_eq!(d.commands,[0x119,0x319,0x219,0x319,0x819,0x819,0x819,0x819,0xb19,0xb19,0xb19,0xb19,0xe19,
        0x129,0x329,0x229,0x329,0x829,0x829,0x829,0x829,0xb29,0xb29,0xb29,0xb29,0xe29]);
    assert_eq!(d.notices,[RfkState::Start,RfkState::OneshotStart,RfkState::OneshotStop,RfkState::OneshotStart,RfkState::OneshotStop,RfkState::Stop]);
    assert_eq!(d.scheduler,2);assert!(results.iter().all(|r|!r.failed()&&r.lok_attempts==1&&r.enable&0xf0f==0x505));
    for p in 0..2{for at in RF_BACKUP{if at<256{assert_eq!(d.rf[p][at as usize],before[p][at as usize]);}}
        assert_eq!(d.get(0x1e014+p*0x1000,4),0x12341);assert_eq!(d.get(0x18138+p*256,4),0x40000000);}
    assert_eq!(BB_BACKUP.map(|at|d.get(BASE+at,4)),bb);assert_eq!(d.get(0x19fe4,4),0x06000600);
    assert_eq!(d.get(0x19fe0,4),0x2a000000);assert_eq!(d.get(0x19fe8,4)&0xff00,0x200);
    assert!(!d.events.iter().any(|e|matches!(e,Event::Write(0xc348|0x1000|0x1010,_))));
    let rx:Vec<_>=d.events.iter().filter_map(|e|if let Event::Rf(p,0,0x3ff0,bits)=e{Some((*p,*bits>>4))}else{None}).collect();
    assert_eq!(rx,[(0,0x212),(0,0x21c),(0,0x350),(0,0x360),(1,0x212),(1,0x21c),(1,0x350),(1,0x360)]);
}
#[test]
fn lok_range_retries_are_bounded_and_actual_coordinates_are_retained(){
    let mut d=Device::new();d.bad_lok=2;let r=execute(&mut d).unwrap();assert_eq!(r[0].lok_attempts,3);assert_eq!(d.commands.len(),34);
    assert_eq!(r[0].lok_idac,(12<<15)|(13<<10));assert_eq!(r[0].lok_vbuf,(20<<14)|(21<<4));
    let mut d=Device::new();d.bad_lok=10;let e=execute(&mut d).unwrap_err();assert_eq!(e.stage,Stage::Quality);
    assert!(e.results.iter().all(|r|r.lok_range_fail&&r.lok_attempts==3));assert_eq!(d.commands.len(),42);d.off();
}
#[test]
fn nctl_quality_flags_are_not_silently_accepted_even_for_vbuffer_and_restore(){
    for command in 1..=26{let mut d=Device::new();d.flag_command=Some(command);let e=execute(&mut d).unwrap_err();
        assert_eq!(e.stage,Stage::Quality,"command {command}");assert!(!e.nctl_uncertain);assert_eq!(d.commands.len(),26);d.off();}
}
#[test]
fn timeout_or_uncertain_trigger_never_replaces_nctl_or_restores_live_registers(){
    for failed_write in [false,true]{for command in [1,4,8,12,13,14,26]{let mut d=Device::new();
        if failed_write{d.fail_command=Some(command);}else{d.stuck=Some(command);}
        let e=execute(&mut d).unwrap_err();assert!(e.nctl_uncertain);assert_eq!(d.commands.len(),command);
        let at=d.events.iter().rposition(|e|matches!(e,Event::Write(0x18000,_))).unwrap();
        assert!(!d.events[at+1..].iter().any(|e|matches!(e,Event::Rf(..)|Event::Write(0x12344|0x15800|0x17800,_))));
        let off=d.events.iter().position(|e|matches!(e,Event::Write(0x2f0,_))).unwrap();
        let resume=d.events.iter().rposition(|e|matches!(e,Event::Scheduler)).unwrap();assert!(off<resume);
        assert_eq!(d.notices.last(),Some(&RfkState::Stop));d.off();}}
}
#[test]
fn frozen_clock_and_unavailable_controller_still_have_finite_work(){
    let mut d=Device::new();d.frozen=true;d.stuck=Some(1);let e=execute(&mut d).unwrap_err();
    assert_eq!(e.cause.kind,FaultKind::Timeout);assert!(d.reads<9_000);assert_eq!(d.now,1000);d.off();
    let mut d=Device::new();d.put(0xc348,4,u32::MAX);let e=execute(&mut d).unwrap_err();assert_eq!(e.cause.kind,FaultKind::Unavailable);assert!(d.commands.is_empty());d.off();
}
#[test]
fn denied_or_late_nested_admission_has_matching_cleanup_without_calibration(){
    for late in [false,true]{let mut d=Device::new();if late{d.late_notice=Some(2);}else{d.fail_notice=Some(2);}
        let e=execute(&mut d).unwrap_err();assert_eq!(e.stage,Stage::NestedStart);assert!(d.commands.is_empty());
        assert_eq!(d.notices,[RfkState::Start,RfkState::OneshotStart,RfkState::OneshotStop,RfkState::Stop]);d.off();
        let off=d.events.iter().rposition(|e|matches!(e,Event::Write(0x270,_))).unwrap();
        let stop=d.events.iter().position(|e|matches!(e,Event::Notice(RfkState::OneshotStop))).unwrap();assert!(off<stop);}
}
#[test]
fn uncertain_scheduler_exchange_never_gets_retried(){
    for call in [1,2]{let mut d=Device::new();d.fail_scheduler=Some(call);let e=execute(&mut d).unwrap_err();
        assert_eq!(d.scheduler,call);assert_eq!(e.stage,if call==1{Stage::Pause}else{Stage::Resume});d.off();}
}
#[test]
fn every_applied_write_failure_keeps_primary_fault_and_finite_owned_cleanup(){
    let mut good=Device::new();execute(&mut good).unwrap();
    for index in 1..=good.writes{let mut d=Device::new();d.fail_write=Some(index);let e=execute(&mut d).unwrap_err();
        assert_eq!(e.cause.kind,FaultKind::Access,"write {index}");assert_eq!(d.notices.last(),Some(&RfkState::Stop));d.off();}
}
#[test]
fn final_read_crossing_stage_budget_and_cleanup_failure_are_both_retained(){
    let mut d=Device::new();d.late_read=Some(0x1f014);d.late_read_after=1;d.fail_cleanup=true;
    let e=execute(&mut d).unwrap_err();assert_eq!(e.cause.kind,FaultKind::Timeout);assert!(e.radio_stop.is_some());
    assert_eq!(d.notices.last(),Some(&RfkState::Stop));
}
#[test]
fn failed_stop_is_attempted_once_and_does_not_issue_completed(){
    for call in [3,5,6]{let mut d=Device::new();d.fail_notice=Some(call);let e=execute(&mut d).unwrap_err();
        if call==6{assert_eq!(e.stage,Stage::BtcStop);assert!(e.btc_stop.is_some());assert_eq!(d.notices.len(),6);}
        else{assert_eq!(e.stage,Stage::NestedStop);assert!(e.nested_stop.is_some());assert_eq!(d.notices.len(),call+1);}
        d.off();}
}
#[test]
fn completed_identity_and_calibration_authority_cannot_cross_attempts_or_engines(){
    use phy::channel::{Tuned,Access,CalibrationEngine,CalibrationTransmit};
    let tuned=Tuned{attempt:3,session:5,channel:6,policy_version:7,access:Access::InitiateRadiation};
    let authority=CalibrationTransmit::modeled(&tuned,CalibrationEngine::Iqk).unwrap();
    assert!(authority.matches(&tuned,CalibrationEngine::Iqk));assert!(!authority.matches(&tuned,CalibrationEngine::Tssi));
    let done=Completed{attempt:3,session:5,channel:6,policy_version:7,results:[PathResult::default();2]};
    assert!(done.matches(3,5,6,7));for args in [(4,5,6,7),(3,6,6,7),(3,5,7,7),(3,5,6,8),(0,5,6,7)]{assert!(!done.matches(args.0,args.1,args.2,args.3));}
    let passive=Tuned{access:Access::PassiveOnly,..tuned};assert!(!authority.matches(&passive,CalibrationEngine::Iqk));
    assert!(CalibrationTransmit::modeled(&passive,CalibrationEngine::Iqk).is_none());
}
