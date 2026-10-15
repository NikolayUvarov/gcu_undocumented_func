use super::*;

#[derive(Debug,PartialEq)]
struct Write(usize,usize,u32);
struct Device {
    bytes:Vec<u8>,bt:u32,wl:Option<u32>,grants:u32,pending:Option<u32>,writes:Vec<Write>,now:u64,
    frozen:bool,stuck:bool,stall_after_command:bool,unavailable:Option<usize>,fail_write:Option<usize>,late_write:Option<usize>,backward_write:Option<usize>,
}
impl Device {
    fn new()->Self { let mut d=Self {bytes:vec![0;0x20000],bt:0,wl:None,grants:0x12345678,pending:None,writes:Vec::new(),now:1000,
        frozen:false,stuck:false,stall_after_command:false,unavailable:None,fail_write:None,late_write:None,backward_write:None};
        d.put(0x73,1,0xa1); d.put(0xc67c,2,0x144); d }
    fn put(&mut self,at:usize,width:usize,v:u32){self.bytes[at..at+width].copy_from_slice(&v.to_le_bytes()[..width]);}
    fn get(&self,at:usize,width:usize)->u32{let mut b=[0;4];b[..width].copy_from_slice(&self.bytes[at..at+width]);u32::from_le_bytes(b)}
    fn tick(&mut self,us:u64){if !self.frozen{self.now+=us;}}
    fn read(&mut self,at:usize,width:usize)->io::Result<u32>{
        io::check_offset(at,width,self.bytes.len())?;self.tick(1);
        if self.unavailable==Some(at){return Ok(u32::MAX)}
        if at==SCOREBOARD{return Ok(self.bt)}
        if at==LTE_CTRL+3{
            if self.stuck{return Ok(0)}
            if let Some(command)=self.pending.take(){if command==0xc00f0038{self.grants=self.get(LTE_WDATA,4)}}
            return Ok(0x20)
        }
        if at==LTE_RDATA{assert_eq!(self.pending,Some(0x800f0038));return Ok(self.grants)}
        Ok(self.get(at,width))
    }
    fn write(&mut self,at:usize,width:usize,v:u32)->io::Result<()>{
        io::check_offset(at,width,self.bytes.len())?;self.tick(1);self.writes.push(Write(at,width,v));
        if at==SCOREBOARD{self.wl=Some(v)}else{self.put(at,width,v)}
        if at==LTE_CTRL{assert!(self.pending.is_none(),"must not replace pending indirect command");assert!(matches!(v,0x800f0038|0xc00f0038));self.pending=Some(v);if self.stall_after_command{self.stuck=true}}
        if self.late_write==Some(self.writes.len()){self.now+=200_000;}
        if self.backward_write==Some(self.writes.len()){self.now=0;}
        if self.fail_write==Some(self.writes.len()){return Err(Fault::new(FaultKind::Access,at,v))}
        Ok(())
    }
}
impl RegisterIo for Device {
    fn read8(&mut self,a:usize)->io::Result<u8>{self.read(a,1).map(|v|v as u8)}
    fn read16(&mut self,a:usize)->io::Result<u16>{self.read(a,2).map(|v|v as u16)}
    fn read32(&mut self,a:usize)->io::Result<u32>{self.read(a,4)}
    fn write8(&mut self,a:usize,v:u8)->io::Result<()>{self.write(a,1,v as u32)}
    fn write16(&mut self,a:usize,v:u16)->io::Result<()>{self.write(a,2,v as u32)}
    fn write32(&mut self,a:usize,v:u32)->io::Result<()>{self.write(a,4,v)}
    fn now_us(&mut self)->u64{self.now}
    fn delay_us(&mut self,us:u32)->io::Result<()>{self.tick(us as u64);Ok(())}
}

#[test]
fn scoreboard_write_uses_wlan_shadow_not_bluetooth_lower_bits() {
    let mut d=Device::new();d.bt=0x7effffff;let mut s=WlanShadow::new();s.initialize();s.publish(&mut d,10_000).unwrap();
    assert_eq!(d.wl,Some(0xff004003));assert_eq!(d.bt,0x7effffff);assert_eq!(s.value(),0x4003);
    s.set_rfk(true);s.publish(&mut d,10_000).unwrap();assert_eq!(d.wl,Some(0xff004803));
    s.set_rfk(false);s.publish(&mut d,10_000).unwrap();assert_eq!(d.wl,Some(0xff004003));assert!(d.now>=4000);
}
#[test]
fn ambiguous_scoreboard_write_is_poisoned_and_never_retried() {
    for late in [false,true]{let mut d=Device::new();if late{d.late_write=Some(1)}else{d.fail_write=Some(1)}
        let mut s=WlanShadow::new();s.initialize();assert!(s.publish(&mut d,10_000).is_err());
        d.fail_write=None;d.late_write=None;assert!(s.publish(&mut d,10_000).is_err());assert_eq!(d.writes.len(),1);}
}
#[test]
fn initial_grants_precede_control_and_depend_on_actual_bt_on() {
    for bt in [0,2]{let mut d=Device::new();d.bt=bt;let state=antenna(&mut d,Antenna::Initial,100_000).unwrap();
        assert_eq!(state.enabled,bt==2);assert_eq!(d.grants,if bt==2{0xdd00dd00}else{0x77007700});
        assert_eq!(d.get(0x73,1),0xa5);assert_eq!(d.get(0xc67c,2),0x166);
        assert!(d.writes.iter().position(|w|w.0==LTE_CTRL).unwrap()<d.writes.iter().position(|w|w.0==0x73).unwrap());assert!(d.pending.is_none());}
}
#[test]
fn rfk_and_stop_apply_source_control_grant_and_pollution_order() {
    let mut d=Device::new();d.bt=2;antenna(&mut d,Antenna::Rfk,100_000).unwrap();
    assert_eq!(d.writes[0],Write(0x73,1,0xa5));assert_eq!(d.grants,0x77007700);assert_eq!(d.get(0xc67c,2),0x100);
    d.writes.clear();antenna(&mut d,Antenna::NoLink,100_000).unwrap();
    assert_eq!(d.writes[0],Write(0x73,1,0xa5));assert_eq!(d.grants,0);assert_eq!(d.get(0xc67c,2),0x166);
}
#[test]
fn live_bt_rfk_or_request_refuses_before_any_register_write() {
    for bt in [1<<5,1<<6,u32::MAX]{let mut d=Device::new();d.bt=bt;
        assert!(antenna(&mut d,Antenna::Rfk,100_000).is_err());assert!(d.writes.is_empty());}
}
#[test]
fn actual_snapshot_restores_prior_values_without_touching_control_neighbors() {
    let mut d=Device::new();let saved=SavedAntenna::read(&mut d,200_000).unwrap();
    antenna(&mut d,Antenna::Rfk,100_000).unwrap();d.put(0x73,1,0x5e);
    saved.restore(&mut d,200_000).unwrap();assert_eq!(d.grants,0x12345678);assert_eq!(d.get(0x73,1),0x5a);assert_eq!(d.get(0xc67c,2),0x144);
}
#[test]
fn pending_lte_command_is_not_replaced_even_by_restore() {
    let mut d=Device::new();let saved=SavedAntenna::read(&mut d,200_000).unwrap();d.writes.clear();d.stall_after_command=true;
    assert_eq!(antenna(&mut d,Antenna::Initial,100_000).unwrap_err().kind,FaultKind::Timeout);
    assert_eq!(d.pending,Some(0xc00f0038));let writes=d.writes.len();
    assert_eq!(saved.restore(&mut d,200_000).unwrap_err().kind,FaultKind::Timeout);assert_eq!(d.writes.len(),writes);
}
#[test]
fn frozen_lte_clock_and_unavailable_device_are_bounded_without_writes() {
    let mut d=Device::new();d.frozen=true;d.stuck=true;assert_eq!(antenna(&mut d,Antenna::Initial,100_000).unwrap_err().kind,FaultKind::Timeout);assert!(d.writes.is_empty());
    for at in [SCOREBOARD,LTE_CTRL+3]{let mut d=Device::new();d.unavailable=Some(at);
        assert_eq!(antenna(&mut d,Antenna::Initial,100_000).unwrap_err().kind,FaultKind::Unavailable);assert!(d.writes.is_empty());}
}
#[test]
fn every_applied_write_failure_stops_transition_and_late_or_backward_clock_is_rejected() {
    let mut baseline=Device::new();antenna(&mut baseline,Antenna::Rfk,100_000).unwrap();
    for at in 1..=baseline.writes.len(){let mut d=Device::new();d.fail_write=Some(at);
        assert!(antenna(&mut d,Antenna::Rfk,100_000).is_err());assert_eq!(d.writes.len(),at);}
    for backward in [false,true]{let mut d=Device::new();if backward{d.backward_write=Some(1)}else{d.late_write=Some(1)}
        assert_eq!(antenna(&mut d,Antenna::Rfk,100_000).unwrap_err().kind,if backward{FaultKind::Clock}else{FaultKind::Timeout});assert_eq!(d.writes.len(),1);}
}
#[test]
fn common_action_programs_pinned_preagc_and_btg_only_for_patched_enabled_bt() {
    for active in [false,true]{let mut d=Device::new();let bt=BtState{enabled:active,patched:active,running_rfk:false,requests_rfk:false};
        common_initial(&mut d,bt,100_000).unwrap();assert_eq!(d.writes.len(),if active{25}else{14});
        assert_eq!(d.get(0x146d0,4)&3,0);assert_eq!(d.get(0x14688,4)>>24,0x1a);assert_eq!(d.get(0x14694,4)&0xffff,0x2a2a);
        assert_eq!(d.get(0x1469c,4)>>26,0x26);assert_eq!(d.get(0x14738,4)&(1<<19),if active{1<<19}else{0});
        assert!(d.wl.is_none());assert!(d.writes.iter().all(|w|w.0>=BASEBAND));}
}
#[test]
fn default_power_preserves_unrelated_fields_and_rejects_final_overrun() {
    let mut d=Device::new();d.put(0xd200,4,0x1234ffff);d.put(0xd220,4,0x7654ffff);
    default_power(&mut d,100_000).unwrap();assert_eq!(d.get(0xd200,4),0x1234fc00);assert_eq!(d.get(0xd220,4),0x7654f005);
    let mut d=Device::new();d.late_write=Some(2);assert_eq!(default_power(&mut d,100_000).unwrap_err().kind,FaultKind::Timeout);
}
