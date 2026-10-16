use crate::{download, fwcmd, initial_phy, model, phycap, post_mac, runtime_mac, rx};

#[path="rtw89_initial_phy_device.rs"] mod device;
use device::Device;

fn run(speed: u8, configure: impl FnOnce(&mut Device), check: impl FnOnce(&mut Device, Result<initial_phy::Observations, initial_phy::Failure>)) {
    download::tests::with_synthetic_firmware(|verified| {
        let mut raw = model::Device::new(speed); let board = model::read_board(&mut raw);
        let mut region = model::Region::new(); let (mut queue, mut rings) = region.pair(&mut raw);
        model::prepare_receive(&mut raw, &mut queue, &mut rings).unwrap();
        let ready = download::run(&mut raw, &mut queue, verified).unwrap();
        let caps = phycap::query(&mut raw, &mut queue, &board, runtime_mac::BAR_BYTES, &ready, phycap::MAX_TIMEOUT_US).unwrap();
        let runtime = runtime_mac::prepare(&mut raw, &mut queue, &board, runtime_mac::BAR_BYTES, &ready).unwrap();
        let receiving = post_mac::activate(&mut raw, &mut queue, &mut rings, &board, runtime_mac::BAR_BYTES, &runtime).unwrap();
        let mut device = Device::new(raw); configure(&mut device); let mut scratch = vec![0; rx::BUFFER_BYTES];
        let result = initial_phy::diagnose(&mut device, &mut queue, &mut rings, &receiving, ready, &board, &caps, &mut scratch);
        check(&mut device, result);
        assert_eq!(device.inner.get(0xc348, 4) & 0xffff, 0);
        assert_eq!(device.inner.get(fwcmd::DMA_STOP, 4) & 0xb0f00, 0xb0f00);
        let stopped = rings.stop(&mut device); assert_eq!((stopped.request, stopped.hci, stopped.idle), (None, None, None));
        runtime_mac::disable_radio(&mut device).unwrap();
        assert_eq!(download::stop(&mut device, &mut queue), download::Stop { dma: None, cpu: None });
        model::power_off(&mut device.inner); assert_eq!(device.inner.command, 0);
    });
}

#[test]
fn cold_gen1_and_gen2_complete_owned_initial_phy_and_then_shut_down() {
    for speed in [1, 2] { run(speed, |_| {}, |d, result| {
        let observed = result.unwrap(); assert_eq!(observed.rck.values, [7, 11]); assert_eq!(observed.dack.drck, 17);
        assert_eq!(observed.dack.completion, [[3, 3]; 2]); assert_eq!((observed.rx_path, observed.rx_nss), (3, 2));
        assert_eq!(observed.encoded_rts, 0x2ff);
        let expected = [(2, 8, 0), (2, 9, 0), (2, 16, 2), (2, 16, 0), (2, 16, 1),
            (2, 16, 5), (2, 16, 5), (2, 16, 3), (2, 16, 10), (2, 16, 5)];
        assert_eq!(observed.submitted as usize, expected.len());
        let headers: Vec<_> = d.inner.normal_packets.iter().map(|p| (p[0] & 3, p[0] >> 2, p[1])).collect();
        assert_eq!(headers, expected);
        assert_eq!(d.inner.normal_packets.len(), observed.submitted as usize);
        for (index, packet) in d.inner.normal_packets.iter().enumerate() { assert_eq!(packet[3], index as u8); }
        assert_eq!(d.inner.get(0xce40, 4), 0x2b); assert_eq!(d.inner.get(0x9c18, 4) & 3, 1);
        assert_eq!(d.inner.get(0x8040, 1), 15); assert!(d.inner.now < 2_000_000);
    }); }
}

#[test]
fn failed_radio_ack_prevents_btc_and_all_initial_calibration() {
    run(2, |d| d.inner.normal_ack = model::NormalAck::Missing, |d, result| {
        let error = result.unwrap_err(); assert!(matches!(error.cause, initial_phy::Cause::Phy(_)));
        assert_eq!(d.inner.normal_packets.len(), 1); assert!(!d.drck && d.addck == [false; 2]);
        assert!(!d.inner.writes.iter().any(|&(at, _, value)| at == 0xce40 && value == 0x2b));
    });
}

#[test]
fn bt_busy_denies_initialization_before_nctl_and_rfk() {
    run(1, |d| d.bt = 2 | 32, |d, result| {
        assert!(matches!(result.unwrap_err().cause, initial_phy::Cause::Coexistence(_)));
        assert!(!d.drck); assert!(!d.inner.writes.iter().any(|&(at, _, value)| at == 0x18080 && value == 4));
    });
}

#[test]
fn failed_nctl_rck_and_dack_cannot_publish_path_or_report_readiness() {
    for failed in 0..3 { run(2, |d| match failed { 0 => d.nctl_stuck = true, 1 => d.rck_stuck = true, _ => d.drck_stuck = true }, |d, result| {
        let error = result.unwrap_err();
        match failed {
            0 | 1 => assert!(matches!(error.cause, initial_phy::Cause::Phy(_))),
            _ => assert!(matches!(error.cause, initial_phy::Cause::Dack(_))),
        }
        assert!(!d.inner.writes.iter().any(|&(at, _, value)| at == 0xce40 && value == 0x2b));
        assert!(d.inner.now < 2_000_000);
    }); }
}

#[test]
fn final_report_write_failure_retains_independent_cleanup_evidence() {
    run(1, |d| d.fail_write = Some(0xce40), |_, result| {
        let error = result.unwrap_err();
        let initial_phy::Cause::Reports { stage, report_stop, receive_stop, radio_stop, command_stop, .. } = error.cause else { panic!("{error:?}") };
        assert_eq!(stage, "ppdu"); assert!(report_stop.is_none() && radio_stop.is_none());
        assert!(receive_stop.is_some() && command_stop.is_some());
    });
}
