use super::*;
use crate::{c2h::Ack, h2c::{Engine, Outcome}};

fn board() -> Board {
    Board { identity: crate::board::Identity { cut:1,mac:[2;6],rfe:1,xtal:32,country:*b"US",channel_plan:127 },
        maps:crate::efuse::Maps { logical:[0xff;crate::efuse::LOGICAL_SIZE],dav_logical:[0xff;crate::efuse::DAV_LOGICAL_SIZE],phycap:[0xff;crate::efuse::PHYCAP_SIZE] } }
}
fn profile() -> Profile { Profile::select_version(&board(), [0,29,29,15]).unwrap() }

#[test]
fn unsupported_version_cut_rfe_or_security_never_selects_profile() {
    for case in 0..5 { let mut b=board(); let mut v=[0,29,29,15];
        match case { 0=>v[3]=5,1=>b.identity.cut=0,2=>b.identity.rfe=2,3=>b.maps.phycap[0x6c]=0,_=>b.maps.phycap[0x6d]=0 }
        assert!(matches!(Profile::select_version(&b,v),Err(Error::Profile)));
    }
}

#[test]
fn packed_monitor_and_default_slots_use_pinned_versions_and_little_endian_fields() {
    let p=profile(); let monitor=p.monitor_table(); assert_eq!(monitor.bytes().len(),130);
    assert_eq!(&monitor.bytes()[..18], &[2,16,0,0,4,0,0x24,0xda,0,0,0,0,4,0,0x28,0xda,0,0]);
    assert_eq!(&monitor.bytes()[98..106], &[1,0,4,0,0x80,9,0,0]);
    assert_eq!(&monitor.bytes()[122..], &[1,0,4,0,0x94,0x46,0,0]);
    let slots=p.slot_defaults(); assert_eq!(slots.bytes().len(),146);
    assert_eq!(&slots.bytes()[..18], &[1,18,100,0,0x55,0x55,0x55,0x55,0,0,5,0,0x5a,0x5a,0x5a,0xea,1,0]);
    assert_eq!(&slots.bytes()[138..], &[50,0,0xff,0xdf,0xff,0xff,1,0]);
}

#[test]
fn cxdrvinfo_init_control_role_are_zero_initialized_exact_layouts() {
    let p=profile(); assert_eq!(p.driver_init().bytes(), &[0,12,0,2,10,0,1,1,2,0,6,2,0,0]);
    assert_eq!(p.driver_control(true).bytes(), &[6,4,2,0,0,0]);
    assert_eq!(p.driver_control(false).bytes(), &[6,4,0,0,0,0]);
    let role=p.driver_no_link(); assert_eq!(role.bytes().len(),66); assert_eq!(&role.bytes()[..2], &[1,64]);
    assert!(role.bytes()[2..].iter().all(|b|*b==0));
    for m in [p.driver_init(),p.driver_control(true),role] { assert_eq!(m.command(),h2c::Command{category:2,class:16,function:5}); assert!(!m.acks().done); }
}

#[test]
fn report105_maps_to_wire5_and_no_link_afh_keeps_source_guard_width() {
    let p=profile(); assert_eq!(p.reports(0x0c,true).unwrap().bytes(), &[5,12,0,0,0,1,0,0,0]);
    assert_eq!(p.reports(4,false).unwrap().bytes(), &[5,4,0,0,0,0,0,0,0]);
    assert!(matches!(p.reports(0x200,true),Err(Error::Reports)));
    assert_eq!(p.afh_no_link().bytes(), &[0,0,32]);
}

#[test]
fn initial_no_link_changes_only_off_slot_and_rfk_forces_all_slots() {
    let p=profile(); let init=p.policy(Policy::ResetDefaults,Policy::NoLink).unwrap();
    assert_eq!(init.bytes(), &[1,10,1,0,100,0,0x55,0x55,0x55,0xe5,0,0]);
    assert!(p.policy(Policy::NoLink,Policy::NoLink).is_none());
    let rfk=p.policy(Policy::NoLink,Policy::Rfk).unwrap(); assert_eq!(rfk.bytes().len(),230);
    assert_eq!(&rfk.bytes()[..26], &[0,12,3,0,0,0,0,0,0,0,0,0,0,1,1,10,1,0,100,0,0xaa,0xaa,0xaa,0xaa,1,0]);
    for (sid,tlv) in rfk.bytes()[14..].chunks_exact(12).enumerate() { assert_eq!(&tlv[..4], &[1,10,1,sid as u8]); }
    assert_eq!(p.policy(Policy::Rfk,Policy::Rfk).unwrap().bytes(),rfk.bytes());
}

#[test]
fn stop_policy_clears_instant_execute_and_restores_no_link_slot() {
    let m=profile().policy(Policy::Rfk,Policy::NoLink).unwrap(); assert_eq!(m.bytes().len(),26);
    assert_eq!(&m.bytes()[..14], &[0,12,3,0,0,0,0,0,0,0,0,0,0,0]);
    assert_eq!(&m.bytes()[14..], &[1,10,1,0,100,0,0x55,0x55,0x55,0xe5,0,0]);
}

#[derive(Default)]
struct Wire { now:u64, packets:Vec<Vec<u8>>, full:bool }
impl h2c::Transport for Wire {
    fn now_us(&mut self)->u64 { self.now }
    fn submit(&mut self,p:&[u8],_:u32)->Result<(),crate::fwcmd::Error> { if self.full{return Err(crate::fwcmd::Error::Full)} self.packets.push(p.to_vec()); Ok(()) }
}
#[test]
fn encoder_uses_production_h2c_ack_distinction_and_never_consumes_full_sequence() {
    let p=profile(); let mut e=Engine::new(0); let mut w=Wire{full:true,..Wire::default()};
    assert_eq!(p.monitor_table().submit(&mut e,&mut w,100),Err(h2c::Error::Full)); assert_eq!(e.submitted(),0);
    w.full=false; let t=p.monitor_table().submit(&mut e,&mut w,100).unwrap();
    assert_eq!(&w.packets[0][..8], &[0x42,2,0,0,138,0xc0,0,0]);
    assert_eq!(e.take(1,t).unwrap(),None); e.acknowledge(2,Ack::Done{command:t.identity(),status:0}).unwrap();
    assert_eq!(e.take(2,t).unwrap(),None); e.acknowledge(3,Ack::Received(t.identity())).unwrap();
    assert_eq!(e.take(3,t).unwrap(),Some(Outcome::Done));
    w.now=3; let t=p.driver_init().submit(&mut e,&mut w,100).unwrap();
    assert_eq!(&w.packets[1][..8], &[0x42,5,0,1,22,0,0,0]); assert_eq!(e.take(3,t).unwrap(),Some(Outcome::Submitted));
}
