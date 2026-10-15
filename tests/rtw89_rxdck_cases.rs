use super::*;

fn prepared(p:&mut Phy<Device>) {
    dack_cases::prepared(p);p.dack(dack::Band::Ghz2).unwrap();let d=p.bus_between_stages();
    d.events.clear();d.btc.clear();d.btc_event_positions.clear();
    d.rf[0][0]=0x14567;d.rf[1][0]=0x15678;d.rf[0][0x92]=0x12342;d.rf[1][0x92]=0x23440;
    d.rf[0][0x93]=0xfedcf;d.rf[1][0x93]=0xedcbf;
}
fn pair(d:&Device,band:u8) {
    assert_eq!(d.btc.len(),2);
    for ((notice,_),state) in d.btc.iter().zip([dack::RfkState::Start,dack::RfkState::Stop]) {
        assert_eq!(notice.calibration,dack::Calibration::Rxdck);assert_eq!(notice.state,state);assert_eq!(notice.phy_map,0x10|band<<6);
    }
}
fn off(d:&Device) {
    assert_eq!(d.get(2,1)&3,0);assert_eq!(d.get(0x2f0,4)&(1<<17),0);assert_eq!(d.xtal[0x80]&7,0);assert_eq!(d.xtal[0x81]&7,0);
    assert_eq!(d.get(0xc348,4),0);assert_eq!(d.command,6);assert_eq!(d.get(0x1010,4),0xb0f03);
}
#[test]
fn rxdck_rechecks_post_dack_phase_and_stopped_scheduler_before_callbacks() {
    let mut d=Device::new();let mut p=Phy::new(&mut d,identity());
    assert_eq!(p.rxdck(dack::Band::Ghz2).unwrap_err().stage,rxdck::Stage::Guard);assert!(d.events.is_empty());
    for (at,value) in [(0x8040,14),(0xc348,1),(0xc6ec,1<<7),(0x1000,0x2800)] {
        let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);p.bus_between_stages().put(at,4,value);
        assert_eq!(p.rxdck(dack::Band::Ghz2).unwrap_err().stage,rxdck::Stage::Guard);
        let d=p.bus_between_stages();assert!(d.btc.is_empty()&&d.scheduler_calls.is_empty()&&d.events.is_empty());
    }
}
#[test]
fn initial_rxdck_matches_both_source_paths_and_never_enables_tx_or_tssi() {
    let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);let start=p.bus_between_stages().now;
    assert_eq!(p.rxdck(dack::Band::Ghz5).unwrap().restored_fine,[1,0]);assert_eq!(p.state(),State::RxdckDone);
    let d=p.bus_between_stages();pair(d,1);assert_eq!(d.scheduler_calls.len(),2);assert_eq!(d.scheduler_calls[0].1,0);
    assert_eq!(d.scheduler_calls[1].1,d.events.len());assert_eq!(d.btc_event_positions,[0,d.events.len()]);
    let values:Vec<_>=d.events.iter().filter_map(|e|if let Event::Write(0x10370,4,v)=e{Some(*v)}else{None}).collect();
    assert_eq!(values,[0x80500000,0x89200000,0x80030000,0x89300000,0x89200000,0x89200001,0x89200002,0x00565433,
        0x90500000,0x99200000,0x90030000,0x99300000,0x99200000,0x99200001,0x99200000,0x10576543]);
    assert_eq!(d.rf[0][0],0x34567);assert_eq!(d.rf[1][0],0x35678);assert_eq!(d.rf[0][5],0x65433);assert_eq!(d.rf[1][5],0x76543);
    assert_eq!(d.rf[0][0x92],0x12343);assert_eq!(d.rf[1][0x92],0x23441);assert_eq!(d.rf[0][0x93],0xfedc0);assert_eq!(d.rf[1][0x93],0xedcb0);
    assert!(d.now-start>=2_000);assert!(!d.events.iter().any(|e|matches!(e,Event::Write(0x15818|0x17818|0xc348,_,_))));
}
#[test]
fn rxdck_btc_denial_prevents_scheduler_and_rf_writes_before_cleanup() {
    let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);p.bus_between_stages().btc_start_fail=true;
    let error=p.rxdck(dack::Band::Ghz2).unwrap_err();assert_eq!(error.stage,rxdck::Stage::BtcStart);
    let d=p.bus_between_stages();pair(d,0);off(d);assert!(d.scheduler_calls.is_empty());
    assert!(!d.events.iter().any(|e|matches!(e,Event::Write(0x10370,_,_))));assert_eq!(d.btc_event_positions[1],d.events.len());
}
#[test]
fn uncertain_scheduler_pause_never_retries_mailbox_and_cannot_start_rf_calibration() {
    for late in [false,true] {
        let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);let d=p.bus_between_stages();
        if late {d.scheduler_late=Some(1);}else{d.scheduler_fail=Some(1);}
        let error=p.rxdck(dack::Band::Ghz2).unwrap_err();assert_eq!(error.stage,rxdck::Stage::SchedulerPause);assert!(error.scheduler_restore.is_none());
        let d=p.bus_between_stages();assert_eq!(d.scheduler_calls.len(),1);pair(d,0);off(d);assert!(!d.events.iter().any(|e|matches!(e,Event::Write(0x10370,_,_))));
    }
}
#[test]
fn stuck_tx_rf_mode_aborts_with_source_bound_and_restores_zero_scheduler() {
    for path in 0..2 {
        let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);let d=p.bus_between_stages();d.rf[path][0]=0x24567;let start=d.now;
        let error=p.rxdck(dack::Band::Ghz2).unwrap_err();assert_eq!(error.stage,if path==0{rxdck::Stage::WaitA}else{rxdck::Stage::WaitB});
        assert_eq!(error.cause.kind,FaultKind::Timeout);let d=p.bus_between_stages();assert!(d.now-start<6_000);assert_eq!(d.scheduler_calls.len(),2);
        assert!(!d.events.iter().any(|e|matches!(e,Event::Write(0x10370,_,_))));pair(d,0);off(d);
    }
}
#[test]
fn source_delay_failure_restores_fields_then_scheduler_before_releasing_btc() {
    let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);p.bus_between_stages().rx_delay_fail=true;
    let error=p.rxdck(dack::Band::Ghz2).unwrap_err();assert_eq!(error.stage,rxdck::Stage::PathA);assert_eq!(error.rf_restore,[None;2]);
    assert!(error.scheduler_restore.is_none()&&error.scheduler_status.is_none()&&error.radio_stop.is_none()&&error.btc_stop.is_none());
    let d=p.bus_between_stages();assert_eq!(d.rf[0][0],0x14567);assert_eq!(d.rf[0][5],0x65433);assert_eq!(d.rf[0][0x92]&2,2);assert_eq!(d.rf[1][0],0x15678);
    assert_eq!(d.scheduler_calls.len(),2);assert!(d.scheduler_calls[1].1<d.events.len());assert_eq!(d.btc_event_positions[1],d.events.len());pair(d,0);off(d);
}
#[test]
fn failed_or_late_scheduler_restore_cannot_publish_or_retry() {
    for late in [false,true] {
        let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);let d=p.bus_between_stages();
        if late {d.scheduler_late=Some(2);}else{d.scheduler_fail=Some(2);}
        let error=p.rxdck(dack::Band::Ghz5).unwrap_err();assert_eq!(error.stage,rxdck::Stage::SchedulerRestore);
        assert_eq!(error.scheduler_restore.unwrap().kind,if late{FaultKind::Timeout}else{FaultKind::Access});assert_eq!(p.state(),State::Failed);
        let d=p.bus_between_stages();assert_eq!(d.scheduler_calls.len(),2);pair(d,1);off(d);
    }
}
#[test]
fn rxdck_cleanup_errors_remain_independent_and_btc_stop_is_last() {
    let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);let d=p.bus_between_stages();
    d.rx_delay_fail=true;d.restore_fail=true;d.scheduler_fail=Some(2);d.btc_stop_fail=true;d.fail_cleanup=Some(0x2f0);
    let error=p.rxdck(dack::Band::Ghz2).unwrap_err();assert_eq!(error.cause.register,0x92);assert!(error.rf_restore[0].is_some());
    assert!(error.scheduler_restore.is_some()&&error.radio_stop.is_some()&&error.btc_stop.is_some());assert!(error.scheduler_status.is_none());
    let d=p.bus_between_stages();assert_eq!(d.scheduler_calls.len(),2);assert_eq!(d.btc_event_positions[1],d.events.len());pair(d,0);
}
#[test]
fn every_applied_rxdck_register_failure_is_poisoned_and_retains_dma() {
    let count={let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);p.rxdck(dack::Band::Ghz2).unwrap();p.bus_between_stages().events.len()};
    for index in 1..=count {
        let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);let d=p.bus_between_stages();d.fail_write=Some(d.writes+index);
        let error=p.rxdck(dack::Band::Ghz2).unwrap_err();assert_eq!(error.cause.kind,FaultKind::Access,"index{index}");assert_eq!(p.state(),State::Failed);
        let d=p.bus_between_stages();pair(d,0);off(d);assert_eq!(d.scheduler_calls.len(),2);let n=d.events.len();
        assert!(p.rxdck(dack::Band::Ghz2).is_err());assert_eq!(p.bus_between_stages().events.len(),n);
    }
}
#[test]
fn late_rxdck_delay_cannot_finish_or_skip_independent_restoration() {
    let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);p.bus_between_stages().rx_delay_late=true;
    let error=p.rxdck(dack::Band::Ghz2).unwrap_err();assert_eq!(error.stage,rxdck::Stage::PathA);assert_eq!(error.cause.kind,FaultKind::Timeout);
    assert_eq!(error.rf_restore,[None;2]);assert!(error.scheduler_restore.is_none()&&error.radio_stop.is_none());
    let d=p.bus_between_stages();assert_eq!(d.rf[0][0],0x14567);assert_eq!(d.scheduler_calls.len(),2);pair(d,0);off(d);
}
#[test]
fn uncertain_mailbox_actual_nonzero_tx_status_is_reported_without_retry() {
    let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);let d=p.bus_between_stages();
    d.scheduler_fail=Some(1);d.scheduler_dirty=true;
    let error=p.rxdck(dack::Band::Ghz2).unwrap_err();assert_eq!(error.stage,rxdck::Stage::SchedulerPause);
    assert_eq!(error.scheduler_status.unwrap().register,0xc348);assert!(error.radio_stop.is_none()&&error.btc_stop.is_none());
    let d=p.bus_between_stages();assert_eq!(d.scheduler_calls.len(),1);assert_eq!(d.get(2,1)&3,0);assert_eq!(d.btc_event_positions[1],d.events.len());pair(d,0);
}
#[test]
fn failed_btc_stop_never_retries_or_marks_rxdck_complete() {
    let mut d=Device::new();let mut p=Phy::new(&mut d,identity());prepared(&mut p);p.bus_between_stages().btc_stop_fail=true;
    let error=p.rxdck(dack::Band::Ghz2).unwrap_err();assert_eq!(error.stage,rxdck::Stage::BtcStop);assert!(error.btc_stop.is_some());
    assert_eq!(error.rf_restore,[None;2]);assert_eq!(p.state(),State::Failed);
    let d=p.bus_between_stages();assert_eq!(d.scheduler_calls.len(),2);assert_eq!(d.rf[0][0],0x14567);assert_eq!(d.rf[1][0],0x15678);pair(d,0);off(d);
}
