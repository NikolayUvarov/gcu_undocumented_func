use crate::{btc_session::listen::{self, Cause, Heard, SCAN_OPEN}, download, fwcmd, model, phycap, post_mac, runtime_mac, rx,
    phy::channel::{self, Access, CalibrationEngine, CalibrationTransmit, Tuned}};

#[path="rtw89_initial_phy_device.rs"] mod device;
use device::{Device, channel::ChannelModel};

// A management frame from `bssid` with the given elements, followed by a four-byte FCS.
fn management(subtype: u8, bssid: [u8; 6], elements: &[u8]) -> Vec<u8> {
    let mut frame = vec![subtype << 4, 0, 0, 0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
    frame.extend_from_slice(&bssid); frame.extend_from_slice(&bssid); frame.extend_from_slice(&[0x10, 0]);
    frame.extend_from_slice(&[0; 8]); frame.extend_from_slice(&[100, 0, 0x11, 4]);
    frame.extend_from_slice(elements); frame.extend_from_slice(&[0; 4]); frame
}

fn run(center_mhz: u16, frames: Vec<Vec<u8>>, check: impl FnOnce(&mut Device, Result<listen::Listened, listen::Failure>, Vec<Heard>)) {
    download::tests::with_synthetic_firmware(|verified| {
        let mut raw = model::Device::new(2); let board = model::read_board(&mut raw);
        let mut region = model::Region::new(); let (mut queue, mut rings) = region.pair(&mut raw);
        model::prepare_receive(&mut raw, &mut queue, &mut rings).unwrap();
        let ready = download::run(&mut raw, &mut queue, verified).unwrap();
        let caps = phycap::query(&mut raw, &mut queue, &board, runtime_mac::BAR_BYTES, &ready, phycap::MAX_TIMEOUT_US).unwrap();
        let runtime = runtime_mac::prepare(&mut raw, &mut queue, &board, runtime_mac::BAR_BYTES, &ready).unwrap();
        let receiving = post_mac::activate(&mut raw, &mut queue, &mut rings, &board, runtime_mac::BAR_BYTES, &runtime).unwrap();
        let mut device = Device::new(raw); device.channel = Some(ChannelModel::default()); device.on_scan = frames;
        let mut scratch = vec![0; rx::BUFFER_BYTES]; let mut heard = Vec::new();
        let result = listen::listen(&mut device, &mut queue, &mut rings, &receiving, ready, &board, &caps, &mut scratch,
            center_mhz, 3_000, |found| heard.push(*found));
        check(&mut device, result, heard);
        assert_eq!(device.inner.get(0xc348, 4) & 0xffff, 0);
        assert_eq!(device.inner.get(fwcmd::DMA_STOP, 4) & 0xb0f00, 0xb0f00);
        let stopped = rings.stop(&mut device); assert_eq!((stopped.request, stopped.hci, stopped.idle), (None, None, None));
        runtime_mac::disable_radio(&mut device).unwrap();
        assert_eq!(download::stop(&mut device, &mut queue), download::Stop { dma: None, cpu: None });
        model::power_off(&mut device.inner); assert_eq!(device.inner.command, 0);
    });
}

#[test]
fn receive_only_listen_hears_each_bssid_once_with_ssid_and_ds_channel() {
    let a = [2, 0x11, 0x22, 0x33, 0x44, 0x55]; let b = [2, 0x66, 0x77, 0x88, 0x99, 0xaa];
    let beacon = management(8, a, &[0, 3, b'n', b'e', b't', 1, 1, 0x82, 3, 1, 1]);
    let data = { let mut f = vec![8, 2]; f.resize(24, 0); f.extend_from_slice(&[0; 4]); f };
    let frames = vec![beacon.clone(), beacon, management(5, b, &[0, 0, 3, 1, 1, 0xdd, 9, 1]), data];
    run(2412, frames, |d, result, heard| {
        let listened = result.unwrap();
        assert_eq!(listened.channel, 1); assert!(listened.elapsed_us >= 3_000);
        let t = listened.tally;
        assert_eq!((t.frames, t.beacons, t.probe_responses, t.other_frames, t.rejected, t.unlisted), (4, 2, 1, 1, 0, 0));
        assert_eq!(listened.heard, 2); assert_eq!(heard.len(), 2);
        assert_eq!((heard[0].bssid, heard[0].ds_channel, heard[0].ssid()), (a, 1, &b"net"[..]));
        assert_eq!((heard[1].bssid, heard[1].ds_channel, heard[1].ssid()), (b, 1, &b""[..]));
        let filter = d.inner.get(0xce20, 4);
        assert_eq!(filter & SCAN_OPEN, 0); assert_eq!(filter & 0x0300_4438, 0x0300_4438);
        assert_eq!(d.inner.get(0xce40, 4), 0x2b); assert!(d.on_scan.is_empty());
    });
}

#[test]
fn a_frequency_outside_channels_one_to_thirteen_is_refused_before_any_command() {
    for center in [2484, 2413, 5180] { run(center, Vec::new(), |d, result, heard| {
        assert!(matches!(result.unwrap_err().cause, Cause::Policy)); assert!(heard.is_empty());
        assert!(d.inner.normal_packets.is_empty()); assert_eq!(d.inner.get(0xce20, 4) & SCAN_OPEN, SCAN_OPEN);
    }); }
}

#[test]
fn a_passive_tune_can_never_carry_calibration_transmit_authority() {
    let mut raw = model::Device::new(2); let board = model::read_board(&mut raw);
    let policy = channel::Policy::passive(&board, 2412).unwrap();
    assert!(channel::Policy::passive(&board, 2472).is_ok());
    let tuned = Tuned::modeled(1, 1, 1, 0, Access::PassiveOnly);
    for engine in [CalibrationEngine::Iqk, CalibrationEngine::Tssi, CalibrationEngine::Dpk] {
        assert!(CalibrationTransmit::modeled(&tuned, engine).is_none());
    }
    assert_eq!(format!("{policy:?}").contains("PassiveOnly"), true);
}
