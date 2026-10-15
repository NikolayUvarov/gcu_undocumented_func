use crate::{c2h, download, fwcmd, io, model, phycap, post_mac, runtime_mac, rx, startup};

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
