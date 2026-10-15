use crate::{c2h, download, fwcmd, io, model, phycap, post_mac, runtime_mac, rx, startup};
use crate::{btc_session,h2c,phy::PhyIo,io::RegisterIo};

impl btc_session::DeviceIo for model::Device {
    fn pci_id(&mut self)->io::Result<u32>{Ok(0xb85210ec)}
    fn bar_len(&self)->usize{0x20000}
}

fn shutdown(device: &mut model::Device, queue: &mut fwcmd::Queue<'_>, rings: &mut rx::Rings<'_>) {
    let rx = rings.stop(device);
    assert_eq!((rx.request, rx.hci, rx.idle), (None, None, None));
    runtime_mac::disable_radio(device).unwrap();
    assert_eq!(download::stop(device, queue), download::Stop { dma: None, cpu: None });
    model::power_off(device);
    assert_eq!(device.command, 0);
    assert_eq!(device.get(fwcmd::DMA_STOP, 4) & 0x1f0f03, 0x1f0f03);
    assert_eq!(device.get(fwcmd::INIT_CONFIG, 4) & 0x2800, 0);
    assert_eq!(device.get(0x88, 4) & 2, 0);
    assert_eq!(device.get(2, 1) & 3, 0);
}

#[test]
fn one_cold_device_reaches_actual_capabilities_mac_and_receive_without_reseeding() {
    for speed in [1, 2] {
        download::tests::with_synthetic_firmware(|verified| {
            let mut device = model::Device::new(speed);
            let board = model::read_board(&mut device);
            let mut region = model::Region::new();
            let (mut queue, mut rings) = region.pair(&mut device);
            assert_eq!(rings.physical(), queue.physical() + startup::COMMAND_REGION_BYTES as u64);
            model::prepare_receive(&mut device, &mut queue, &mut rings).unwrap();
            assert_eq!(rings.state(), rx::State::Installed);
            let ready = download::run(&mut device, &mut queue, verified).unwrap();
            let caps = phycap::query(&mut device, &mut queue, &board, runtime_mac::BAR_BYTES, &ready, phycap::MAX_TIMEOUT_US).unwrap();
            assert!(caps.belongs_to(&ready));
            assert_eq!((caps.tx_nss(), caps.rx_nss(), caps.effective_rx_path()), (2, 2, 3));
            assert!(!caps.tx_nss_defaulted() && !caps.rx_nss_defaulted());
            let prepared = runtime_mac::prepare(&mut device, &mut queue, &board, runtime_mac::BAR_BYTES, &ready).unwrap();
            let receiving = post_mac::activate(&mut device, &mut queue, &mut rings, &board, runtime_mac::BAR_BYTES, &prepared).unwrap();
            receiving.validate(&mut device, &mut queue, &rings, 1000).unwrap();
            assert_eq!(device.mailbox_requests, [[0x103, 0, 0, 0], [0x205, 0xffff, 0, 0]]);
            assert_eq!(device.get(0x1f5, 1), 0x22);
            assert_eq!(queue.pending(), 0); assert_eq!(queue.retained(), 8);
            assert_eq!(rings.state(), rx::State::Running);
            assert_eq!(device.get(0xc348, 4) & 0xffff, 0);
            assert_eq!(device.get(fwcmd::DMA_STOP, 4) & 0x1f0f03, 0xb0f00);
            assert_eq!([0x1a0, 0x10b0, 0x13b0].map(|at| device.get(at, 4)), [0; 3]);
            assert_eq!(board.identity.mac, [2, 3, 4, 5, 6, 7]);
            assert!(device.now < 2_000_000);

            // Hardware writes into the actual programmed descriptor, then reuses it immediately after host release.
            let event = [0x49, 0, 0, 0, 12, 0, 0, 0, 0x5a, 0x6b, 0x7c, 0x8d];
            device.inject_c2h(&event);
            let mut output = vec![0; rx::BUFFER_BYTES];
            let rx::Outcome::Packet(packet) = rings.receive(&mut device, rx::Channel::Rxq, &mut output, 1000).unwrap() else { panic!("missing C2H"); };
            let received = c2h::parse(packet.bytes(&output).unwrap()).unwrap();
            assert_eq!(received.bytes, event);
            assert_eq!(device.get(0x1050, 4), 0x10001);
            assert_eq!(rings.receive(&mut device, rx::Channel::Rxq, &mut output, 1000).unwrap(), rx::Outcome::Empty);
            receiving.validate(&mut device, &mut queue, &rings, 1000).unwrap();

            shutdown(&mut device, &mut queue, &mut rings);
            let before = device.writes.len();
            assert!(receiving.validate(&mut device, &mut queue, &rings, 1000).is_err());
            assert_eq!(device.writes.len(), before);
        });
    }
}

#[test]
fn real_zero_or_single_nss_reply_keeps_source_resolution_in_same_startup_session() {
    for (response, nss, path, defaulted) in [([0x403, 0, 0, 0], 2, 3, true), ([0x10403, 1, 0, 0x10100], 1, 2, false)] {
        download::tests::with_synthetic_firmware(|verified| {
            let mut device = model::Device::new(1); device.feature_reply = Some(response);
            let board = model::read_board(&mut device); let mut region = model::Region::new();
            let (mut queue, mut rings) = region.pair(&mut device);
            model::prepare_receive(&mut device, &mut queue, &mut rings).unwrap();
            let ready = download::run(&mut device, &mut queue, verified).unwrap();
            let caps = phycap::query(&mut device, &mut queue, &board, runtime_mac::BAR_BYTES, &ready, phycap::MAX_TIMEOUT_US).unwrap();
            assert_eq!(caps.raw(), response);
            assert_eq!((caps.rx_nss(), caps.tx_nss(), caps.effective_rx_path()), (nss, nss, path));
            assert_eq!((caps.tx_nss_defaulted(), caps.rx_nss_defaulted()), (defaulted, defaulted));
            let prepared = runtime_mac::prepare(&mut device, &mut queue, &board, runtime_mac::BAR_BYTES, &ready).unwrap();
            post_mac::activate(&mut device, &mut queue, &mut rings, &board, runtime_mac::BAR_BYTES, &prepared).unwrap();
            assert_eq!(device.mailbox_requests.len(), 2);
            shutdown(&mut device, &mut queue, &mut rings);
        });
    }
}

#[test]
fn missing_feature_response_cannot_become_default_capabilities_or_rx_activation() {
    download::tests::with_synthetic_firmware(|verified| {
        let mut device = model::Device::new(1); device.feature_reply = None;
        let board = model::read_board(&mut device); let mut region = model::Region::new();
        let (mut queue, mut rings) = region.pair(&mut device);
        model::prepare_receive(&mut device, &mut queue, &mut rings).unwrap();
        let ready = download::run(&mut device, &mut queue, verified).unwrap();
        let error = phycap::query(&mut device, &mut queue, &board, runtime_mac::BAR_BYTES, &ready, phycap::MAX_TIMEOUT_US).unwrap_err();
        assert_eq!(error.stage, phycap::Stage::Mailbox); assert_eq!(error.cause.kind, io::FaultKind::Timeout);
        assert_eq!(device.mailbox_requests, [[0x103, 0, 0, 0]]);
        assert_eq!(queue.state(), fwcmd::State::Faulted); assert_eq!(rings.state(), rx::State::Installed);
        assert_eq!(device.get(2, 1) & 3, 0);
        assert_eq!(device.get(fwcmd::INIT_CONFIG, 4) & 0x2800, 0);
        let count = device.writes.len();
        assert_eq!(runtime_mac::prepare(&mut device, &mut queue, &board, runtime_mac::BAR_BYTES, &ready).unwrap_err().stage, runtime_mac::Stage::Guard);
        assert_eq!(device.writes.len(), count);
        shutdown(&mut device, &mut queue, &mut rings);
    });
}

#[test]
fn runtime_station_timeout_preserves_primary_error_and_stops_same_downloaded_device() {
    download::tests::with_synthetic_firmware(|verified| {
        let mut device = model::Device::new(2); device.station_stuck = true;
        let board = model::read_board(&mut device); let mut region = model::Region::new();
        let (mut queue, mut rings) = region.pair(&mut device);
        model::prepare_receive(&mut device, &mut queue, &mut rings).unwrap();
        let ready = download::run(&mut device, &mut queue, verified).unwrap();
        phycap::query(&mut device, &mut queue, &board, runtime_mac::BAR_BYTES, &ready, phycap::MAX_TIMEOUT_US).unwrap();
        let error = runtime_mac::prepare(&mut device, &mut queue, &board, runtime_mac::BAR_BYTES, &ready).unwrap_err();
        assert_eq!(error.stage, runtime_mac::Stage::Station); assert_eq!(error.cause.kind, io::FaultKind::Timeout);
        assert!(error.radio_stop.is_none()); assert_eq!(error.command_stop, Some(download::Stop { dma: None, cpu: None }));
        assert_eq!(device.get(2, 1) & 3, 0); assert_eq!(device.get(0x88, 4) & 2, 0);
        assert_eq!(rings.state(), rx::State::Installed); assert_eq!(queue.state(), fwcmd::State::Faulted);
        shutdown(&mut device, &mut queue, &mut rings);
    });
}

#[test]
fn ignored_receive_ring_installation_never_reaches_mastering_or_firmware() {
    let mut device = model::Device::new(1); device.ignore_receive_base = true;
    model::read_board(&mut device); let mut region = model::Region::new();
    let (mut queue, mut rings) = region.pair(&mut device);
    assert!(model::prepare_receive(&mut device, &mut queue, &mut rings).is_err());
    assert_eq!(device.command, 2); assert_eq!(queue.state(), fwcmd::State::Installed);
    assert_eq!(rings.state(), rx::State::Faulted); assert!(device.packets.is_empty());
    assert!(device.mailbox_requests.is_empty());
    shutdown(&mut device, &mut queue, &mut rings);
}

#[test]
fn possibly_applied_receive_enable_error_stops_rx_radio_and_command_independently() {
    download::tests::with_synthetic_firmware(|verified| {
        let mut device = model::Device::new(1); let board = model::read_board(&mut device);
        let mut region = model::Region::new(); let (mut queue, mut rings) = region.pair(&mut device);
        model::prepare_receive(&mut device, &mut queue, &mut rings).unwrap();
        let ready = download::run(&mut device, &mut queue, verified).unwrap();
        phycap::query(&mut device, &mut queue, &board, runtime_mac::BAR_BYTES, &ready, phycap::MAX_TIMEOUT_US).unwrap();
        let prepared = runtime_mac::prepare(&mut device, &mut queue, &board, runtime_mac::BAR_BYTES, &ready).unwrap();
        device.fail_receive_enable = true;
        let error = post_mac::activate(&mut device, &mut queue, &mut rings, &board, runtime_mac::BAR_BYTES, &prepared).unwrap_err();
        assert_eq!(error.stage, post_mac::Stage::Receive);
        assert!(matches!(error.cause, post_mac::Error::Register(io::Fault { kind: io::FaultKind::Access, .. })));
        assert_eq!(error.receive_stop, Some(rx::Stop { request: None, hci: None, idle: None }));
        assert!(error.radio_stop.is_none()); assert_eq!(error.command_stop, Some(download::Stop { dma: None, cpu: None }));
        assert_eq!(rings.state(), rx::State::Stopped); assert_eq!(queue.state(), fwcmd::State::Faulted);
        shutdown(&mut device, &mut queue, &mut rings);
    });
}

fn with_cold_session(speed:u8,ack:model::NormalAck,hold:bool,check:impl FnOnce(&mut btc_session::Session<'_,'_,'_,model::Device>))->Vec<Vec<u8>> {
    let mut packets=Vec::new();
    download::tests::with_synthetic_firmware(|verified| {
        let mut device=model::Device::new(speed);let board=model::read_board(&mut device);
        let mut region=model::Region::new();let(mut queue,mut rings)=region.pair(&mut device);
        model::prepare_receive(&mut device,&mut queue,&mut rings).unwrap();
        let ready=download::run(&mut device,&mut queue,verified).unwrap();
        let tables=ready.default_tables();
        let caps=phycap::query(&mut device,&mut queue,&board,runtime_mac::BAR_BYTES,&ready,phycap::MAX_TIMEOUT_US).unwrap();
        assert!(caps.belongs_to(&ready));assert!(tables.belongs_to(&ready));
        let prepared=runtime_mac::prepare(&mut device,&mut queue,&board,runtime_mac::BAR_BYTES,&ready).unwrap();
        let receiving=post_mac::activate(&mut device,&mut queue,&mut rings,&board,runtime_mac::BAR_BYTES,&prepared).unwrap();
        device.normal_ack=ack;device.hold_normal_dma=hold;
        let mut scratch=vec![0;rx::BUFFER_BYTES];
        let failed={
            let mut session=btc_session::Session::new(&mut device,&mut queue,&mut rings,&receiving,ready,&mut scratch).unwrap();
            assert_eq!(session.command_session(),tables.session());
            check(&mut session);
            session.failed().is_some()
        };
        assert_eq!(device.transferred,326_472);assert_eq!(device.packets.len(),164);
        if hold{assert_eq!(queue.pending(),1);}else if !failed{assert_eq!(queue.pending(),0);}else{assert!(queue.pending()<=1);}
        assert_eq!(device.mailbox_requests,[[0x103,0,0,0],[0x205,0xffff,0,0]]);
        assert_eq!(board.identity.mac,[2,3,4,5,6,7]);
        shutdown(&mut device,&mut queue,&mut rings);
        packets=std::mem::take(&mut device.normal_packets);
    });packets
}

fn ack_command(session:&mut btc_session::Session<'_,'_,'_,model::Device>)->Result<h2c::Outcome,btc_session::Error>{
    session.command(h2c::Command{category:2,class:16,function:3},&[0,12,3,0,0,0,0,0,0,0,0,0,0,0],h2c::Acks{received:false,done:true},1000)
}

fn assert_poisoned(session:&mut btc_session::Session<'_,'_,'_,model::Device>,error:btc_session::Error){
    assert_eq!(session.failed(),Some(error));let sent=session.submitted();
    assert_eq!(ack_command(session),Err(error));assert_eq!(session.submitted(),sent);
    assert_eq!(PhyIo::command_session(session),0);assert!(session.pci_id().is_err());
    assert!(session.receive_permit(1000).is_err());
}

#[test]
fn cold_session_shares_normal_sequence_across_rf_mirror_and_btc_without_reseeding(){
    for speed in [1,2]{
        let packets=with_cold_session(speed,model::NormalAck::Complete,false,|session|{
            session.send_rf_page(0,0,&[0x78,0x56,0x34,0x12],1000).unwrap();
            assert_eq!(ack_command(session).unwrap(),h2c::Outcome::Done);
            for _ in 0..4{session.send_rf_page(1,0,&[0x21,0x43,0x65,0x87],1000).unwrap();}
            assert_eq!(session.submitted(),6);assert_eq!(session.failed(),None);
            assert_eq!(session.counts(),btc_session::Counts::default());session.receive_permit(1000).unwrap();
        });
        assert_eq!(packets.len(),6);
        for(i,packet)in packets.iter().enumerate(){
            let flags=u32::from_le_bytes(packet[4..8].try_into().unwrap());
            assert_eq!(packet[3],i as u8);assert_eq!(flags&0x3fff,packet.len()as u32);
            assert_eq!(flags&0x4000!=0,i%4==0);assert_eq!(flags&0x8000!=0,i==1);
        }
        assert_eq!(&packets[0][8..],&[0x78,0x56,0x34,0x12]);assert_eq!(packets[0][0],2|8<<2);
        assert_eq!(packets[1][0],2|16<<2);assert_eq!(packets[1][1],3);
        assert_eq!(packets[2][0],2|9<<2);
    }
}

#[test]
fn cold_session_dma_consumption_without_matching_ack_times_out_and_cannot_retry(){
    for ack in [model::NormalAck::Missing,model::NormalAck::Foreign]{
        let packets=with_cold_session(1,ack,false,|session|{
            let start=session.now_us();let error=ack_command(session).unwrap_err();
            assert!(matches!(error,btc_session::Error::Timeout|btc_session::Error::Command(h2c::Error::Failed(h2c::Failure::Timeout))|btc_session::Error::Register(io::Fault{kind:io::FaultKind::Timeout,..})));
            assert!(session.now_us()-start<=1000);
            assert_eq!(session.counts().unrelated_ack,if ack==model::NormalAck::Foreign{2}else{0});
            assert_poisoned(session,error);
        });assert_eq!(packets.len(),1);
    }
}

#[test]
fn cold_session_rejects_firmware_error_truncated_ack_and_wrong_rx_channel(){
    for ack in [model::NormalAck::Rejected(7),model::NormalAck::Truncated,model::NormalAck::WrongChannel]{
        let packets=with_cold_session(2,ack,false,|session|{
            let error=ack_command(session).unwrap_err();
            assert_eq!(error,match ack{model::NormalAck::Rejected(status)=>btc_session::Error::Rejected(status),
                model::NormalAck::Truncated=>btc_session::Error::Event(c2h::Error::Truncated),_=>btc_session::Error::Malformed});
            assert_poisoned(session,error);
        });assert_eq!(packets.len(),1);
    }
}

#[test]
fn cold_session_ack_does_not_replace_hardware_dma_consumption(){
    let packets=with_cold_session(1,model::NormalAck::Complete,true,|session|{
        let start=session.now_us();let error=ack_command(session).unwrap_err();
        assert!(matches!(error,btc_session::Error::Timeout|btc_session::Error::Register(io::Fault{kind:io::FaultKind::Timeout,..})));
        assert!(session.now_us()-start<=1000);assert_eq!(session.counts().unrelated_ack,0);
        assert_poisoned(session,error);
    });assert_eq!(packets.len(),1);
}
