use crate::{btc_session::Session,channel_power::Constraints,channel_rfk::{self,Authorities},download,fwcmd,
    initial_phy,io::{self,RegisterIo},model,phy::{Phy,State,channel::{self,CalibrationEngine as Engine,CalibrationTransmit}},
    phycap,post_mac,runtime_mac,rx,station::{self,Stage}};
#[path="rtw89_initial_phy_device.rs"]mod initial;
#[path="rtw89_station_device.rs"]mod device;
use device::Device;
use initial::channel::ChannelModel;

fn run(speed:u8,setup:impl FnOnce(&mut Device),tamper:bool,repeat:bool,
    check:impl FnOnce(&Device,Result<station::Prepared,station::Failure>,State)){
    run_validation(speed,setup,tamper,repeat,0,check);
}
fn run_validation(speed:u8,setup:impl FnOnce(&mut Device),tamper:bool,repeat:bool,validation:u8,
    check:impl FnOnce(&Device,Result<station::Prepared,station::Failure>,State)){
    download::tests::with_synthetic_firmware(|verified|{
        let mut raw=model::Device::new(speed);let board=model::read_board(&mut raw);
        let mut region=model::Region::new();let(mut queue,mut rings)=region.pair(&mut raw);
        model::prepare_receive(&mut raw,&mut queue,&mut rings).unwrap();
        let ready=download::run(&mut raw,&mut queue,verified).unwrap();let tables=ready.default_tables();
        let caps=phycap::query(&mut raw,&mut queue,&board,runtime_mac::BAR_BYTES,&ready,phycap::MAX_TIMEOUT_US).unwrap();
        let runtime=runtime_mac::prepare(&mut raw,&mut queue,&board,runtime_mac::BAR_BYTES,&ready).unwrap();
        let receiving=post_mac::activate(&mut raw,&mut queue,&mut rings,&board,runtime_mac::BAR_BYTES,&runtime).unwrap();
        let mut base=initial::Device::new(raw);base.channel=Some(ChannelModel::default());let mut d=Device::new(base);setup(&mut d);
        let mut scratch=vec![0;rx::BUFFER_BYTES];
        let(result,state)={
            let mut session=Session::new(&mut d,&mut queue,&mut rings,&receiving,ready,&mut scratch).unwrap();
            let mut phy=Phy::new(&mut session,board.identity);let initial=initial_phy::prepare(&mut phy,&board,&caps).unwrap();
            let policy=channel::Policy::resolve(&board,channel::Rule{country:*b"US",version:1,center_mhz:2437,
                access:channel::Access::InitiateRadiation,eirp_half_dbm:40,conducted_half_dbm:34},
                channel::Platform::Resolved(Constraints{per_ntx:[30,24],per_path:[30,30]})).unwrap();
            let mut tuned=channel::run(&mut phy,&board,&initial.prefix,&initial.paths,&tables,&policy).unwrap();
            let permissions=Authorities{iqk:CalibrationTransmit::modeled(&tuned,Engine::Iqk).unwrap(),
                tssi:CalibrationTransmit::modeled(&tuned,Engine::Tssi).unwrap(),dpk:CalibrationTransmit::modeled(&tuned,Engine::Dpk).unwrap()};
            let calibrated=channel_rfk::run(&mut phy,&board,&initial.prefix,&tuned,&initial.owner,permissions).unwrap();
            if tamper{tuned.policy_version+=1;}
            let mut result=station::prepare(&mut phy,&board,&tuned,&calibrated,&initial.owner,&caps);
            if let Ok(prepared)=&result{assert!(prepared.matches(&tuned,board.identity.mac));assert_eq!(prepared.mac_id(),0);
                assert!(!prepared.matches(&tuned,[2,3,4,5,6,8]));}
            if (1..10).contains(&validation){
                let prepared=result.as_ref().unwrap();
                if validation==2||validation==4{
                    let session=phy.bus_between_stages();let deadline=session.now_us()+10_000;
                    session.registers(deadline,|bus|{
                        if validation==2{bus.write32(0xc04,0x18840000)?;io::modify32(bus,0x40000,1<<21,0)?;bus.write32(0xc04,0x18888000)?;}
                        else{io::modify32(bus,0xc660,0x30,0)?;}Ok(())
                    }).unwrap();
                }
                if validation==3{tuned.policy_version+=1;}
                let checked=prepared.validate(&mut phy,&board,&tuned,&initial.owner,if validation==5{1}else{10_000});
                if let Err(error)=checked{result=Err(error);}
            }
            if validation>=10{
                let prepared=result.as_mut().unwrap();assert!(prepared.current_filter().is_none());
                let selected=validation==11;
                let filter=prepared.filter(!selected,if selected{[2,8,7,6,5,4]}else{[0;6]},
                    if selected{[2,10,11,12,13,14]}else{[0;6]});
                if validation==15{tuned.policy_version+=1;}
                let checked=prepared.configure_filter(&mut phy,&board,&tuned,&initial.owner,filter);
                if let Err(error)=checked{assert!(prepared.current_filter().is_none());result=Err(error);}
                else{
                    assert_eq!(prepared.current_filter(),Some(filter));
                    if validation==12{let before=phy.bus_between_stages().submitted();
                        assert_eq!(prepared.configure_filter(&mut phy,&board,&tuned,&initial.owner,filter).unwrap_err().stage,Stage::Guard);
                        assert_eq!(phy.bus_between_stages().submitted(),before);}
                    if validation==13{let session=phy.bus_between_stages();let deadline=session.now_us()+10_000;
                        session.registers(deadline,|bus|io::modify32(bus,0xce20,0,1<<11)).unwrap();}
                    let checked=prepared.validate(&mut phy,&board,&tuned,&initial.owner,10_000);
                    if let Err(error)=checked{result=Err(error);}
                }
            }
            if repeat{assert!(result.is_ok());let before=phy.bus_between_stages().submitted();
                assert_eq!(station::prepare(&mut phy,&board,&tuned,&calibrated,&initial.owner,&caps).unwrap_err().stage,Stage::Guard);
                assert_eq!(phy.bus_between_stages().submitted(),before);}
            (result,phy.state())
        };
        check(&d,result,state);
        assert_eq!(d.base.inner.get(0xc348,4)&0xffff,0);assert_eq!(d.base.inner.get(fwcmd::DMA_STOP,4)&0xb0f00,0xb0f00);
        assert_eq!(rings.stop(&mut d),rx::Stop{request:None,hci:None,idle:None});
        runtime_mac::disable_radio(&mut d).unwrap();assert_eq!(download::stop(&mut d,&mut queue),download::Stop{dma:None,cpu:None});
        model::power_off(&mut d.base.inner);
    });
}
fn headers(d:&Device)->Vec<(u8,u8,u8)>{d.station_packets.iter().map(|p|(p[0]&3,p[0]>>2,p[1])).collect()}
fn words(p:&[u8])->Vec<u32>{p[8..].chunks_exact(4).map(|b|u32::from_le_bytes(b.try_into().unwrap())).collect()}
fn radio_off(d:&Device){assert_eq!(d.base.inner.get(2,1)&3,0);assert_eq!(d.base.inner.get(0x2f0,4)&(1<<17),0);}
#[test]
fn actual_cold_phy_channel_rfk_then_station_uses_source_no_link_cam_and_control(){for speed in [1,2]{run(speed,|d|d.base.inner.put(0x9eb0,4,1),false,false,|d,result,state|{
    result.unwrap();assert_eq!(state,State::ChannelTuned);assert_eq!(headers(d),[(1,9,8),(1,8,4),(1,8,0),(1,6,0),(1,5,2)]);
    assert_eq!(words(&d.station_packets[0]),[0,0,0,0,1,0,0,0]);
    assert_eq!(words(&d.station_packets[1]),[0x2000]);assert_eq!(words(&d.station_packets[2]),[0x4000100]);
    assert_eq!(words(&d.station_packets[3]),[0,0x400000,0x10001,0,0x05040302,0x706,0,0,0,0x20000,0,0,0x80000,0xfd,0]);
    let ctl=words(&d.station_packets[4]);assert_eq!(ctl,[0x80,0x200000,0,0,0,0,0x430000,0,0,0x200000,0,0,0,0xe00,0xffff0000,0xf0c0000,0]);
    assert_eq!(d.window_writes,12);assert_eq!(d.memory[&0x18840000]&0x200000,0x200000);
    assert_eq!(d.selector,0x18888000);assert_eq!(d.base.inner.get(0x9eb0,4)&1,0);
    for(i,p)in d.station_packets.iter().enumerate(){let flags=u32::from_le_bytes(p[4..8].try_into().unwrap());
        assert_eq!(flags&0x8000,if i==0{0}else{0x8000});
        assert_eq!(flags&0x4000,if i==0||p[3]%4==0{0x4000}else{0});}
    assert_eq!(d.base.inner.get(0xc400,4)&0x13e1f,4);assert_eq!(d.base.inner.get(0xc414,4)&0xffff,100);
    assert_eq!(d.base.inner.get(0xc40c,4)&0xfff,160);assert_eq!(d.base.inner.get(0xc660,4)&0x30,0x10);
    for(i,p)in d.base.inner.normal_packets.iter().enumerate(){assert_eq!(p[3],i as u8);}
});}}
#[test]
fn granted_bar_must_include_actual_indirect_aperture_before_any_station_write(){run(1,|d|d.bar=0x20000,false,false,|d,result,state|{
    assert_eq!(result.unwrap_err().stage,Stage::Guard);assert_eq!(state,State::ChannelTuned);assert_eq!(d.window_writes,0);assert!(d.station_packets.is_empty());
});}
#[test]
fn changed_policy_cannot_reuse_completed_calibration_for_station(){run(1,|_|{},true,false,|d,result,_|{
    assert_eq!(result.unwrap_err().stage,Stage::Guard);assert_eq!(d.window_writes,0);assert!(d.station_packets.is_empty());
});}
#[test]
fn initial_context_cannot_be_created_twice_over_active_port(){run(2,|_|{},false,true,|d,result,_|{
    result.unwrap();assert_eq!(d.station_packets.len(),5);assert_eq!(d.window_writes,12);
});}
#[test]
fn firmware_owned_unmasked_control_fields_do_not_invalidate_written_fields(){run(2,|d|d.unrelated_firmware_fields=true,false,false,|d,result,_|{
    result.unwrap();assert_eq!(d.memory[&0x18800000],0x1234);assert_eq!(d.memory[&0x18840008],0x5678);
});}
#[test]
fn ignored_cctl_mask_write_is_detected_despite_firmware_done_ack(){run(1,|d|d.ignore_control=true,false,false,|d,result,state|{
    let e=result.unwrap_err();assert_eq!(e.stage,Stage::Verify);assert_eq!(state,State::Failed);assert!(e.radio_stop.is_none());radio_off(d);
    assert_eq!(d.station_packets.len(),5);assert_eq!(d.base.inner.get(0xc400,4)&4,0);
});}
#[test]
fn missing_role_dma_completion_is_not_retried_or_followed_by_join(){run(1,|d|d.base.inner.hold_header=Some((1,8,4)),false,false,|d,result,state|{
    let e=result.unwrap_err();assert_eq!(e.stage,Stage::Role);assert_eq!(state,State::Failed);radio_off(d);
    assert_eq!(headers(d),[(1,9,8),(1,8,4)]);assert!(e.selector_restore.is_none());assert_eq!(d.selector,0x18888000);
});}
#[test]
fn ignored_and_applied_failed_port_writes_stop_before_indirect_tables(){for applied in [false,true]{run(1,|d|{
    d.ignore_port=!applied;d.fail_port=applied;
},false,false,|d,result,state|{
    assert_eq!(result.unwrap_err().stage,Stage::Port);assert_eq!(state,State::Failed);assert_eq!(d.window_writes,0);radio_off(d);
});}}
#[test]
fn late_final_command_cannot_publish_station_ready(){run(2,|d|d.late_control=true,false,false,|d,result,state|{
    assert!(matches!(result.unwrap_err().stage,Stage::Control|Stage::Verify));assert_eq!(state,State::Failed);radio_off(d);
});}
#[test]
fn selector_restoration_failure_is_visible_and_radio_is_stopped(){run(1,|d|d.restore_fail=true,false,false,|d,result,state|{
    let e=result.unwrap_err();assert_eq!(e.stage,Stage::Verify);assert!(e.selector_restore.is_some());assert_eq!(state,State::Failed);radio_off(d);
});}

#[test]
fn firmware_receipt_without_macid_unpause_is_not_station_ready(){run(1,|d|{
    d.base.inner.put(0x9eb0,4,1);d.ignore_unpause=true;
},false,false,|d,result,state|{
    assert_eq!(result.unwrap_err().stage,Stage::Verify);assert_eq!(state,State::Failed);
    assert_eq!(d.base.inner.get(0x9eb0,4)&1,1);radio_off(d);
});}

#[test]
fn current_station_validates_before_consuming_into_data_activation(){run_validation(2,|_|{},false,false,1,|d,result,state|{
    result.unwrap();assert_eq!(state,State::ChannelTuned);assert_eq!(d.station_packets.len(),5);assert_eq!(d.selector,0x18888000);
});}
#[test]
fn changed_cctl_or_report_path_cannot_reuse_station_readiness(){for fault in [2,4]{run_validation(1,|_|{},false,false,fault,|d,result,state|{
    assert_eq!(result.unwrap_err().stage,Stage::Verify);assert_eq!(state,State::Failed);radio_off(d);
    assert_eq!(d.station_packets.len(),5);assert_eq!(d.selector,0x18888000);
});}}
#[test]
fn changed_policy_is_rejected_before_validation_register_checks(){run_validation(1,|_|{},false,false,3,|d,result,state|{
    assert_eq!(result.unwrap_err().stage,Stage::Guard);assert_eq!(state,State::ChannelTuned);assert_eq!(d.station_packets.len(),5);
});}
#[test]
fn station_validation_budget_includes_owner_and_final_selector_checks(){for final_check in [false,true]{run_validation(1,|d|{
    if final_check{d.late_restore=2;}
},false,false,if final_check{6}else{5},|d,result,state|{
    assert_eq!(result.unwrap_err().stage,if final_check{Stage::Verify}else{Stage::Guard});assert_eq!(state,State::Failed);radio_off(d);
    assert_eq!(d.station_packets.len(),5);
});}}

#[test]
fn initial_scan_filter_requires_cam_done_and_preserves_mpdu_limit(){run_validation(2,|_|{},false,false,10,|d,result,state|{
    let ready=result.unwrap();let filter=ready.current_filter().unwrap();assert!(filter.all_beacons());assert_eq!(filter.own(),[2,3,4,5,6,7]);
    assert_eq!(filter.bssid(),[0;6]);assert_eq!(state,State::ChannelTuned);assert_eq!(d.station_packets.len(),6);
    assert_eq!(words(&d.station_packets[5]),words(&d.station_packets[3]));
    assert_eq!(d.base.inner.get(0xce20,4),0x03164400);
    assert_ne!(u32::from_le_bytes(d.station_packets[5][4..8].try_into().unwrap())&0x8000,0);
});}
#[test]
fn selected_bssid_and_override_mac_pack_source_cam_fields(){run_validation(1,|_|{},false,false,11,|d,result,state|{
    let filter=result.unwrap().current_filter().unwrap();assert!(!filter.all_beacons());assert_eq!(filter.own(),[2,10,11,12,13,14]);
    assert_eq!(filter.bssid(),[2,8,7,6,5,4]);assert_eq!(state,State::ChannelTuned);
    assert_eq!(words(&d.station_packets[5]),[0,0x400000,0x0a0c0001,0,0x0c0b0a02,0x08020e0d,0x04050607,0,0,0x20000,0,0,0x80000,0x080200fd,0x04050607]);
});}
#[test]
fn initial_filter_cannot_be_replaced_without_runtime_epoch_transition(){run_validation(1,|_|{},false,false,12,|d,result,state|{
    result.unwrap();assert_eq!(state,State::ChannelTuned);assert_eq!(d.station_packets.len(),6);
});}
#[test]
fn changed_hardware_filter_invalidates_prepared_context(){run_validation(1,|_|{},false,false,13,|d,result,state|{
    assert_eq!(result.unwrap_err().stage,Stage::Verify);assert_eq!(state,State::Failed);radio_off(d);
});}
#[test]
fn missing_cam_ack_or_dma_completion_never_publishes_filter(){for fault in [1,2]{run_validation(1,|d|d.filter_command_fault=fault,false,false,14,|d,result,state|{
    assert_eq!(result.unwrap_err().stage,Stage::Cam);assert_eq!(state,State::Failed);radio_off(d);
    assert_eq!(d.station_packets.len(),6);assert_ne!(d.base.inner.get(0xce20,4)&0xffc0ffff,0x03004400);
});}}
#[test]
fn changed_policy_refuses_filter_before_second_cam_command(){run_validation(1,|_|{},false,false,15,|d,result,state|{
    assert_eq!(result.unwrap_err().stage,Stage::Guard);assert_eq!(state,State::ChannelTuned);assert_eq!(d.station_packets.len(),5);
});}
#[test]
fn ignored_applied_failed_or_late_filter_write_stops_radio(){for fault in 0..3{run_validation(1,|d|{
    d.ignore_filter=fault==0;d.fail_filter=fault==1;d.late_filter=fault==2;
},false,false,16,|d,result,state|{
    assert_eq!(result.unwrap_err().stage,Stage::Filter);assert_eq!(state,State::Failed);radio_off(d);assert_eq!(d.station_packets.len(),6);
});}}
