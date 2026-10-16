use crate::{btc_session::{self,connection,Session,data_mode::{DataSession,Shutdown}},channel_power::Constraints,
    download, io, mac, model, pcie, phy::channel::{self,calibration_policy::{Resolved,PlatformInput}},
    phycap, post_mac, power, runtime_mac, rx, tx};
#[path="rtw89_initial_phy_device.rs"]mod initial;
#[path="rtw89_station_device.rs"]mod station_device;
#[path="rtw89_data_mode_device.rs"]mod device;
use std::{cell::RefCell,rc::Rc};
use device::{Control,Device,Region};

fn frame()->Vec<u8>{let mut f=vec![0;64];f[0]=8;f[4..10].copy_from_slice(&[2,1,1,1,1,1]);f[10..16].copy_from_slice(&[2,3,4,5,6,7]);f}

fn run(fault:u8,missing_ack:bool,check:impl FnOnce(&mut DataSession<'_,'_,'_,'_,'_,'_,Device>)->Result<u32,u32>) {
    download::tests::with_synthetic_firmware(|verified| {
        let mut raw=model::Device::new(2);let board=model::read_board(&mut raw);
        let resolved=Resolved::resolve(&board,channel::Rule{country:*b"US",version:1,center_mhz:2437,
            access:channel::Access::InitiateRadiation,eirp_half_dbm:40,conducted_half_dbm:34},
            PlatformInput::Limits(Constraints{per_ntx:[30,24],per_path:[30,30]})).unwrap();
        let mut region=model::Region::new();let txmem=Region::new();let(mut queue,mut rings)=region.pair(&mut raw);
        let mut transmit=unsafe{tx::Queue::new(&mut raw,txmem.base,tx::DMA_BYTES,0x1000000).unwrap()};
        power::on_for_board_read(&mut io::TimedIo::new(&mut raw,1_000_000)).unwrap();mac::prepare_download(&mut raw).unwrap();
        let pcie=pcie::prepare(&mut raw).unwrap();queue.install(&mut raw).unwrap();pcie.finish(&mut raw,&queue).unwrap();
        rings.install(&mut raw).unwrap();transmit.install(&mut raw).unwrap();raw.command=7;
        crate::startup::command_dma(&mut raw).unwrap();unsafe{queue.start(&mut raw).unwrap()};
        let ready=download::run(&mut raw,&mut queue,verified).unwrap();let tables=ready.default_tables();
        let caps=phycap::query(&mut raw,&mut queue,&board,runtime_mac::BAR_BYTES,&ready,phycap::MAX_TIMEOUT_US).unwrap();
        let runtime=runtime_mac::prepare(&mut raw,&mut queue,&board,runtime_mac::BAR_BYTES,&ready).unwrap();
        let receiving=post_mac::activate(&mut raw,&mut queue,&mut rings,&board,runtime_mac::BAR_BYTES,&runtime).unwrap();
        if missing_ack{raw.hold_header=Some((2,8,0));}
        let mut base=initial::Device::new(raw);base.channel=Some(initial::channel::ChannelModel::default());
        let control=Rc::new(RefCell::new(Control{armed:true,fault,..Control::default()}));
        let mut bus=Device::new(base,&txmem,control.clone());let mut scratch=vec![0;rx::BUFFER_BYTES];
        let mut bank=Box::new(btc_session::inbox::Inbox::new());let mut called=false;
        let session=Session::new_with_inbox(&mut bus,&mut queue,&mut rings,&receiving,ready,&mut scratch,&mut bank).unwrap();
        let result=connection::run(session,&mut transmit,&board,&caps,&tables,resolved,|data|{called=true;check(data)});
        if missing_ack{assert!(matches!(result,Err(connection::Failure::Initial(_))));assert!(!called);assert!(bus.published.is_empty());}
        else if fault!=0{assert!(matches!(result,Err(connection::Failure::Activation(_))));assert!(!called);assert!(bus.published.is_empty());}
        else{let done=result.unwrap();assert!(called);assert!(done.initial.submitted>0);assert_eq!(done.initial.encoded_rts,0x02ff);
            assert_eq!(bus.station.base.inner.get(0x9c18,4)&3,1);
            assert!(matches!(done.shutdown,Shutdown::Attempted{radio:None,transmit:tx::Stop{request:None,idle:None}}));
            assert!(matches!(done.value,Ok(42)|Err(73)));assert_eq!(bus.shutdowns,1);}
        let stop=transmit.stop(&mut bus);assert!(stop.request.is_none()&&stop.idle.is_none());
        rings.stop(&mut bus);runtime_mac::disable_radio(&mut bus).unwrap();download::stop(&mut bus,&mut queue);
        model::power_off(&mut bus.station.base.inner);
    });
}

#[test]
fn real_connection_chain_reaches_callback_and_reports_actual_tx_then_stops() {
    run(0,false,|data|{data.begin(&frame(),12,3,100_000).unwrap();let sent=data.advance(100_000).unwrap().unwrap();
        assert!(sent.acked);assert_eq!(sent.attempts,tx::AttemptCount::Known(1));Ok(42)});
}

#[test]
fn callback_error_value_still_runs_packet_shutdown_and_retains_dma() { run(0,false,|_|Err(73)); }

#[test]
fn missing_actual_initial_ack_never_invokes_service_or_sends_packets() { run(0,true,|_|panic!("service before admission")); }

#[test]
fn refused_actual_data_activation_never_invokes_service() { run(1,false,|_|panic!("service before activation")); }
