use crate::{btc_session::{self,Session,data_mode::{self,DataSession,Stage,Cause}},channel_power::Constraints,
    channel_rfk::{self,Authorities},download,h2c,initial_phy,io,mac,model,pcie,
    phy::{Phy,channel::{self,calibration_policy::{Resolved,PlatformInput}}},phycap,post_mac,power,runtime_mac,rx,station,tx};
#[path="rtw89_initial_phy_device.rs"]mod initial;
#[path="rtw89_station_device.rs"]mod station_device;
#[path="rtw89_data_mode_device.rs"]mod device;
use std::{rc::Rc,cell::RefCell};
use device::{Control,Device,Region};
fn frame()->Vec<u8>{let mut f=vec![0;64];f[0]=8;f[4..10].copy_from_slice(&[2,1,1,1,1,1]);f[10..16].copy_from_slice(&[2,3,4,5,6,7]);f}
fn command<B:btc_session::DeviceIo>(data:&mut DataSession<'_,'_,'_,'_,'_,'_,B>){
    data.command(h2c::Command{category:2,class:10,function:2},&[0;20],h2c::Acks{received:true,done:true},100_000).unwrap();
}
fn run(fault:u8,configure:bool,inbox:bool,check:impl FnOnce(&mut DataSession<'_,'_,'_,'_,'_,'_,Device>,&Rc<RefCell<Control>>)){
    download::tests::with_synthetic_firmware(|verified|{
        let mut raw=model::Device::new(2);let board=model::read_board(&mut raw);let mut region=model::Region::new();let txmem=Region::new();
        let(mut queue,mut rings)=region.pair(&mut raw);let mut tx=unsafe{tx::Queue::new(&mut raw,txmem.base,tx::DMA_BYTES,0x1000000).unwrap()};
        power::on_for_board_read(&mut io::TimedIo::new(&mut raw,1_000_000)).unwrap();mac::prepare_download(&mut raw).unwrap();
        let pcie=pcie::prepare(&mut raw).unwrap();queue.install(&mut raw).unwrap();pcie.finish(&mut raw,&queue).unwrap();
        rings.install(&mut raw).unwrap();tx.install(&mut raw).unwrap();raw.command=7;crate::startup::command_dma(&mut raw).unwrap();unsafe{queue.start(&mut raw).unwrap()};
        let ready=download::run(&mut raw,&mut queue,verified).unwrap();let tables=ready.default_tables();
        let caps=phycap::query(&mut raw,&mut queue,&board,runtime_mac::BAR_BYTES,&ready,phycap::MAX_TIMEOUT_US).unwrap();
        let runtime=runtime_mac::prepare(&mut raw,&mut queue,&board,runtime_mac::BAR_BYTES,&ready).unwrap();
        let receiving=post_mac::activate(&mut raw,&mut queue,&mut rings,&board,runtime_mac::BAR_BYTES,&runtime).unwrap();
        let mut base=initial::Device::new(raw);base.channel=Some(initial::channel::ChannelModel::default());
        let control=Rc::new(RefCell::new(Control::default()));let mut d=Device::new(base,&txmem,control.clone());let mut scratch=vec![0;rx::BUFFER_BYTES];let mut bank=Box::new(btc_session::inbox::Inbox::new());
        {
            let mut session=if inbox{Session::new_with_inbox(&mut d,&mut queue,&mut rings,&receiving,ready,&mut scratch,&mut bank).unwrap()}
                else{Session::new(&mut d,&mut queue,&mut rings,&receiving,ready,&mut scratch).unwrap()};
            let mut phy=Phy::new(&mut session,board.identity);let initial=initial_phy::prepare(&mut phy,&board,&caps).unwrap();
            let resolved=Resolved::resolve(&board,channel::Rule{country:*b"US",version:1,center_mhz:2437,
                access:channel::Access::InitiateRadiation,eirp_half_dbm:40,conducted_half_dbm:34},
                PlatformInput::Limits(Constraints{per_ntx:[30,24],per_path:[30,30]})).unwrap();
            let mut tuned=channel::run(&mut phy,&board,&initial.prefix,&initial.paths,&tables,resolved.policy()).unwrap();
            let[iqk,tssi,dpk]=resolved.grant(&mut phy,&board,&tables,&mut tuned,&initial.owner).unwrap();
            let calibrated=channel_rfk::run(&mut phy,&board,&initial.prefix,&tuned,&initial.owner,Authorities{iqk,tssi,dpk}).unwrap();
            let mut station=station::prepare(&mut phy,&board,&tuned,&calibrated,&initial.owner,&caps).unwrap();
            if configure{let filter=station.filter(true,[0;6],[0;6]);station.configure_filter(&mut phy,&board,&tuned,&initial.owner,filter).unwrap();}
            let prep=data_mode::prepare(&mut phy,&board,&tuned,&calibrated,&initial.owner,station,&tx);
            if !configure||!inbox{assert!(prep.is_err());drop(phy);drop(session);}
            else{
                let prep=prep.unwrap();drop(phy);control.borrow_mut().armed=true;control.borrow_mut().fault=fault;
                if fault==8{control.borrow_mut().old=3;}
                match DataSession::activate(session,&mut tx,prep){Ok(mut data)=>{
                    assert!(!matches!(fault,1..=6|9..=13));check(&mut data,&control);data.shutdown();
                },Err(mut failed)=>{
                    assert!(matches!(fault,1..=6|9..=13),"unexpected {:?}",failed.failure);
                    assert!(failed.failure.shutdown.is_some());let before=failed.owner.shutdown();let after=failed.owner.shutdown();
                    assert_eq!(format!("{before:?}"),format!("{after:?}"));
                }};

            }
        }
        if matches!(fault,1..=6|9..=13){assert_eq!(d.station.base.inner.get(2,1)&3,0);
            assert_eq!(d.scheduler,usize::from(matches!(fault,3|4|6)));assert_eq!(d.shutdowns,1);assert!(d.published.is_empty());}
        assert!(d.published.iter().all(|f|f[10..16]==[2,3,4,5,6,7]));
        rings.stop(&mut d);runtime_mac::disable_radio(&mut d).unwrap();download::stop(&mut d,&mut queue);model::power_off(&mut d.station.base.inner);
    });
}
#[test]
fn cold_source_profile_station_and_single_rx_owner_activate_then_complete_actual_tx(){run(0,true,true,|data,_|{
    command(data);data.begin(&frame(),12,3,100_000).unwrap();let c=data.advance(100_000).unwrap().unwrap();
    assert!(c.acked);assert_eq!(c.completed_descriptors,1);assert_eq!(c.attempts,tx::AttemptCount::Unknown);assert_eq!(c.raw_data_tx_count,1);
    assert_eq!(data.pending_packets(),0);command(data);
});}
#[test]
fn missing_actual_filter_or_inbox_never_activates(){for(filter,inbox)in[(false,true),(true,false)]{run(0,filter,inbox,|_,_|panic!("unreachable"));}}
#[test]
fn ignored_applied_failed_or_late_dma_enable_retains_owner_and_stops_radio(){for fault in[1,2,5]{run(fault,true,true,|_,_|panic!("unreachable"));}}
#[test]
fn wrong_missing_scheduler_reply_or_wrong_ctn_readback_never_exposes_data(){for fault in[3,4,6]{run(fault,true,true,|_,_|panic!("unreachable"));}}
#[test]
fn old_rx_epoch_is_counted_and_discarded_before_first_delivery(){run(8,true,true,|data,_|{assert_eq!(data.counts().previous_epoch,3);assert_eq!(data.pending_packets(),0);command(data);});}
#[test]
fn ordinary_wifi_pressure_does_not_starve_command_ack_or_final_tx_reports(){run(0,true,true,|data,c|{
    data.begin(&frame(),12,1,100_000).unwrap();c.borrow_mut().burst=true;command(data);
    let completion=data.advance(100_000).unwrap().unwrap();assert!(completion.acked);assert_eq!(data.pending_packets(),4);
    assert_eq!(data.counts().pressure,8);assert_eq!(data.counts().filtered,3);assert_eq!(data.counts().invalid,1);
    while let Some(record)=data.packet(){let f=crate::frames::from_packet(record.metadata().packet,record.buffer()).unwrap();assert_eq!(&f.mpdu()[4..10],&[2,3,4,5,6,7]);data.consume_packet();}
    command(data);assert_eq!(data.pending_packets(),0);
});}
#[test]
fn busy_invalid_timeout_and_wrong_transmitter_are_nonterminal_admission_errors(){run(0,true,true,|data,c|{
    let mut wrong=frame();wrong[10]^=2;let e=data.begin(&wrong,12,1,100_000).unwrap_err();assert!(e.shutdown.is_none());
    assert!(data.poll_packets(0).unwrap_err().shutdown.is_none());assert!(data.advance(100_001).unwrap_err().shutdown.is_none());
    assert_eq!(c.borrow().published,0);data.begin(&frame(),12,1,100_000).unwrap();assert!(matches!(data.begin(&frame(),12,1,100_000).unwrap_err().cause,Cause::Tx(tx::Error::Busy)));
    assert_eq!(c.borrow().published,1);assert!(data.advance(100_000).unwrap().unwrap().acked);command(data);
});}
#[test]
fn dma_and_rpq_without_final_c2h_are_never_success(){run(7,true,true,|data,c|{
    data.begin(&frame(),12,1,10_000).unwrap();assert!(data.advance(10_000).unwrap().is_none());
    c.borrow_mut().advance_us=10_001;assert!(data.advance(10_000).is_err());
    assert!(data.advance(10_000).is_err());
});}
#[test]
fn changed_owned_tx_layout_invalidates_active_mode(){run(0,true,true,|data,c|{c.borrow_mut().corrupt=true;
    let e=data.poll_packets(10_000).unwrap_err();assert_eq!(e.stage,Stage::Runtime);assert!(e.shutdown.is_some());assert!(data.advance(10_000).is_err());
});}

#[test]
fn changed_bb_reset_afc_cycle_and_channel_are_rejected_before_dma(){for fault in 9..=13{run(fault,true,true,|_,_|panic!("unreachable"));}}

#[test]
fn active_radio_tamper_is_rejected_before_any_tx_descriptor(){for fault in 9..=13{run(0,true,true,|data,c|{
    c.borrow_mut().fault=fault;assert!(data.begin(&frame(),12,1,100_000).is_err());assert_eq!(c.borrow().published,0);
});}}
#[test]
fn foreign_report_handler_failure_keeps_actual_consumed_copy(){run(14,true,true,|data,_|{
    data.begin(&frame(),12,1,100_000).unwrap();assert!(data.advance(100_000).is_err());
    let record=data.terminal_packet().expect("consumed report remains inspectable");assert_eq!(record.metadata().channel,rx::Channel::Rpq);
});}
#[test]
fn control_overflow_stays_terminal_and_retains_fifth_record(){run(0,true,true,|data,c|{
    c.borrow_mut().unknown=true;assert!(data.command(h2c::Command{category:2,class:10,function:2},&[0;20],
        h2c::Acks{received:true,done:true},100_000).is_err());assert_eq!(data.pending_packets(),4);
    assert_eq!(data.terminal_packet().unwrap().bytes()[8],4);assert_eq!(data.counts().pressure,0);
});}
