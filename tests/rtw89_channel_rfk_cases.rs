use crate::{btc_context, btc_session::Session, channel_power::Constraints, channel_rfk::{self, Authorities, Cause, Stage},
    dack::Band, download, fwcmd, model, phy::{self, Phy, State, channel::{self, CalibrationEngine as Engine, CalibrationTransmit}},
    phycap, post_mac, runtime_mac, rx};
#[path="rtw89_initial_phy_device.rs"] mod device;
use device::{Device, channel::ChannelModel};

fn run(speed:u8, setup:impl FnOnce(&mut Device), invalid:u8,
    check:impl FnOnce(&Device,Result<channel_rfk::Completed,channel_rfk::Failure>,State)){
    download::tests::with_synthetic_firmware(|verified|{
        let mut raw=model::Device::new(speed);let board=model::read_board(&mut raw);
        let mut region=model::Region::new();let(mut queue,mut rings)=region.pair(&mut raw);
        model::prepare_receive(&mut raw,&mut queue,&mut rings).unwrap();
        let ready=download::run(&mut raw,&mut queue,verified).unwrap();let tables=ready.default_tables();
        let caps=phycap::query(&mut raw,&mut queue,&board,runtime_mac::BAR_BYTES,&ready,phycap::MAX_TIMEOUT_US).unwrap();
        let runtime=runtime_mac::prepare(&mut raw,&mut queue,&board,runtime_mac::BAR_BYTES,&ready).unwrap();
        let receiving=post_mac::activate(&mut raw,&mut queue,&mut rings,&board,runtime_mac::BAR_BYTES,&runtime).unwrap();
        let mut d=Device::new(raw);d.channel=Some(ChannelModel::default());setup(&mut d);
        let mut scratch=vec![0;rx::BUFFER_BYTES];
        let(result,state)={
            let mut session=Session::new(&mut d,&mut queue,&mut rings,&receiving,ready,&mut scratch).unwrap();
            let mut phy=Phy::new(&mut session,board.identity);
            phy.baseband().unwrap();phy.radio().unwrap();let owner=btc_context::initialize(&mut phy,&board).unwrap();
            let prefix=phy::dm::run(&mut phy,Band::Ghz2).unwrap();phy.nctl().unwrap();phy.rck().unwrap();phy.dack(Band::Ghz2).unwrap();phy.rxdck(Band::Ghz2).unwrap();
            let configured=phy::tail::run(&mut phy,&board,&prefix,&caps,phy::tail::Subband::Ghz2).unwrap();
            let policy=channel::Policy::resolve(&board,channel::Rule{country:*b"US",version:1,center_mhz:2437,
                access:channel::Access::InitiateRadiation,eirp_half_dbm:40,conducted_half_dbm:34},
                channel::Platform::Resolved(Constraints{per_ntx:[30,24],per_path:[30,30]})).unwrap();
            let mut tuned=channel::run(&mut phy,&board,&prefix,&configured,&tables,&policy).unwrap();
            let permissions=Authorities{iqk:CalibrationTransmit::modeled(&tuned,Engine::Iqk).unwrap(),
                tssi:CalibrationTransmit::modeled(&tuned,if invalid==1{Engine::Dpk}else{Engine::Tssi}).unwrap(),
                dpk:CalibrationTransmit::modeled(&tuned,Engine::Dpk).unwrap()};
            if invalid==2{tuned.policy_version+=1;}if invalid==3{tuned.access=channel::Access::PassiveOnly;}
            let result=channel_rfk::run(&mut phy,&board,&prefix,&tuned,&owner,permissions);
            if let Ok(done)=&result{assert!(done.matches(&tuned));}
            (result,phy.state())
        };
        check(&d,result,state);
        assert_eq!(d.inner.get(0xc348,4)&0xffff,0);assert_eq!(d.inner.get(fwcmd::DMA_STOP,4)&0xb0f00,0xb0f00);
        let stop=rings.stop(&mut d);assert_eq!((stop.request,stop.hci,stop.idle),(None,None,None));
        runtime_mac::disable_radio(&mut d).unwrap();assert_eq!(download::stop(&mut d,&mut queue),download::Stop{dma:None,cpu:None});
        model::power_off(&mut d.inner);assert_eq!(d.inner.command,0);
    });
}

fn headers(d:&Device)->Vec<(u8,u8,u8)>{d.inner.normal_packets.iter().map(|p|(p[0]&3,p[0]>>2,p[1])).collect()}
fn radio_off(d:&Device){assert_eq!(d.inner.get(2,1)&3,0);assert_eq!(d.inner.get(0x2f0,4)&(1<<17),0);}

#[test]
fn one_cold_device_runs_channel_rxdck_iqk_tssi_dpk_and_final_mcc_in_order(){for speed in [1,2]{run(speed, |_|{},0,|d,result,state|{
    let done=result.unwrap();assert_eq!(state,State::ChannelTuned);assert_eq!(done.tssi.reports,[[160,100],[200,142]]);
    assert_eq!(done.dpk.data.paths.map(|p|p.txagc),[0x35;2]);let model=d.channel.as_ref().unwrap();
    assert_eq!(model.commands.len(),40);assert_eq!(model.commands[0].0,0x119);assert_eq!(model.commands[25].0,0xe29);
    assert_eq!(model.commands[26].0,0x1919);assert_eq!(model.commands[39].0,0x1129);
    assert_eq!(model.shots.iter().map(|&(p,power,count,period,_)|(p,power,count,period)).collect::<Vec<_>>(),
        [(0,48,100,5000),(0,20,100,5000),(1,48,100,5000),(1,20,100,5000)]);
    assert!(model.commands[25].1<model.shots[0].4&&model.shots[3].4<model.commands[26].1);
    let packet=d.inner.normal_packets.last().unwrap();assert_eq!(headers(d).last(),Some(&(2,10,2)));
    assert_eq!(&packet[8..],&[6u32,6,0,0,6].into_iter().flat_map(u32::to_le_bytes).collect::<Vec<_>>());
    for(index,p)in d.inner.normal_packets.iter().enumerate(){assert_eq!(p[3],index as u8);}
    assert!(d.inner.now<15_000_000);
});}}
#[test]
fn all_three_policy_bound_authorities_are_checked_before_any_rfk_or_mcc(){for invalid in 1..=3{run(2, |_|{},invalid,|d,result,state|{
    assert_eq!(result.unwrap_err().stage,Stage::Guard);assert_eq!(state,State::ChannelTuned);
    assert!(d.channel.as_ref().unwrap().commands.is_empty()&&d.channel.as_ref().unwrap().shots.is_empty());
    assert!(!headers(d).contains(&(2,10,2)));
});}}
#[test]
fn unknown_iqk_completion_prevents_tssi_dpk_and_mcc(){run(1,|d|d.channel.as_mut().unwrap().stuck_command=Some(1),0,|d,result,state|{
    let failure=result.unwrap_err();assert_eq!(failure.stage,Stage::Iqk);assert!(matches!(failure.cause,Cause::Iqk(_)));
    assert_eq!(state,State::Failed);assert_eq!(d.channel.as_ref().unwrap().commands.len(),1);assert!(d.channel.as_ref().unwrap().shots.is_empty());
    assert!(!headers(d).contains(&(2,10,2)));radio_off(d);
});}
#[test]
fn missing_tssi_report_prevents_dpk_and_mcc(){run(2,|d|d.channel.as_mut().unwrap().stuck_tssi=true,0,|d,result,state|{
    let failure=result.unwrap_err();assert_eq!(failure.stage,Stage::Tssi);assert!(matches!(failure.cause,Cause::Tssi(_)));
    assert_eq!(state,State::Failed);assert_eq!(d.channel.as_ref().unwrap().commands.len(),26);assert_eq!(d.channel.as_ref().unwrap().shots.len(),1);
    assert!(!headers(d).contains(&(2,10,2)));radio_off(d);
});}
#[test]
fn bad_dpk_quality_keeps_completed_tssi_but_never_publishes_channel_success(){run(2,|d|d.channel.as_mut().unwrap().bad_dpk=true,0,|d,result,state|{
    let failure=result.unwrap_err();assert_eq!(failure.stage,Stage::Dpk);assert!(matches!(failure.cause,Cause::Dpk(_)));
    assert_eq!(state,State::Failed);assert_eq!(d.channel.as_ref().unwrap().commands.len(),30);assert_eq!(d.channel.as_ref().unwrap().shots.len(),4);
    assert!(!headers(d).contains(&(2,10,2)));radio_off(d);
});}
#[test]
fn mcc_dma_timeout_is_not_retried_or_relabelled_as_ready(){run(1,|d|d.inner.hold_header=Some((2,10,2)),0,|d,result,state|{
    let failure=result.unwrap_err();assert_eq!(failure.stage,Stage::Mcc);assert!(matches!(failure.cause,Cause::Owner(_)));
    assert_eq!(state,State::Failed);assert_eq!(headers(d).iter().filter(|&&h|h==(2,10,2)).count(),1);radio_off(d);
});}
#[test]
fn actual_a2dp_event_preserves_bluetooth_time_between_all_three_engines(){run(2,|d|{
    d.bt=2|1<<30;let mut event=Vec::new();event.extend_from_slice(&(2u32|0x12<<2|1<<8).to_le_bytes());
    event.extend_from_slice(&16u32.to_le_bytes());event.extend_from_slice(&[0,6,0x40,0,0,0,0,0]);d.inner.inject_c2h(&event);
},0,|d,result,_|{
    result.unwrap();let model=d.channel.as_ref().unwrap();
    assert!(model.shots[0].4-model.commands[25].1>=30_000);
    assert!(model.commands[26].1-model.shots[3].4>=30_000);
    assert_eq!(headers(d).last(),Some(&(2,10,2)));
});}
