use crate::{btc_session::{listen, Session}, codec::{self, Reader, Wire, Writer}, download::{self, DefaultTables}, fwcmd, model, phycap,
    post_mac, runtime_mac, rx, phy::{Phy, channel::{self, Policy}}, board::Board,
    receive_service::{Ask, Event, ReceiveOnly}, service_ipc::ipc, wlanhw, wire};

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

type Live<'p, 'a, 'c, 'r> = Phy<'p, Session<'a, 'c, 'r, Device>>;

fn serve(admits: bool, frames: Vec<Vec<u8>>,
    check: impl FnOnce(&mut ReceiveOnly, &mut Live<'_, '_, '_, '_>, &listen::Receiving, &DefaultTables, &Board, &mut Vec<Event>)) {
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
            let mut service = ReceiveOnly::new(board.identity.mac, receive.channel, admits);
            let mut events = Vec::new();
            check(&mut service, &mut phy, &receive, &tables, &board, &mut events);
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

// What the serving loop does for Ask::Tune: the passive policy of that channel, tuned on the live PHY.
fn tune(service: &mut ReceiveOnly, phy: &mut Live<'_, '_, '_, '_>, receive: &listen::Receiving, tables: &DefaultTables, board: &Board) -> u8 {
    let Some(Ask::Tune(ch, call)) = service.take_ask() else { panic!("no tune asked") };
    let policy = Policy::passive(board, 2407 + 5 * u16::from(ch)).unwrap();
    channel::run(phy, board, &receive.prepared.prefix, &receive.prepared.paths, tables, &policy).unwrap();
    // A tune is said to take 100 us a channel number, so that a scan's longest is known.
    service.tuned(ch, u64::from(ch) * 100); wlanhw::reply_set_channel(call, Ok(())).unwrap(); ch
}
fn step(service: &mut ReceiveOnly, phy: &mut Live<'_, '_, '_, '_>, events: &mut Vec<Event>) {
    assert!(service.step(phy.bus_between_stages(), &mut |e| events.push(e)));
}
fn all(eirp: u8) -> Vec<(u8, u8, bool)> { (1..=13).map(|ch| (ch, eirp, false)).collect() }
fn send_body() -> Vec<u8> { send_frame(&beacon()[..40]) }
fn send_frame(frame: &[u8]) -> Vec<u8> {
    let mut body = vec![0; wlanhw::REQUEST_MAX]; let mut w = Writer::new(&mut body);
    codec::encode_bytes::<2346>(frame, &mut w).unwrap(); 2u8.encode(&mut w).unwrap(); 1u8.encode(&mut w).unwrap();
    let length = w.len(); body.truncate(length); body
}
// A management frame of `subtype` to BSSID, as the station sends it.
fn management(subtype: u8) -> Vec<u8> {
    let mut frame = vec![subtype << 4, 0, 0, 0]; frame.extend_from_slice(&BSSID); frame.extend_from_slice(&[2, 3, 4, 5, 6, 7]);
    frame.extend_from_slice(&BSSID); frame.extend_from_slice(&[0, 0, 0, 0, 1, 0, 0, 0]); frame
}
fn filter(all_beacons: bool, bssid: u64) -> Vec<u8> {
    let mut out = vec![0; 17]; let mut w = Writer::new(&mut out);
    wlanhw::Filter { all_beacons, bssid, own: 0 }.encode(&mut w).unwrap(); out
}

#[test]
fn every_channel_is_tuned_passively_before_a_country() { serve(true, Vec::new(), |service, phy, receive, tables, board, events| {
    let id = ipc::buffer(1, &[]); step(service, phy, events);
    let first = info(id);
    assert_eq!(first.channels.as_slice(), &(1..=13).collect::<Vec<u8>>()[..]);
    assert_eq!(first.supported.as_slice(), &(1..=13).collect::<Vec<u8>>()[..]);
    // Listening on channel 1, but no channel set by the client yet: 0, as a restarted radio says (550-NET-0061).
    assert_eq!((first.channel, first.bands, first.frame_max), (0, 5, 2346));
    let id = ipc::words(2, 1); step(service, phy, events); assert_eq!(ipc::response(id).unwrap().0, [0, 0]);
    let id = ipc::buffer(1, &[]); step(service, phy, events); assert_eq!(info(id).channel, 1);
    assert!(service.take_ask().is_none());
    // Channel 6: answered after the tune the serving loop runs, which sets the RF channel field.
    let id = ipc::words(2, 6); step(service, phy, events); assert!(ipc::response(id).is_none());
    assert_eq!(tune(service, phy, receive, tables, board), 6); assert_eq!(ipc::response(id).unwrap().0, [0, 0]);
    assert_eq!(crate::io::read32(phy.bus_between_stages(), 0x10734).unwrap() >> 16 & 255, 6);
    let id = ipc::buffer(1, &[]); step(service, phy, events); assert_eq!(info(id).channel, 6);
    for channel in [0, 14] { let id = ipc::words(2, channel); step(service, phy, events); error(id, wlanhw::Error::Invalid); }
    assert!(events.is_empty());
});}

#[test]
fn nothing_is_sent_before_a_country() { serve(true, Vec::new(), |service, phy, _, _, _, events| {
    let id = ipc::buffer(4, &send_body()); step(service, phy, events);
    error(id, wlanhw::Error::Failed); assert!(service.take_ask().is_none());
});}

#[test]
fn a_country_is_applied_unless_the_platforms_power_policy_is_unknown() {
    serve(true, Vec::new(), |service, phy, _, _, _, events| {
        let id = ipc::buffer(9, &country("UZ", &all(20))); step(service, phy, events);
        assert_eq!(info(id).channels.as_slice(), &(1..=13).collect::<Vec<u8>>()[..]);
        assert_eq!(events.as_slice(), &[Event::CountryTaken { code: *b"UZ", channels: 13, listen_only: 0, applied: true }]);
        assert_eq!(service.limits().unwrap().applied(1).unwrap().eirp_half_dbm, 40);
        let id = ipc::buffer(9, &country("00", &all(20))); step(service, phy, events);
        assert_eq!(info(id).channels.len(), 13);
        assert_eq!(events[1], Event::CountryTaken { code: *b"00", channels: 13, listen_only: 2, applied: true });
        for body in [country("uz", &all(20)), country("UZ", &[(1, 20, false), (1, 20, false)]), country("UZ", &[(14, 20, false)])] {
            let id = ipc::buffer(9, &body); step(service, phy, events);
            error(id, wlanhw::Error::Invalid);
        }
        assert_eq!(events.len(), 2, "a refused country is no event");
    });
    serve(false, Vec::new(), |service, phy, _, _, _, events| {
        let id = ipc::buffer(9, &country("UZ", &all(20))); step(service, phy, events);
        error(id, wlanhw::Error::Failed);
        assert_eq!(events.as_slice(), &[Event::CountryTaken { code: *b"UZ", channels: 13, listen_only: 0, applied: false }]);
        let id = ipc::buffer(4, &send_body()); step(service, phy, events);
        error(id, wlanhw::Error::Failed); assert!(service.take_ask().is_none(), "unknown platform: nothing is admitted");
    });
}

#[test]
fn under_a_country_the_first_send_asks_admission_and_listen_only_channels_ask_nothing() {
    serve(true, Vec::new(), |service, phy, receive, tables, board, events| {
        // US leaves channels 1 to 11: 12 is refused once it applies.
        let us: Vec<_> = (1..=11).map(|ch| (ch, 30, false)).collect();
        let id = ipc::buffer(9, &country("US", &us)); step(service, phy, events); assert_eq!(info(id).channels.len(), 11);
        let id = ipc::words(2, 12); step(service, phy, events); error(id, wlanhw::Error::Invalid);
        let id = ipc::buffer(4, &send_body()); step(service, phy, events);
        error(id, wlanhw::Error::Busy);
        assert!(matches!(service.take_ask(), Some(Ask::Admit(1))));
        // 00 lets channel 13 be listened to only: tuned there, a send is refused and admits nothing.
        let id = ipc::buffer(9, &country("00", &all(20))); step(service, phy, events); info(id);
        let id = ipc::words(2, 13); step(service, phy, events); tune(service, phy, receive, tables, board);
        assert_eq!(ipc::response(id).unwrap().0, [0, 0]);
        let id = ipc::buffer(4, &send_body()); step(service, phy, events);
        error(id, wlanhw::Error::Failed); assert!(service.take_ask().is_none());
    });
}

#[test]
fn received_beacons_reach_the_station_and_other_traffic_does_not() {
    let mut data = other_data(); data.extend_from_slice(&[0; 4]);
    serve(true, vec![beacon(), data], |service, phy, _, _, _, events| {
        let wait = ipc::words(6, 0); step(service, phy, events);
        assert_eq!(ipc::response(wait).unwrap().0, [1 << 16, 0], "one frame waits: the data frame was filtered out");
        let id = ipc::buffer(5, &[]); step(service, phy, events);
        let (w, b) = ipc::response(id).unwrap(); let mut r = Reader::new(&b[..w[0] >> 16]);
        let received = codec::decode_bytes::<2354>(&mut r).unwrap(); assert!(r.done());
        let mpdu = &beacon()[..beacon().len() - 4];
        assert_eq!(&received[..8], &[mpdu.len() as u8, (mpdu.len() >> 8) as u8, 0, 1, 2, 1, 0, 0]);
        assert_eq!(&received[8..], mpdu);
        let empty = ipc::buffer(5, &[]); step(service, phy, events);
        error(empty, wlanhw::Error::Empty);
        assert_eq!(service.dropped(), 0);
    });
}

#[test]
fn a_scans_probe_request_admits_nothing_and_the_joins_first_frame_does() { serve(true, Vec::new(), |service, phy, receive, tables, board, events| {
    let id = ipc::buffer(9, &country("RU", &all(20))); step(service, phy, events); info(id);
    // The scan's probe request on channel 8: refused, and the scan goes on receive-only.
    let id = ipc::words(2, 8); step(service, phy, events); tune(service, phy, receive, tables, board);
    assert_eq!(ipc::response(id).unwrap().0, [0, 0]);
    let mut probe = management(4); probe[4..10].fill(0xff); probe[16..22].fill(0xff);
    let id = ipc::buffer(4, &send_frame(&probe)); step(service, phy, events);
    error(id, wlanhw::Error::Failed); assert!(service.take_ask().is_none());
    let id = ipc::words(2, 9); step(service, phy, events); assert_eq!(tune(service, phy, receive, tables, board), 9);
    assert_eq!(ipc::response(id).unwrap().0, [0, 0]);
    // The join: back to 8 with the network's filter; its authentication asks admission there, and the filter is kept.
    let id = ipc::words(2, 8); step(service, phy, events); tune(service, phy, receive, tables, board); ipc::response(id).unwrap();
    let id = ipc::buffer(3, &filter(false, 0x021122334455)); step(service, phy, events); assert_eq!(ipc::response(id).unwrap().0, [0, 0]);
    let id = ipc::buffer(4, &send_frame(&management(11))); step(service, phy, events);
    error(id, wlanhw::Error::Busy);
    assert!(matches!(service.take_ask(), Some(Ask::Admit(8))));
    let kept = service.filter(); assert!(!kept.all_beacons()); assert_eq!(kept.bssid(), BSSID);
    assert_eq!(kept.own(), board.identity.mac);
});}

#[test]
fn a_scan_ends_with_its_tunes_and_the_frames_it_kept() { serve(true, vec![beacon()], |service, phy, receive, tables, board, events| {
    // The filter starts as a scan's: the beacon is kept.
    let wait = ipc::words(6, 0); step(service, phy, events); assert_eq!(ipc::response(wait).unwrap().0, [1 << 16, 0]);
    for ch in [6, 3] {
        let id = ipc::words(2, ch); step(service, phy, events); tune(service, phy, receive, tables, board);
        assert_eq!(ipc::response(id).unwrap().0, [0, 0]);
    }
    let id = ipc::buffer(3, &filter(false, 0)); step(service, phy, events); assert_eq!(ipc::response(id).unwrap().0, [0, 0]);
    assert_eq!(events.as_slice(), &[Event::Scanned { tunes: 2, longest_us: 600, kept: 1, dropped: 0 }]);
    // A filter that ends no scan logs none; the next scan counts from its own start.
    let id = ipc::buffer(3, &filter(false, 0)); step(service, phy, events); ipc::response(id).unwrap();
    let id = ipc::buffer(3, &filter(true, 0)); step(service, phy, events); ipc::response(id).unwrap();
    let id = ipc::words(2, 2); step(service, phy, events); tune(service, phy, receive, tables, board); ipc::response(id).unwrap();
    let id = ipc::buffer(3, &filter(false, 0)); step(service, phy, events); ipc::response(id).unwrap();
    assert_eq!(events[1..], [Event::Scanned { tunes: 1, longest_us: 200, kept: 0, dropped: 0 }]);
});}

#[test]
fn a_networks_filter_narrows_the_hardware_and_a_scan_opens_it_again() { serve(true, Vec::new(), |service, phy, _, _, _, events| {
    let set = |service: &mut ReceiveOnly, phy: &mut Live<'_, '_, '_, '_>, events: &mut Vec<Event>, all: bool, bssid: u64| {
        let id = ipc::buffer(3, &filter(all, bssid)); step(service, phy, events); assert_eq!(ipc::response(id).unwrap().0, [0, 0]);
    };
    set(service, phy, events, true, 0); assert_eq!(service.take_narrowing(), None, "the start's filter is already a scan's");
    set(service, phy, events, false, 0x021122334455); assert_eq!(service.take_narrowing(), Some(true));
    set(service, phy, events, false, 0x021122334455); assert_eq!(service.take_narrowing(), None);
    set(service, phy, events, true, 0); assert_eq!(service.take_narrowing(), Some(false));
});}
