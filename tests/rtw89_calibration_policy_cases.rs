use crate::{board::Board,btc_context::{self,Initialized},btc_session::Session,channel_power::Constraints,
    channel_rfk::{self,Authorities},dack::Band,download::{self,DefaultTables},model,phy::{self,Phy,State,
    channel::{self,Access,Rule,Tuned,calibration_policy::{Resolved,PlatformInput,PlatformBasis,SourceCalibrationProfile,Error}}},
    phycap,post_mac,runtime_mac,rx};
#[path="rtw89_initial_phy_device.rs"] mod device;
use device::{Device,channel::ChannelModel};

fn rule()->Rule{Rule{country:*b"US",version:1,center_mhz:2437,access:Access::InitiateRadiation,eirp_half_dbm:40,conducted_half_dbm:34}}
fn limits()->PlatformInput{PlatformInput::Limits(Constraints{per_ntx:[30,24],per_path:[30,30]})}
fn board()->Board{model::read_board(&mut model::Device::new(2))}
fn with_channel(check:impl FnOnce(&mut Phy<'_,Session<'_,'_,'_,Device>>,&Board,&DefaultTables,&mut Tuned,&Initialized,Resolved,&phy::dm::Prefix)){
    download::tests::with_synthetic_firmware(|verified|{
        let mut raw=model::Device::new(2);let board=model::read_board(&mut raw);
        let mut region=model::Region::new();let(mut queue,mut rings)=region.pair(&mut raw);
        model::prepare_receive(&mut raw,&mut queue,&mut rings).unwrap();let ready=download::run(&mut raw,&mut queue,verified).unwrap();
        let tables=ready.default_tables();let caps=phycap::query(&mut raw,&mut queue,&board,runtime_mac::BAR_BYTES,&ready,phycap::MAX_TIMEOUT_US).unwrap();
        let runtime=runtime_mac::prepare(&mut raw,&mut queue,&board,runtime_mac::BAR_BYTES,&ready).unwrap();
        let receiving=post_mac::activate(&mut raw,&mut queue,&mut rings,&board,runtime_mac::BAR_BYTES,&runtime).unwrap();
        let mut d=Device::new(raw);d.channel=Some(ChannelModel::default());let mut scratch=vec![0;rx::BUFFER_BYTES];
        {
            let mut session=Session::new(&mut d,&mut queue,&mut rings,&receiving,ready,&mut scratch).unwrap();
            let mut phy=Phy::new(&mut session,board.identity);phy.baseband().unwrap();phy.radio().unwrap();
            let owner=btc_context::initialize(&mut phy,&board).unwrap();let prefix=phy::dm::run(&mut phy,Band::Ghz2).unwrap();
            phy.nctl().unwrap();phy.rck().unwrap();phy.dack(Band::Ghz2).unwrap();phy.rxdck(Band::Ghz2).unwrap();
            let configured=phy::tail::run(&mut phy,&board,&prefix,&caps,phy::tail::Subband::Ghz2).unwrap();
            let resolved=Resolved::resolve(&board,rule(),limits()).unwrap();
            let mut tuned=channel::run(&mut phy,&board,&prefix,&configured,&tables,resolved.policy()).unwrap();
            check(&mut phy,&board,&tables,&mut tuned,&owner,resolved,&prefix);
        }
        let stop=rings.stop(&mut d);assert_eq!((stop.request,stop.hci,stop.idle),(None,None,None));
        runtime_mac::disable_radio(&mut d).unwrap();assert_eq!(download::stop(&mut d,&mut queue),download::Stop{dma:None,cpu:None});
        model::power_off(&mut d.inner);
    });
}

#[test]
fn checked_absent_is_resolved_but_unknown_and_invalid_platform_are_distinct(){
    let b=board();let resolved=Resolved::resolve(&b,rule(),PlatformInput::CheckedAbsent).unwrap();
    assert_eq!(resolved.platform_basis(),PlatformBasis::NoAdditionalSar);
    assert_eq!(resolved.profile(),SourceCalibrationProfile::Rtl8852bCbvRfe1Initial2g20V1);
    assert!(matches!(Resolved::resolve(&b,rule(),PlatformInput::Unknown),Err(Error::UnknownPlatform)));
    assert!(matches!(Resolved::resolve(&b,rule(),PlatformInput::Invalid),Err(Error::InvalidPlatform)));
    assert_eq!(Resolved::resolve(&b,rule(),limits()).unwrap().platform_basis(),PlatformBasis::ExplicitLimits);
}
#[test]
fn supported_profile_requires_real_consistent_board_identity_and_internal_pa(){for change in 0..7{
    let mut b=board();match change{0=>b.identity.cut=0,1=>b.identity.rfe=2,2=>b.maps.logical[0x2ca]=2,
        3=>b.maps.logical[0x400]^=2,4=>b.maps.phycap[0x6c]=0,5=>b.maps.phycap[0x6d]=0,_=>b.identity.xtal^=1}
    assert!(matches!(Resolved::resolve(&b,rule(),limits()),Err(Error::Board)),"case {change}");
}}
#[test]
fn country_frequency_passive_and_disabled_rules_do_not_grant_calibration(){for change in 0..6{
    let mut r=rule();match change{0=>r.country=*b"DE",1=>r.version=0,2=>r.center_mhz=2484,
        3=>r.access=Access::Disabled,4=>r.access=Access::PassiveOnly,_=>r.conducted_half_dbm=41}
    assert!(Resolved::resolve(&board(),r,limits()).is_err(),"case {change}");
}}
#[test]
fn tssi_twelve_dbm_boundary_checks_both_paths_one_tx_and_country_conducted_cap(){
    let b=board();for which in 0..4{for cap in [23,24]{let mut r=rule();let mut c=Constraints{per_ntx:[24,0],per_path:[24,24]};
        match which{0=>c.per_ntx[0]=cap,1=>c.per_path[0]=cap,2=>c.per_path[1]=cap,_=>r.conducted_half_dbm=cap}
        let result=Resolved::resolve(&b,r,PlatformInput::Limits(c));
        if cap==23{assert!(matches!(result,Err(Error::TssiPower)),"case {which}");}else{result.unwrap();}
    }}
}
#[test]
fn source_advisory_power_flag_and_erased_trim_do_not_invent_a_certification_gate(){
    let mut b=board();b.maps.phycap[0x69]=255;
    assert!(!crate::calibration::decode(&b).unwrap().power_k_valid);
    Resolved::resolve(&b,rule(),limits()).unwrap();
}
#[test]
fn actual_cold_channel_and_checked_provider_run_all_three_source_calibrations(){with_channel(|phy,b,tables,tuned,owner,resolved,prefix|{
    let [iqk,tssi,dpk]=resolved.grant(phy,b,tables,tuned,owner).unwrap();
    assert!(iqk.matches(tuned,channel::CalibrationEngine::Iqk));assert!(!iqk.matches(tuned,channel::CalibrationEngine::Dpk));
    let done=channel_rfk::run(phy,b,prefix,tuned,owner,Authorities{iqk,tssi,dpk}).unwrap();assert!(done.matches(tuned));
});}
#[test]
fn same_version_different_caps_cannot_replace_written_policy(){with_channel(|phy,b,tables,tuned,owner,_,_|{
    let mut c=Constraints{per_ntx:[30,24],per_path:[30,30]};c.per_ntx[0]=29;
    let replacement=Resolved::resolve(b,rule(),PlatformInput::Limits(c)).unwrap();
    assert!(matches!(replacement.grant(phy,b,tables,tuned,owner),Err(Error::PolicyChanged)));
    let replacement=Resolved::resolve(b,rule(),limits()).unwrap();replacement.grant(phy,b,tables,tuned,owner).unwrap();
});}
#[test]
fn changed_actual_otp_and_edited_tuned_identity_never_mint_tokens(){with_channel(|phy,b,tables,tuned,owner,_,_|{
    let mut changed=Board{identity:b.identity,maps:crate::efuse::Maps{logical:b.maps.logical,dav_logical:b.maps.dav_logical,phycap:b.maps.phycap}};
    changed.maps.logical[0x210]^=1;
    let resolved=Resolved::resolve(&changed,rule(),limits()).unwrap();
    assert!(matches!(resolved.grant(phy,&changed,tables,tuned,owner),Err(Error::Identity)));
    let original=tuned.session;tuned.session+=1;
    assert!(matches!(Resolved::resolve(b,rule(),limits()).unwrap().grant(phy,b,tables,tuned,owner),Err(Error::Identity)));
    tuned.session=original;Resolved::resolve(b,rule(),limits()).unwrap().grant(phy,b,tables,tuned,owner).unwrap();
});}
#[test]
fn one_tuned_result_cannot_mint_another_set_even_from_a_new_identical_policy(){with_channel(|phy,b,tables,tuned,owner,resolved,_|{
    resolved.grant(phy,b,tables,tuned,owner).unwrap();
    assert!(matches!(Resolved::resolve(b,rule(),limits()).unwrap().grant(phy,b,tables,tuned,owner),Err(Error::AlreadyAttempted)));
});}
#[test]
fn foreign_firmware_default_token_and_failed_phy_do_not_mint_authority(){with_channel(|phy,b,tables,tuned,owner,_,_|{
    download::tests::with_downloaded_queue(|ready,_|{
        assert!(matches!(Resolved::resolve(b,rule(),limits()).unwrap().grant(phy,b,&ready.default_tables(),tuned,owner),Err(Error::Firmware)));
    });
    phy.mark_failed();assert_eq!(phy.state(),State::Failed);
    assert!(matches!(Resolved::resolve(b,rule(),limits()).unwrap().grant(phy,b,tables,tuned,owner),Err(Error::State)));
});}

#[test]
fn selected_countries_with_the_same_power_group_still_have_distinct_policy_snapshots(){
    let mut b=board();b.identity.country=[255;2];b.maps.logical[0x2cb..0x2cd].fill(255);
    let mut de=rule();de.country=*b"DE";let mut fr=rule();fr.country=*b"FR";
    let de=Resolved::resolve(&b,de,limits()).unwrap();let fr=Resolved::resolve(&b,fr,limits()).unwrap();
    assert_ne!(de.policy(),fr.policy());
}
#[test]
fn failed_live_receive_validation_consumes_attempt_without_h2c_or_retry(){with_channel(|phy,b,tables,tuned,owner,resolved,_|{
    use crate::io::RegisterIo;
    let s=phy.bus_between_stages();let submitted=s.submitted();let old=s.read32(crate::fwcmd::DMA_STOP).unwrap();
    s.write32(crate::fwcmd::DMA_STOP,old|1<<18).unwrap();
    assert!(matches!(resolved.grant(phy,b,tables,tuned,owner),Err(Error::Owner(_))));
    assert_eq!(phy.bus_between_stages().submitted(),submitted);
    phy.bus_between_stages().write32(crate::fwcmd::DMA_STOP,old).unwrap();
    assert!(matches!(Resolved::resolve(b,rule(),limits()).unwrap().grant(phy,b,tables,tuned,owner),Err(Error::AlreadyAttempted)));
});}
