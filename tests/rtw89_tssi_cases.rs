use super::*;
use phy::{channel::{self,CalibrationEngine as Engine,CalibrationTransmit as Admission},tssi,dm};

fn prepared(check:impl FnOnce(&mut Phy<Device>,dm::Prefix,channel::Tuned)){
    channel_cases::with_prepared(|phy,prefix,configured,tables|{
        let board=channel_cases::board();let policy=channel::Policy::resolve(&board,channel_cases::rule(),channel_cases::platform()).unwrap();
        let tuned=channel::run(phy,&board,&prefix,&configured,&tables,&policy).unwrap();
        let d=phy.bus_between_stages();d.events.clear();d.btc.clear();d.btc_event_positions.clear();d.scheduler_calls.clear();d.writes=0;
        d.tssi=Some(TssiModel{delay:40,..TssiModel::default()});d.put(0x1458c,4,3<<28);d.put(0x14594,4,0x1f4<<22);
        check(phy,prefix,tuned);
    });
}
fn run(p:&mut Phy<Device>,prefix:&dm::Prefix,tuned:&channel::Tuned)->std::result::Result<tssi::Completed,tssi::Failure>{
    let permission=Admission::modeled(tuned,Engine::Tssi).unwrap();
    tssi::modeled_engine(p,&channel_cases::board(),prefix,tuned,&permission)
}
fn off(d:&Device){assert_eq!(d.get(2,1)&3,0);assert_eq!(d.get(0x2f0,4)&(1<<17),0);assert_eq!(d.get(0x109c4,4)&16,0);
    assert_eq!(d.get(0x10980,4)&0x10001,0);assert_eq!(d.get(0x109a4,4)&0x1041c,0);
    let m=d.tssi.as_ref().unwrap();assert!(!m.outer&&!m.nested);}

#[test]
fn tssi_source_thermal_tables_keep_saturation_sign_and_erased_fallback(){
    assert_eq!(tssi::modeled_thermal(0,255),[0;16]);assert_eq!(tssi::modeled_thermal(1,255),[0;16]);
    let a:Vec<_>=tssi::modeled_thermal(0,23).into_iter().flat_map(u32::to_le_bytes).collect();
    let b:Vec<_>=tssi::modeled_thermal(1,23).into_iter().flat_map(u32::to_le_bytes).collect();
    assert_eq!(&a[..32],&[0;32]);assert_eq!(&b[..14],&[0;14]);assert_eq!(&b[14..28],&[1;14]);assert_eq!(&b[28..32],&[2;4]);
    assert_eq!((a[32],a[63],b[32],b[63]),(5,1,6,1));
}
#[test]
fn tssi_actual_pmac_four_samples_restore_paths_then_enable_both_calibrated_paths(){
    prepared(|p,prefix,tuned|{
        let done=run(p,&prefix,&tuned).unwrap();assert!(done.matches(&tuned));assert_eq!(done.is_tssi_mode(),[true;2]);
        assert_eq!(done.reports,[[160,100],[200,142]]);assert_eq!(done.alignment[0][0],done.alignment[0][2]);
        let d=p.bus_between_stages();assert_eq!(d.tssi.as_ref().unwrap().shots,[(0,48,100,5000),(0,20,100,5000),(1,48,100,5000),(1,20,100,5000)]);
        assert_eq!(d.scheduler_calls.len(),4);assert_eq!(d.btc.iter().map(|v|v.0.state as u8).collect::<Vec<_>>(),[1,2,3,0]);
        assert_eq!(d.get(0x1458c,4)>>28,3);assert_eq!(d.get(0x149c4,4)&15,3);assert_eq!(d.get(0x14594,4)>>22&511,0x1f4);
        assert_eq!(d.get(0x109c4,4)&16,0);assert_eq!(d.get(0x10980,4)&0x10001,0);assert_eq!(d.get(0x109a4,4)&0x1041c,0);
        for path in 0..2{let off=path*0x2000;assert_eq!(d.get(0x15820+off,4)>>31,1);assert_eq!(d.get(0x15818+off,4)&255,0xc0);
            assert_eq!(d.get(0x15838+off,4)>>12&1023,1023);assert_eq!(d.get(0x15858+off,4)>>12&1023,1023);}
        assert_eq!(p.state(),State::ChannelTuned);
    });
}
#[test]
fn tssi_passive_wrong_engine_or_policy_cannot_reach_any_register_write(){
    for case in 0..3{prepared(|p,prefix,mut tuned|{
        let permission=Admission::modeled(&tuned,if case==0{Engine::Iqk}else{Engine::Tssi}).unwrap();
        if case==1{tuned.access=channel::Access::PassiveOnly;}if case==2{tuned.policy_version+=1;}
        assert_eq!(tssi::modeled_engine(p,&channel_cases::board(),&prefix,&tuned,&permission).unwrap_err().stage,tssi::Stage::Guard);
        let d=p.bus_between_stages();assert!(d.events.is_empty());assert!(d.btc.is_empty());assert!(d.scheduler_calls.is_empty());
    });}
}
#[test]
fn tssi_timeout_stops_pmac_and_restores_before_releasing_grants(){
    prepared(|p,prefix,tuned|{
        p.bus_between_stages().tssi.as_mut().unwrap().stuck=true;
        let e=run(p,&prefix,&tuned).unwrap_err();assert_eq!(e.stage,tssi::Stage::AlignmentA);assert_eq!(e.cause.kind,FaultKind::Timeout);
        assert!(e.pmac_stop.is_none()&&e.restore.is_none()&&e.radio_stop.is_none()&&e.oneshot_stop.is_none()&&e.outer_stop.is_none());
        assert_eq!(p.state(),State::Failed);let d=p.bus_between_stages();off(d);assert_eq!(d.tssi.as_ref().unwrap().shots.len(),1);
        assert_eq!(d.get(0x1458c,4)>>28,3);assert_eq!(d.get(0x149c4,4)&15,3);assert_eq!(d.scheduler_calls.len(),2);
        let radio_off=d.events.iter().position(|e|matches!(e,Event::Write(2,1,value)if value&3==0)).unwrap();
        assert!(d.btc_event_positions[2]>radio_off);assert!(d.btc_event_positions[3]>radio_off);
    });
}
#[test]
fn tssi_stale_ready_report_is_not_accepted_as_new_alignment(){
    prepared(|p,prefix,tuned|{p.bus_between_stages().tssi.as_mut().unwrap().stale=true;
        let e=run(p,&prefix,&tuned).unwrap_err();assert_eq!(e.cause.kind,FaultKind::Timeout);
        let d=p.bus_between_stages();assert!(d.tssi.as_ref().unwrap().shots.is_empty());off(d);});
}
#[test]
fn tssi_unknown_pause_or_restore_is_never_retried(){
    for call in [1,2,3,4]{prepared(|p,prefix,tuned|{
        p.bus_between_stages().scheduler_fail=Some(call);let e=run(p,&prefix,&tuned).unwrap_err();
        assert_eq!(e.stage,if call%2==1{tssi::Stage::Pause}else{tssi::Stage::Restore});
        let d=p.bus_between_stages();assert_eq!(d.scheduler_calls.len(),call);off(d);
    });}
}
#[test]
fn tssi_failed_nested_admission_still_closes_inner_before_outer_once(){
    prepared(|p,prefix,tuned|{p.bus_between_stages().tssi.as_mut().unwrap().fail_notice=Some(dack::RfkState::OneshotStart);
        let e=run(p,&prefix,&tuned).unwrap_err();assert_eq!(e.stage,tssi::Stage::OneshotStart);
        let d=p.bus_between_stages();assert!(d.tssi.as_ref().unwrap().shots.is_empty());
        assert_eq!(d.btc.iter().map(|v|v.0.state as u8).collect::<Vec<_>>(),[1,2,3,0]);off(d);});
}
#[test]
fn tssi_failed_stop_is_not_retried_and_keeps_fault_visible(){
    for state in [dack::RfkState::OneshotStop,dack::RfkState::Stop]{prepared(|p,prefix,tuned|{
        p.bus_between_stages().tssi.as_mut().unwrap().fail_notice=Some(state);let e=run(p,&prefix,&tuned).unwrap_err();
        assert_eq!(e.cause.kind,FaultKind::Access);assert_eq!(e.stage,if state==dack::RfkState::Stop{tssi::Stage::OuterStop}else{tssi::Stage::OneshotStop});
        let d=p.bus_between_stages();assert_eq!(d.btc.iter().filter(|v|v.0.state==state).count(),1);off(d);
    });}
}
#[test]
fn tssi_caller_deadline_rejects_late_success_and_cleans_up(){
    prepared(|p,prefix,tuned|{p.bus_between_stages().tssi.as_mut().unwrap().late_notice=Some(dack::RfkState::OneshotStop);
        let e=run(p,&prefix,&tuned).unwrap_err();assert_eq!(e.cause.kind,FaultKind::Timeout);off(p.bus_between_stages());});
}
#[test]
fn tssi_independent_cleanup_preserves_primary_timeout_and_radio_failure(){
    prepared(|p,prefix,tuned|{let d=p.bus_between_stages();d.tssi.as_mut().unwrap().stuck=true;d.fail_cleanup=Some(0x2f0);
        let e=run(p,&prefix,&tuned).unwrap_err();assert_eq!(e.cause.kind,FaultKind::Timeout);assert!(e.radio_stop.is_some());
        assert!(e.pmac_stop.is_none());assert_eq!(p.bus_between_stages().btc.last().unwrap().0.state,dack::RfkState::Stop);});
}
#[test]
fn tssi_unconfirmed_pmac_stop_disables_radio_without_restoring_live_power_or_paths(){
    prepared(|p,prefix,tuned|{p.bus_between_stages().tssi.as_mut().unwrap().ignore_pmac_stop=true;
        let e=run(p,&prefix,&tuned).unwrap_err();assert_eq!(e.stage,tssi::Stage::AlignmentA);
        assert!(e.pmac_stop.is_some()&&e.restore_skipped);assert!(e.radio_stop.is_none());
        let d=p.bus_between_stages();assert_eq!(d.get(2,1)&3,0);assert_eq!(d.get(0x2f0,4)&(1<<17),0);
        let stop=d.events.iter().position(|e|matches!(e,Event::Write(0x109c4,4,v)if v&16==0&&v>>8==5000)).unwrap();
        assert!(!d.events[stop..].iter().any(|e|matches!(e,Event::Write(0x14594|0x1458c|0x112ac|0x132ac,_,_))));
        assert_eq!(d.get(0x1458c,4)>>28,1);assert!(!d.tssi.as_ref().unwrap().outer);
    });
}
#[test]
fn tssi_applied_stop_error_is_rechecked_before_saved_register_restoration(){
    prepared(|p,prefix,tuned|{p.bus_between_stages().tssi.as_mut().unwrap().fail_pmac_stop=true;
        let e=run(p,&prefix,&tuned).unwrap_err();assert_eq!(e.cause.kind,FaultKind::Access);
        assert!(e.pmac_stop.is_none()&&!e.restore_skipped&&e.restore.is_none());
        let d=p.bus_between_stages();off(d);assert_eq!(d.get(0x1458c,4)>>28,3);
    });
}

// 213-DRV-0087: the 5 GHz groups, thermal rows and tables of rtw8852b_tssi on channel 100.
#[test]
fn tssi_5_ghz_groups_and_thermal_rows_follow_linux(){
    for ch in 1..=13u8{let old=match ch{1..=2=>0,3..=5=>1,6..=8=>2,9..=11=>3,_=>4};
        assert_eq!((tssi::ofdm_group(ch),tssi::cck_group(ch),tssi::trim_group(ch)),(Some((old,false)),old,Some(usize::from(ch>=9))),"{ch}");}
    for (ch,group,trim) in [(36,(5,false),2),(64,(8,false),3),(100,(9,false),4),(108,(10,false),4),(116,(11,false),5),
        (132,(13,false),6),(144,(14,false),6),(149,(15,false),7),(165,(17,false),7),(177,(18,false),7)]{
        assert_eq!((tssi::ofdm_group(ch),tssi::trim_group(ch),tssi::cck_group(ch)),(Some(group),Some(trim),0),"{ch}");}
    assert_eq!(tssi::ofdm_group(42),Some((5,true)));assert_eq!(tssi::ofdm_group(170),Some((17,true)));
    for ch in crate::band::CHANNELS_5G{assert!(tssi::ofdm_group(ch).is_some()&&tssi::trim_group(ch).is_some(),"{ch}");}
    assert_eq!((tssi::ofdm_group(34),tssi::trim_group(145)),(None,None));
    // Path A, 5 GHz middle (100-144), thermal 23: the second rows of delta_swingidx_5ga_n and _p.
    let a:Vec<_>=tssi::modeled_thermal_in(0,23,crate::band::Subband::Ghz5Middle).into_iter().flat_map(u32::to_le_bytes).collect();
    let(down,up)=(crate::phy::tssi::tables::THERMAL_5G_A_N[1],crate::phy::tssi::tables::THERMAL_5G_A_P[1]);
    for j in 0..32{assert_eq!(a[j] as i8,-down[j.min(29)]);assert_eq!(a[63-j] as i8,up[(j+1).min(29)]);}
    assert_eq!(tssi::modeled_thermal_in(1,255,crate::band::Subband::Ghz5High),[0;16]);
}
#[test]
fn tssi_on_channel_100_takes_txa_the_5_ghz_tables_and_no_btg(){
    channel_cases::with_prepared(|phy,prefix,configured,tables|{
        let board=channel_cases::board();
        let rule=channel::Rule{center_mhz:5500,..channel_cases::rule()};
        let policy=channel::Policy::resolve(&board,rule,channel_cases::platform()).unwrap();
        let tuned=channel::run(phy,&board,&prefix,&configured,&tables,&policy).unwrap();assert_eq!(tuned.channel,100);
        let d=phy.bus_between_stages();d.events.clear();d.btc.clear();d.btc_event_positions.clear();d.scheduler_calls.clear();d.writes=0;
        d.tssi=Some(TssiModel{delay:40,..TssiModel::default()});d.put(0x1458c,4,3<<28);d.put(0x14594,4,0x1f4<<22);
        let done=run(phy,&prefix,&tuned).unwrap();assert!(done.matches(&tuned));
        let d=phy.bus_between_stages();
        for path in 0..2{assert_eq!(d.rf[path][0x7f]&(1<<8),1<<8,"RR_TXPOW_TXA on path {path}");}
        assert_eq!(d.get(0x1120c,4)&255,0x44,"tssi_sys_a_defs_5g");assert_eq!(d.get(0x158f8,4)&(1<<30),0);
        assert_eq!(d.get(0x15600,4)&0x3fffffff,0x3f2d2721,"tssi_align_a_5g2_all_defs");
        assert_eq!(d.get(0x14738,4)&(1<<17|1<<19),0,"no BTG and the 5 GHz band select through the PMAC samples");
        assert_eq!(d.get(0x12344,4)>>31,1,"CCK's CCA stays disabled after the PMAC stop on 5 GHz");
        assert!(d.btc.iter().all(|v|v.0.phy_map>>6==1),"every notice names 5 GHz");
        assert_eq!(phy.state(),State::ChannelTuned);
    });
}
