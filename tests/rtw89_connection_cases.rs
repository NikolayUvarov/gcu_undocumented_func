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

#[cfg(rtw89_service)]
#[test]
fn actual_connection_serves_wire_info_filter_and_transmit_before_retained_shutdown(){
    use crate::{codec::{self,Reader,Wire,Writer},service::Service,service_ipc::ipc,wlanhw,wire};
    run(0,false,|data|{
        ipc::reset();let mut service=Service::new(data).unwrap();
        let info=ipc::buffer(1,&[]);assert!(service.step());let(words,bytes)=ipc::response(info).unwrap();
        assert_eq!(words[0]&255,wire::STATUS_OK);let mut r=Reader::new(&bytes[..words[0]>>16]);
        let actual=wlanhw::Info::decode(&mut r).unwrap();assert!(r.done());
        assert_eq!(actual.mac,0x020304050607);assert_eq!(actual.channels.as_slice(),&[6]);assert_eq!(actual.channel,6);
        let mut filter=[0;17];let mut w=Writer::new(&mut filter);
        wlanhw::Filter{all_beacons:false,bssid:0x020101010101,own:0}.encode(&mut w).unwrap();
        let change=ipc::buffer(3,&filter);assert!(service.step());assert_eq!(ipc::response(change).unwrap().0,[0,0]);
        let mut body=[0;wlanhw::REQUEST_MAX];let mut w=Writer::new(&mut body);
        codec::encode_bytes::<2346>(&frame(),&mut w).unwrap();12u8.encode(&mut w).unwrap();1u8.encode(&mut w).unwrap();let length=w.len();
        let sent=ipc::buffer(4,&body[..length]);assert!(service.step());assert!(ipc::response(sent).is_none());
        assert!(service.step());let(words,bytes)=ipc::response(sent).unwrap();assert_eq!(words,[2<<16,0]);
        let mut r=Reader::new(&bytes[..2]);assert_eq!(wlanhw::Sent::decode(&mut r).unwrap(),wlanhw::Sent{acked:true,attempts:1});assert!(r.done());
        let stopped=service.stop();assert!(stopped.failure.is_none());assert!(!stopped.owner.current_filter().all_beacons());
        assert!(matches!(stopped.shutdown,Shutdown::Attempted{radio:None,transmit:tx::Stop{request:None,idle:None}}));
        for id in[info,change,sent]{assert_eq!(ipc::dropped(id),1);}assert_eq!(ipc::outstanding(),0);assert_eq!(ipc::duplicates(),0);
        Ok(42)
    });
}

// The serving loop's passive tune for an Ask::Tune, if one was asked; the channel now tuned.
#[cfg(rtw89_service)]
fn service_channel<B:crate::btc_session::DeviceIo>(service:&mut crate::receive_service::ReceiveOnly,
    phy:&mut crate::phy::Phy<'_,Session<'_,'_,'_,B>>,board:&crate::board::Board,receive:&crate::btc_session::listen::Receiving,
    tables:&download::DefaultTables,ch:u8)->u8{
    use crate::{phy::channel::Policy,receive_service::Ask,wlanhw};
    match service.take_ask(){
        Some(Ask::Tune(to,call))=>{assert_eq!(to,ch);let policy=Policy::passive(board,2407+5*u16::from(to)).unwrap();
            channel::run(phy,board,&receive.prepared.prefix,&receive.prepared.paths,tables,&policy).unwrap();
            service.tuned(to);wlanhw::reply_set_channel(call,Ok(())).unwrap();to}
        Some(Ask::Admit(_))=>panic!("admission asked by a tune"),
        None=>ch,
    }
}

// 213-DRV-0032: the native path, begun receive-only (receive_service), admitted on the station's country when it first
// sends, then served for data on the same attempt.
#[cfg(rtw89_service)]
#[test]
fn a_receive_only_start_is_admitted_under_the_stations_country_and_served(){
    use crate::{codec::{self,Reader,Wire,Writer},service::Service,service_ipc::ipc,wlanhw,btc_session::{data_mode,listen},
        phy::{Phy,channel::Policy},receive_service::{self,Ask,ReceiveOnly}};
    use crate::phy::channel::calibration_policy::PlatformInput as P;
    download::tests::with_synthetic_firmware(|verified|{
        let mut raw=model::Device::new(2);let board=model::read_board(&mut raw);
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
        let mut base=initial::Device::new(raw);base.channel=Some(initial::channel::ChannelModel::default());
        let control=Rc::new(RefCell::new(Control{armed:true,..Control::default()}));
        let mut bus=Device::new(base,&txmem,control.clone());let mut scratch=vec![0;rx::BUFFER_BYTES];
        let mut bank=Box::new(btc_session::inbox::Inbox::new());
        let mut second=Box::new(btc_session::inbox::Inbox::new());
        let mut session=Session::new(&mut bus,&mut queue,&mut rings,&receiving,ready,&mut scratch).unwrap();
        ipc::reset();
        {
            let mut phy=Phy::new(&mut session,board.identity);
            let receive=listen::prepare_receive(&mut phy,&board,&caps,&tables,&Policy::passive(&board,2437).unwrap()).unwrap();
            assert_eq!(receive.channel,6);
            let mut service=ReceiveOnly::new(board.identity.mac,receive.channel,true);let mut events=Vec::new();
            // The station's first scan: every channel tuned passively, as the serving loop does, then back to 6.
            for ch in (1..=13u8).chain([6]){
                let id=ipc::words(2,usize::from(ch));assert!(service.step(phy.bus_between_stages(),&mut |e|events.push(e)));
                if ch==service_channel(&mut service,&mut phy,&board,&receive,&tables,ch){assert_eq!(ipc::response(id).unwrap().0,[0,0]);}
            }
            let mut body=vec![0;wlanhw::REQUEST_MAX];let mut w=Writer::new(&mut body);
            codec::encode_str::<2>("US",&mut w).unwrap();1u8.encode(&mut w).unwrap();
            let limits:Vec<_>=(1..=11).map(|channel|wlanhw::Limit{channel,eirp:20,listen:false}).collect();
            codec::encode_slice::<wlanhw::Limit,64>(&limits,&mut w).unwrap();let length=w.len();
            let set=ipc::buffer(9,&body[..length]);assert!(service.step(phy.bus_between_stages(),&mut |e|events.push(e)));
            assert_eq!(ipc::response(set).unwrap().0[0]&255,0,"the country is applied");
            let mut body=[0;wlanhw::REQUEST_MAX];let mut w=Writer::new(&mut body);
            codec::encode_bytes::<2346>(&frame(),&mut w).unwrap();12u8.encode(&mut w).unwrap();1u8.encode(&mut w).unwrap();let length=w.len();
            let first=ipc::buffer(4,&body[..length]);assert!(service.step(phy.bus_between_stages(),&mut |e|events.push(e)));
            assert_eq!(ipc::response(first).unwrap().0,[2,wlanhw::Error::Busy as usize]);
            let Some(Ask::Admit(6))=service.take_ask() else{panic!("no admission asked")};
            service.hand_over();
            let (tuned,calibrated,station)=receive_service::calibrate(&mut phy,&board,&caps,&tables,&receive.prepared,
                service.limits().unwrap(),6,P::CheckedAbsent).unwrap();
            assert_eq!((tuned.channel,tuned.access),(6,channel::Access::InitiateRadiation));
            // As serve does: the inbox only now, so that receive-only and the calibrations retain no packet.
            assert!(!phy.bus_between_stages().retains_packets());phy.bus_between_stages().attach_inbox(&mut bank).unwrap();
            assert!(phy.bus_between_stages().attach_inbox(&mut second).is_err(),"one inbox only");
            let activation=data_mode::prepare(&mut phy,&board,&tuned,&calibrated,&receive.prepared.owner,station,&transmit).unwrap();
            drop(phy);
            let (mut data,_)=connection::activate(session,&mut transmit,activation).unwrap();
            let mut served=Service::new(&mut data).unwrap();
            let info=ipc::buffer(1,&[]);assert!(served.step());let(words,bytes)=ipc::response(info).unwrap();
            let mut r=Reader::new(&bytes[..words[0]>>16]);assert_eq!(wlanhw::Info::decode(&mut r).unwrap().channel,6);
            // The station's next try goes out, acknowledged.
            let sent=ipc::buffer(4,&body[..length]);assert!(served.step());assert!(served.step());
            let(words,bytes)=ipc::response(sent).unwrap();assert_eq!(words,[2<<16,0]);
            let mut r=Reader::new(&bytes[..2]);assert_eq!(wlanhw::Sent::decode(&mut r).unwrap(),wlanhw::Sent{acked:true,attempts:1});
            let stopped=served.stop();assert!(stopped.failure.is_none());
            assert!(matches!(stopped.shutdown,Shutdown::Attempted{radio:None,transmit:tx::Stop{request:None,idle:None}}));
            assert_eq!(ipc::outstanding(),0);assert_eq!(ipc::duplicates(),0);
        }
        let stop=transmit.stop(&mut bus);assert!(stop.request.is_none()&&stop.idle.is_none());
        rings.stop(&mut bus);runtime_mac::disable_radio(&mut bus).unwrap();download::stop(&mut bus,&mut queue);
        model::power_off(&mut bus.station.base.inner);
    });
}
