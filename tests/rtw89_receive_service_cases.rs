use crate::{btc_session::{listen, Session}, codec::{self, Reader, Wire, Writer}, download, fwcmd, model, phycap, post_mac, runtime_mac, rx,
    phy::Phy, receive_service::{Event, ReceiveOnly}, service_ipc::ipc, wlanhw, wire};

#[path="rtw89_initial_phy_device.rs"] mod device;
use device::{Device, channel::ChannelModel};

const BSSID: [u8; 6] = [2, 0x11, 0x22, 0x33, 0x44, 0x55];

// A beacon from BSSID on channel 1, with its FCS.
fn beacon() -> Vec<u8> {
    let mut frame = vec![0x80, 0, 0, 0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
    frame.extend_from_slice(&BSSID); frame.extend_from_slice(&BSSID); frame.extend_from_slice(&[0x10, 0]);
    frame.extend_from_slice(&[0; 8]); frame.extend_from_slice(&[100, 0, 0x11, 4]);
    frame.extend_from_slice(&[0, 3, b'n', b'e', b't', 3, 1, 1, 0, 0, 0, 0]); frame
}
// Data between two other stations: the scan filter lets it in, the station's filter does not.
fn other_data() -> Vec<u8> { let mut f = vec![0x08, 0x01]; f.extend_from_slice(&[0; 2]); f.extend_from_slice(&[2, 9, 9, 9, 9, 9]); f.resize(28, 7); f }

fn country(code: &str, limits: &[(u8, u8, bool)]) -> Vec<u8> {
    let mut out = vec![0; wlanhw::REQUEST_MAX]; let mut w = Writer::new(&mut out);
    codec::encode_str::<2>(code, &mut w).unwrap(); 2u8.encode(&mut w).unwrap();
    let list: Vec<_> = limits.iter().map(|&(channel, eirp, listen)| wlanhw::Limit { channel, eirp, listen }).collect();
    codec::encode_slice::<wlanhw::Limit, 64>(&list, &mut w).unwrap(); let length = w.len(); out.truncate(length); out
}
fn error(id: usize, wanted: wlanhw::Error) { assert_eq!(ipc::response(id).unwrap().0, [wire::STATUS_ERROR, wanted as usize]); }
fn info(id: usize) -> wlanhw::Info {
    let (w, b) = ipc::response(id).unwrap(); assert_eq!(w[0] & 255, 0);
    let mut r = Reader::new(&b[..w[0] >> 16]); let info = wlanhw::Info::decode(&mut r).unwrap(); assert!(r.done()); info
}

fn serve(frames: Vec<Vec<u8>>, check: impl FnOnce(&mut ReceiveOnly, &mut Session<'_, '_, '_, Device>, &mut Vec<Event>)) {
    download::tests::with_synthetic_firmware(|verified| {
        let mut raw = model::Device::new(2); let board = model::read_board(&mut raw);
        let mut region = model::Region::new(); let (mut queue, mut rings) = region.pair(&mut raw);
        model::prepare_receive(&mut raw, &mut queue, &mut rings).unwrap();
        let ready = download::run(&mut raw, &mut queue, verified).unwrap();
        let caps = phycap::query(&mut raw, &mut queue, &board, runtime_mac::BAR_BYTES, &ready, phycap::MAX_TIMEOUT_US).unwrap();
        let runtime = runtime_mac::prepare(&mut raw, &mut queue, &board, runtime_mac::BAR_BYTES, &ready).unwrap();
        let receiving = post_mac::activate(&mut raw, &mut queue, &mut rings, &board, runtime_mac::BAR_BYTES, &runtime).unwrap();
        let mut device = Device::new(raw); device.channel = Some(ChannelModel::default()); device.on_scan = frames;
        let mut scratch = vec![0; rx::BUFFER_BYTES];
        ipc::reset();
        {
            let (mut session, tables, policy) = listen::open(&mut device, &mut queue, &mut rings, &receiving, ready, &board, &caps,
                &mut scratch, 2412).unwrap();
            let mut phy = Phy::new(&mut session, board.identity);
            let receive = listen::prepare_receive(&mut phy, &board, &caps, &tables, &policy).unwrap();
            assert_eq!(receive.channel, 1);
            let mut service = ReceiveOnly::new(board.identity.mac, receive.channel);
            let mut events = Vec::new();
            check(&mut service, phy.bus_between_stages(), &mut events);
        }
        assert_eq!(ipc::outstanding(), 0); assert_eq!(ipc::duplicates(), 0);
        // Nothing was sent: the scheduler is zero and data DMA stopped, as at the start.
        assert_eq!(device.inner.get(0xc348, 4) & 0xffff, 0);
        assert_eq!(device.inner.get(fwcmd::DMA_STOP, 4) & 0xb0f00, 0xb0f00);
        let stopped = rings.stop(&mut device); assert_eq!((stopped.request, stopped.hci, stopped.idle), (None, None, None));
        runtime_mac::disable_radio(&mut device).unwrap();
        assert_eq!(download::stop(&mut device, &mut queue), download::Stop { dma: None, cpu: None });
        model::power_off(&mut device.inner);
    });
}

#[test]
fn info_names_the_receive_only_channel_and_the_radios_own_channels() { serve(Vec::new(), |service, session, events| {
    let id = ipc::buffer(1, &[]); assert!(service.step(session, &mut |e| events.push(e)));
    let info = info(id);
    assert_eq!(info.channels.as_slice(), &[1]); assert_eq!(info.supported.as_slice(), &(1..=13).collect::<Vec<u8>>()[..]);
    assert_eq!((info.channel, info.bands, info.frame_max), (1, 5, 2346));
    for (channel, ok) in [(1, true), (6, false), (14, false)] {
        let id = ipc::words(2, channel); assert!(service.step(session, &mut |e| events.push(e)));
        if ok { assert_eq!(ipc::response(id).unwrap().0, [0, 0]); } else { error(id, wlanhw::Error::Invalid); }
    }
    assert!(events.is_empty());
});}

#[test]
fn nothing_is_sent_before_a_calibrated_channel() { serve(Vec::new(), |service, session, events| {
    let mut body = vec![0; wlanhw::REQUEST_MAX]; let mut w = Writer::new(&mut body);
    codec::encode_bytes::<2346>(&beacon()[..40], &mut w).unwrap(); 2u8.encode(&mut w).unwrap(); 1u8.encode(&mut w).unwrap();
    let length = w.len(); body.truncate(length);
    let id = ipc::buffer(4, &body); assert!(service.step(session, &mut |e| events.push(e)));
    error(id, wlanhw::Error::Failed);
});}

#[test]
fn a_country_is_taken_but_answered_failed_until_calibration() { serve(Vec::new(), |service, session, events| {
    let all: Vec<_> = (1..=13).map(|ch| (ch, 20, false)).collect();
    let id = ipc::buffer(9, &country("UZ", &all)); assert!(service.step(session, &mut |e| events.push(e)));
    error(id, wlanhw::Error::Failed);
    assert_eq!(events.as_slice(), &[Event::CountryTaken { code: *b"UZ", channels: 13, listen_only: 0 }]);
    assert_eq!(service.limits().unwrap().applied(1).unwrap().eirp_half_dbm, 40);
    let id = ipc::buffer(9, &country("00", &all)); assert!(service.step(session, &mut |e| events.push(e)));
    error(id, wlanhw::Error::Failed);
    assert_eq!(events[1], Event::CountryTaken { code: *b"00", channels: 13, listen_only: 2 });
    for body in [country("uz", &all), country("UZ", &[(1, 20, false), (1, 20, false)]), country("UZ", &[(14, 20, false)])] {
        let id = ipc::buffer(9, &body); assert!(service.step(session, &mut |e| events.push(e)));
        error(id, wlanhw::Error::Invalid);
    }
    assert_eq!(events.len(), 2, "a refused country is no event");
});}

#[test]
fn received_beacons_reach_the_station_and_other_traffic_does_not() {
    let mut data = other_data(); data.extend_from_slice(&[0; 4]);
    serve(vec![beacon(), data], |service, session, events| {
        let wait = ipc::words(6, 0); assert!(service.step(session, &mut |e| events.push(e)));
        assert_eq!(ipc::response(wait).unwrap().0, [1 << 16, 0], "one frame waits: the data frame was filtered out");
        let id = ipc::buffer(5, &[]); assert!(service.step(session, &mut |e| events.push(e)));
        let (w, b) = ipc::response(id).unwrap(); let mut r = Reader::new(&b[..w[0] >> 16]);
        let received = codec::decode_bytes::<2354>(&mut r).unwrap(); assert!(r.done());
        let mpdu = &beacon()[..beacon().len() - 4];
        assert_eq!(&received[..8], &[mpdu.len() as u8, (mpdu.len() >> 8) as u8, 0, 1, 2, 1, 0, 0]);
        assert_eq!(&received[8..], mpdu);
        let empty = ipc::buffer(5, &[]); assert!(service.step(session, &mut |e| events.push(e)));
        error(empty, wlanhw::Error::Empty);
        assert_eq!(service.dropped(), 0);
    });
}
