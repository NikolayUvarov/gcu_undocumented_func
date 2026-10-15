use super::*;
use phy::{channel::{self,Access,ChannelIo,Platform,Policy,PolicyError,Rule},dm,tail};
use channel_power::Constraints;

impl ChannelIo for Device{fn scheduler_zero(&mut self,timeout:u32)->Result<()>{rxdck::RxdckIo::scheduler_zero(self,timeout)}}
pub(super) fn board()->board::Board{board::Board{identity:identity(),maps:efuse::Maps{logical:[255;2048],dav_logical:[255;16],phycap:[255;128]}}}
pub(super) fn rule()->Rule{Rule{country:*b"US",version:1,center_mhz:2437,access:Access::InitiateRadiation,eirp_half_dbm:40,conducted_half_dbm:34}}
pub(super) fn platform()->Platform{Platform::Resolved(Constraints{per_ntx:[30,24],per_path:[30,30]})}
pub(super) fn with_prepared(check:impl FnOnce(&mut Phy<Device>,dm::Prefix,tail::Configured,download::DefaultTables)){
    phycap::tests::with_capabilities_and_tables([0x20403,2,0,0x20000],|cap,tables|{
        let mut d=Device::new();let mut p=Phy::new(&mut d,identity());let prefix=tail_cases::prepared(&mut p,&cap,dack::Band::Ghz2);
        let configured=tail::run(&mut p,&board(),&prefix,&cap,tail::Subband::Ghz2).unwrap();
        let d=p.bus_between_stages();d.events.clear();d.writes=0;d.scheduler_calls.clear();d.rf[0][0xc5]=1<<15;
        d.rf[0][0xb1]=0x54321;d.rf[0][0x18]=0xa7701;d.rf[1][0x18]=0x98701;
        d.put(0x1e060,4,0x6c001);d.put(0x1f060,4,0x7c001);d.put(0xce40,4,0x2b);
        check(&mut p,prefix,configured,tables);
    });
}
fn assert_off(d:&Device){assert_eq!(d.get(2,1)&3,0);assert_eq!(d.get(0x2f0,4)&(1<<17),0);assert_eq!(d.xtal[0x80]&7,0);assert_eq!(d.xtal[0x81]&7,0);}

#[test]
fn policy_requires_actual_platform_and_frequency_without_country_guessing(){
    assert_eq!(Policy::resolve(&board(),rule(),Platform::Unknown).unwrap_err(),PolicyError::UnknownPlatform);
    for case in 0..5{let mut r=rule();match case{0=>r.country=*b"DE",1=>r.version=0,2=>r.center_mhz=2484,3=>r.access=Access::Disabled,_=>r.conducted_half_dbm=41}
        assert!(Policy::resolve(&board(),r,platform()).is_err());}
    let mut erased=board();erased.identity.country=[255,255];assert!(Policy::resolve(&erased,rule(),platform()).is_ok());
    let mut r=rule();r.access=Access::PassiveOnly;assert!(Policy::resolve(&board(),r,platform()).is_ok());
}
#[test]
fn calibration_admission_requires_radiation_and_exact_engine_identity(){
    use channel::{CalibrationEngine as E,CalibrationTransmit as Permit};
    let mut tuned=channel::Tuned{attempt:1,session:2,channel:6,policy_version:3,access:Access::InitiateRadiation};
    let token=Permit::modeled(&tuned,E::Tssi).unwrap();
    assert!(token.matches(&tuned,E::Tssi));
    assert!(!token.matches(&tuned,E::Iqk));assert!(!token.matches(&tuned,E::Dpk));
    for field in 0..5{
        let mut other=channel::Tuned{attempt:1,session:2,channel:6,policy_version:3,access:Access::InitiateRadiation};
        match field{0=>other.attempt+=1,1=>other.session+=1,2=>other.channel+=1,3=>other.policy_version+=1,_=>other.access=Access::PassiveOnly}
        assert!(!token.matches(&other,E::Tssi));
    }
    for field in 0..5{
        match field{0=>tuned.attempt=0,1=>{tuned.attempt=1;tuned.session=0;},2=>{tuned.session=2;tuned.policy_version=0;},
            3=>{tuned.policy_version=3;tuned.channel=14;},_=>{tuned.channel=6;tuned.access=Access::PassiveOnly;}}
        assert!(Permit::modeled(&tuned,E::Tssi).is_none());
    }
}
#[test]
fn complete_channel_transaction_orders_mac_bb_rf_power_and_retains_zero_packet_tx(){
    with_prepared(|p,prefix,configured,tables|{
        let done=channel::run(p,&board(),&prefix,&configured,&tables,&Policy::resolve(&board(),rule(),platform()).unwrap()).unwrap();
        assert_eq!(p.state(),State::ChannelTuned);assert_eq!((done.channel,done.policy_version,done.access),(6,1,Access::InitiateRadiation));
        let d=p.bus_between_stages();assert_eq!(d.scheduler_calls.len(),2);assert_eq!(d.get(0xc348,4),0);assert_eq!(d.get(0x1010,4),0xb0f03);
        let positions:Vec<_>=[0xce40,0x158dc,0x120fc,0xc010,0x123b0,0x1e060,0xd2c0,0xd2ec,0xd33c,0xd200].iter()
            .map(|at|d.events.iter().position(|e|matches!(e,Event::Write(a,_,_)if a==at)).unwrap()).collect();
        assert!(positions.windows(2).all(|p|p[0]<p[1]));
        assert_eq!(d.get(0xce40,4),0x2b);assert_eq!(d.get(0x9c18,4)&3,1);
        assert_eq!(d.get(0x120fc,4)>>24,0);assert_eq!(d.get(0x10734,4)>>16&255,6);
        assert_eq!(d.get(0x149c0,4)&127,108);assert_eq!(d.rf[0][0xb1],0x54321);
        for value in [d.rf[0][0x18],d.rf[1][0x18],d.get(0x1e060,4),d.get(0x1f060,4)]{assert_eq!(value&(0x30000|0xffff),0x1c06);}
        for path in 0..2{assert_eq!(d.rf[path][0xee]&4,0);assert_eq!(d.rf[path][0x33]&31,0x12);assert_eq!(d.rf[path][0x3f]&63,0x1b);}
        let n=d.events.len();assert!(channel::run(p,&board(),&prefix,&configured,&tables,&Policy::resolve(&board(),rule(),platform()).unwrap()).is_err());assert_eq!(p.bus_between_stages().events.len(),n);
    });
}
#[test]
fn channel_and_rx_path_values_follow_board_session_configuration(){
    for(ch,sco)in[(1,109),(6,108),(10,107),(13,106)]{with_prepared(|p,prefix,configured,tables|{
        let mut r=rule();r.center_mhz=2407+ch*5;r.access=Access::PassiveOnly;
        let done=channel::run(p,&board(),&prefix,&configured,&tables,&Policy::resolve(&board(),r,platform()).unwrap()).unwrap();
        assert_eq!(done.access,Access::PassiveOnly);assert_eq!(done.channel,ch as u8);let d=p.bus_between_stages();
        assert_eq!(d.get(0x149c0,4)&127,sco);assert_eq!(d.get(0x149c4,4)&15,3);assert_eq!(d.get(0x149c4,4)&0x3f00,0);
        assert_eq!(d.get(0x1c0e4,4)&0x30,0x20);assert_eq!(d.get(0x1c1e4,4)&0x30,0x20);
    });}
}
#[test]
fn unknown_or_foreign_identity_is_refused_before_scheduler_or_register_writes(){
    for case in 0..5{with_prepared(|p,prefix,mut configured,tables|{
        let mut b=board();match case{0=>configured.session+=1,1=>configured.attempt+=1,2=>b.identity.channel_plan^=1,3=>p.bus_between_stages().session=0,_=>p.mark_failed()}
        let policy=Policy::resolve(&board(),rule(),platform()).unwrap();assert!(channel::run(p,&b,&prefix,&configured,&tables,&policy).is_err());
        let d=p.bus_between_stages();assert!(d.events.is_empty());assert!(d.scheduler_calls.is_empty());
    });}
}
#[test]
fn negative_power_limit_readback_accepts_valid_all_ones_register(){
    with_prepared(|p,prefix,configured,tables|{
        let platform=Platform::Resolved(Constraints{per_ntx:[-1,-1],per_path:[0,0]});
        channel::run(p,&board(),&prefix,&configured,&tables,&Policy::resolve(&board(),rule(),platform).unwrap()).unwrap();
        assert_eq!(p.bus_between_stages().get(0xd2f0,4),u32::MAX);
    });
}
#[test]
fn lpf_timeout_restores_original_ldo_then_stops_radio_and_keeps_primary_error(){
    with_prepared(|p,prefix,configured,tables|{
        p.bus_between_stages().rf[0][0xb7]=1<<8;
        let e=channel::run(p,&board(),&prefix,&configured,&tables,&Policy::resolve(&board(),rule(),platform()).unwrap()).unwrap_err();
        assert_eq!(e.stage,channel::Stage::Radio);assert_eq!(e.cause.kind,FaultKind::Timeout);assert!(e.radio_cleanup.is_none());assert!(e.radio_stop.is_none());
        assert_eq!(p.state(),State::Failed);let d=p.bus_between_stages();assert_eq!(d.rf[0][0xb1],0x54321);assert_off(d);assert_eq!(d.scheduler_calls.len(),2);
    });
}
#[test]
fn unlocked_synth_runs_finite_recovery_then_refuses_without_leaving_lut_or_trigger_open(){
    with_prepared(|p,prefix,configured,tables|{
        p.bus_between_stages().rf[0][0xc5]=0;
        let e=channel::run(p,&board(),&prefix,&configured,&tables,&Policy::resolve(&board(),rule(),platform()).unwrap()).unwrap_err();
        assert_eq!(e.stage,channel::Stage::Radio);assert_eq!(e.cause.register,0xc5);let d=p.bus_between_stages();
        assert_eq!(d.rf[0][0xd3]&(1<<8),0);assert_eq!(d.rf[0][0xdd]&(1<<4),0);assert_eq!(d.rf[0][0xd5]&(1<<8),0);assert_eq!(d.rf[0][0xb1],0x54321);assert_off(d);
    });
}
#[test]
fn uncertain_pause_and_restore_are_never_retried(){
    for call in [1,2]{with_prepared(|p,prefix,configured,tables|{
        p.bus_between_stages().scheduler_fail=Some(call);
        let e=channel::run(p,&board(),&prefix,&configured,&tables,&Policy::resolve(&board(),rule(),platform()).unwrap()).unwrap_err();
        assert_eq!(e.stage,if call==1{channel::Stage::Pause}else{channel::Stage::Restore});assert!(e.scheduler_restore.is_none());
        let d=p.bus_between_stages();assert_eq!(d.scheduler_calls.len(),call);assert_off(d);
    });}
}
#[test]
fn power_readback_failure_disables_radio_and_never_completes_channel(){
    let mut write=0;
    with_prepared(|p,prefix,configured,tables|{
        channel::run(p,&board(),&prefix,&configured,&tables,&Policy::resolve(&board(),rule(),platform()).unwrap()).unwrap();
        write=p.bus_between_stages().events.iter().position(|e|matches!(e,Event::Write(0xd2c0,_,_))).unwrap()+1;
    });
    with_prepared(|p,prefix,configured,tables|{
        p.bus_between_stages().ignore_write=Some(write);
        let e=channel::run(p,&board(),&prefix,&configured,&tables,&Policy::resolve(&board(),rule(),platform()).unwrap()).unwrap_err();
        assert_eq!(e.stage,channel::Stage::Power);assert_eq!(e.cause.register,0xd2c0);assert_eq!(p.state(),State::Failed);assert_off(p.bus_between_stages());
    });
}
#[test]
fn ignored_mac_bandwidth_subcarrier_or_band_write_cannot_report_tuned(){
    for at in [0xc010,0xc088,0xc628]{
        let mut index=0;
        with_prepared(|p,prefix,configured,tables|{
            channel::run(p,&board(),&prefix,&configured,&tables,&Policy::resolve(&board(),rule(),platform()).unwrap()).unwrap();
            index=p.bus_between_stages().events.iter().position(|e|matches!(e,Event::Write(a,_,_)if *a==at)).unwrap()+1;
        });
        with_prepared(|p,prefix,configured,tables|{
            let d=p.bus_between_stages();d.put(at,if at==0xc088{4}else{1},3);d.ignore_write=Some(index);
            let e=channel::run(p,&board(),&prefix,&configured,&tables,&Policy::resolve(&board(),rule(),platform()).unwrap()).unwrap_err();
            assert_eq!(e.stage,channel::Stage::Mac);assert_eq!(e.cause.register,at);assert_off(p.bus_between_stages());
        });
    }
}
#[test]
fn changed_receive_path_invalidates_configured_token_before_pause(){
    with_prepared(|p,prefix,configured,tables|{
        p.bus_between_stages().put(0x149c4,4,1);
        let e=channel::run(p,&board(),&prefix,&configured,&tables,&Policy::resolve(&board(),rule(),platform()).unwrap()).unwrap_err();
        assert_eq!(e.stage,channel::Stage::Guard);assert_eq!(e.cause.register,0x149c4);
        assert!(p.bus_between_stages().events.is_empty());assert!(p.bus_between_stages().scheduler_calls.is_empty());
    });
}
#[test]
fn absolute_deadline_includes_last_successful_register_write_and_reports_cleanup_failure(){
    with_prepared(|p,prefix,configured,tables|{
        let d=p.bus_between_stages();d.deadline_write=Some(0xd200);d.deadline_us=2_200_000;d.xtal_stuck=true;
        let e=channel::run(p,&board(),&prefix,&configured,&tables,&Policy::resolve(&board(),rule(),platform()).unwrap()).unwrap_err();
        assert_eq!(e.cause.kind,FaultKind::Timeout);assert!(e.radio_stop.is_some());assert_eq!(p.state(),State::Failed);
    });
}
