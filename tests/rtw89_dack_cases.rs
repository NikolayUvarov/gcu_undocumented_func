use super::*;

pub(super) fn prepared(p:&mut Phy<Device>) {
    to_nctl(p); p.rck().unwrap();
    let d=p.bus_between_stages(); d.events.clear(); d.dack_model=true;
    for (at,value) in [(0x1c0d0,(17<<15)|8),(0x1c0fc,(0x123<<10)|0x155),(0x1c1fc,(0x234<<10)|0x2ab),
        (0x1c040,1<<31),(0x1c064,1<<31),(0x1c05c,4),(0x1c080,4),
        (0x1c140,1<<31),(0x1c164,1<<31),(0x1c15c,4),(0x1c180,4),
        (0x1c048,0x1aa<<2),(0x1c06c,0x2bb<<2),(0x1c148,0x1cc<<2),(0x1c16c,0x2dd<<2),
        (0x1c060,0x55000000),(0x1c084,0x56000000),(0x1c160,0x65000000),(0x1c184,0x66000000),
        (0x11730,0x00fff001)] {d.put(at,4,value);}
    d.rf[0][0]=0x34567; d.rf[1][0]=0x35678; d.rf[0][5]=0x65432; d.rf[1][5]=0x76542;
}
fn stopped(d:&Device) {
    assert_eq!(d.get(2,1)&3,0); assert_eq!(d.get(0x2f0,4)&(1<<17),0);
    assert_eq!(d.xtal[0x80]&7,0); assert_eq!(d.xtal[0x81]&7,0); assert_eq!(d.command,6);
    assert_eq!(d.get(0xc348,4),0); assert_eq!(d.get(0x1010,4),0xb0f03);
}
fn pair(d:&Device,band:u8) {
    assert_eq!(d.btc.len(),2);
    for ((notice,_),state) in d.btc.iter().zip([dack::RfkState::Start,dack::RfkState::Stop]) {
        assert_eq!(notice.calibration,dack::Calibration::Dack); assert_eq!(notice.state,state); assert_eq!(notice.phy_map,0x10|band<<6);
    }
}

#[test]
fn dack_requires_completed_rck_before_any_btc_or_register_write() {
    let mut d=Device::new(); let mut p=Phy::new(&mut d,identity());
    assert_eq!(p.dack(dack::Band::Ghz2).unwrap_err().stage,dack::Stage::Guard);
    assert!(d.btc.is_empty()&&d.events.is_empty());
}
#[test]
fn source_dack_sequence_collects_calibration_then_restores_rf_and_stops_btc() {
    let mut d=Device::new(); let mut p=Phy::new(&mut d,identity()); prepared(&mut p);
    let result=p.dack(dack::Band::Ghz5).unwrap(); assert_eq!(p.state(),State::DackDone);
    assert_eq!(result.drck,17); assert_eq!(result.addck,[[0x123,0x155],[0x234,0x2ab]]);
    assert_eq!(result.bias,[[0x1aa,0x2bb],[0x1cc,0x2dd]]); assert_eq!(result.dadck,[[0x55,0x56],[0x65,0x66]]);
    assert_eq!(result.completion,[[3,3],[3,3]]); assert_eq!(result.diagnostic_dc,[[-1,1];5]);
    for (path,bases) in [[0x10,0x30],[0x50,0x70]].into_iter().enumerate() {
        for (iq,base) in bases.into_iter().enumerate() {assert_eq!(result.msbk[path][iq],core::array::from_fn(|i|base+i as u8));}
    }
    let d=p.bus_between_stages(); pair(d,1); assert_eq!(d.events[4],Event::Write(0x8040,4,15));
    assert_eq!(d.rf[0][0],0x34567);assert_eq!(d.rf[1][0],0x35678);
    assert_eq!(d.rf[0][5],0x65433);assert_eq!(d.rf[1][5],0x76543);
    assert_eq!(d.get(0x1c0f0,4)&0xffff0000,(0x123<<16)|(0x15<<26));
    assert_eq!(d.get(0x1c1f0,4)&0xffff0000,(0x234<<16)|(0x2b<<26));
    assert_eq!(d.get(0xc348,4),0);assert_eq!(d.command,6);assert_eq!(d.get(0x1010,4),0xb0f03);
}
#[test]
fn btc_start_refusal_and_late_grant_cannot_begin_afe_and_always_attempt_stop() {
    for late in [false,true] {
        let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);
        let d=p.bus_between_stages(); if late {d.btc_start_delay=100_000;} else {d.btc_start_fail=true;}
        let error=p.dack(dack::Band::Ghz2).unwrap_err();assert_eq!(error.stage,dack::Stage::BtcStart);
        assert_eq!(error.cause.kind,if late {FaultKind::Timeout} else {FaultKind::Access});
        assert!(error.btc_stop.is_none());assert!(error.radio_stop.is_none());let d=p.bus_between_stages();pair(d,0);
        assert!(!d.events.iter().any(|e|matches!(e,Event::Write(0x8040,_,_))));stopped(d);
    }
}
#[test]
fn each_calibration_timeout_aborts_and_restores_both_rf_paths() {
    for (at,stage) in [(0x1c0d0,dack::Stage::Drck),(0x1c0fc,dack::Stage::AddckA),(0x1c1fc,dack::Stage::AddckB),
        (0x1c040,dack::Stage::DackA),(0x1c05c,dack::Stage::DackA),(0x1c140,dack::Stage::DackB),(0x1c15c,dack::Stage::DackB)] {
        let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);let d=p.bus_between_stages();d.put(at,4,0);
        if at==0x1c140 {d.put(0x1c164,4,0);} if at==0x1c15c {d.put(0x1c180,4,0);}
        let error=p.dack(dack::Band::Ghz2).unwrap_err();assert_eq!(error.stage,stage);assert_eq!(error.cause.kind,FaultKind::Timeout);
        assert_eq!(error.rf_restore,[None;2]);assert!(error.btc_stop.is_none());assert!(error.radio_stop.is_none());
        let d=p.bus_between_stages();pair(d,0);assert_eq!(d.rf[0][0],0x34567);assert_eq!(d.rf[1][0],0x35678);
        assert_eq!(d.rf[0][5],0x65432);assert_eq!(d.rf[1][5],0x76542);stopped(d);
    }
}
#[test]
fn source_s1_completion_asymmetry_remains_visible_in_returned_flags() {
    let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);
    let d=p.bus_between_stages();d.put(0x1c164,4,0);d.put(0x1c180,4,0);
    assert_eq!(p.dack(dack::Band::Ghz2).unwrap().completion,[[3,3],[1,1]]);
}
#[test]
fn btc_stop_failure_cannot_publish_calibration_or_retry_stop() {
    for late in [false,true] {
        let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);
        let d=p.bus_between_stages();if late {d.btc_stop_delay=100_000;}else{d.btc_stop_fail=true;}
        let error=p.dack(dack::Band::Ghz5).unwrap_err();assert_eq!(error.stage,dack::Stage::BtcStop);
        assert_eq!(error.btc_stop.unwrap().kind,if late {FaultKind::Timeout}else{FaultKind::Access});
        assert_eq!(p.state(),State::Failed);assert_eq!(error.rf_restore,[None;2]);let d=p.bus_between_stages();pair(d,1);stopped(d);
    }
}
#[test]
fn primary_dack_fault_and_all_cleanup_failures_remain_distinct() {
    let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);
    let d=p.bus_between_stages();d.put(0x1c0d0,4,0);d.btc_stop_fail=true;d.restore_fail=true;d.fail_cleanup=Some(0x2f0);
    let error=p.dack(dack::Band::Ghz2).unwrap_err();assert_eq!(error.cause.kind,FaultKind::Timeout);
    assert_eq!(error.btc_stop.unwrap().kind,FaultKind::Access);
    assert!(error.rf_restore.iter().all(|e|e.is_some_and(|e|e.kind==FaultKind::Access)));
    assert_eq!(error.radio_stop.unwrap().register,0x2f0);assert_eq!(p.state(),State::Failed);pair(p.bus_between_stages(),0);
}
#[test]
fn failed_calibration_keeps_btc_arbitration_until_rf_cleanup_finishes() {
    for denied_start in [false,true] {
        let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);
        let d=p.bus_between_stages();d.btc_start_fail=denied_start;d.put(0x1c0d0,4,0);
        p.dack(dack::Band::Ghz2).unwrap_err();let d=p.bus_between_stages();pair(d,0);stopped(d);
        assert_eq!(d.btc_event_positions[0],0);
        assert_eq!(d.btc_event_positions[1],d.events.len(),"BTC STOP must follow every cleanup write");
        assert!(d.btc_event_positions[1]>0);
        if !denied_start {assert_eq!(d.rf[0][0],0x34567);assert_eq!(d.rf[1][0],0x35678);}
    }
}
#[test]
fn every_dack_write_failure_is_poisoned_with_btc_stop_and_radio_cleanup() {
    let count={let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);p.dack(dack::Band::Ghz2).unwrap();p.bus_between_stages().events.len()};
    for index in 1..=count {
        let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);
        let d=p.bus_between_stages();d.fail_write=Some(d.writes+index);
        let error=p.dack(dack::Band::Ghz2).unwrap_err();assert_eq!(error.cause.kind,FaultKind::Access,"index{index}");
        assert_eq!(p.state(),State::Failed);pair(p.bus_between_stages(),0);stopped(p.bus_between_stages());
        let before=p.bus_between_stages().events.len();assert!(p.dack(dack::Band::Ghz2).is_err());assert_eq!(p.bus_between_stages().events.len(),before);
    }
}

#[test]
fn final_dack_path_cannot_publish_after_whole_calibration_deadline() {
    let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);
    let d=p.bus_between_stages();d.deadline_write=Some(0x1c100);d.deadline_us=400_000;
    let error=p.dack(dack::Band::Ghz2).unwrap_err();assert_eq!(error.stage,dack::Stage::DackB);
    assert_eq!(error.cause.kind,FaultKind::Timeout);assert!(error.btc_stop.is_none());assert_eq!(error.rf_restore,[None;2]);
    assert!(error.radio_stop.is_none());assert_eq!(p.state(),State::Failed);pair(p.bus_between_stages(),0);stopped(p.bus_between_stages());
}
