//! Host tests of libmind/src/uvc.rs (issue 158): a camera's descriptors, the probe, the choice of a frame size, rate and
//! alternate setting, and frames assembled from payloads with loss and errors.
extern crate alloc;
#[path = "../libmind/src/uvc.rs"]
mod uvc;
use uvc::*;

fn le16(v: u16) -> [u8; 2] { v.to_le_bytes() }
fn le32(v: u32) -> [u8; 4] { v.to_le_bytes() }

// A frame descriptor: 640x480 and the like, with a list of intervals or (an empty list) the range min..max by step.
fn frame(subtype: u8, index: u8, width: u16, height: u16, default: u32, list: &[u32], range: Option<(u32, u32, u32)>) -> Vec<u8> {
    let mut d = vec![0, 0x24, subtype, index, 0];
    d.extend(le16(width)); d.extend(le16(height));
    d.extend(le32(1_000_000)); d.extend(le32(100_000_000)); d.extend(le32(width as u32 * height as u32 * 2)); d.extend(le32(default));
    match range {
        Some((min, max, step)) => { d.push(0); d.extend(le32(min)); d.extend(le32(max)); d.extend(le32(step)); }
        None => { d.push(list.len() as u8); for &i in list { d.extend(le32(i)); } }
    }
    d[0] = d.len() as u8;
    d
}

fn interface(number: u8, alternate: u8, endpoints: u8, subclass: u8) -> Vec<u8> { vec![9, 4, number, alternate, endpoints, 0x0E, subclass, 0, 0] }
fn endpoint(address: u8, attributes: u8, packet: u16, interval: u8) -> Vec<u8> { let p = le16(packet); vec![7, 5, address, attributes, p[0], p[1], interval] }

// A camera like a laptop's: UVC 1.0, YUY2 at 640x480 and 320x240, MJPEG at 1280x720, three isochronous alternate settings.
fn camera_descriptor() -> Vec<u8> {
    let mut c = vec![9, 2, 0, 0, 2, 1, 0, 0x80, 0xFA];
    c.extend([8, 0x0B, 0, 2, 0x0E, 3, 0, 2]); // interface association
    c.extend(interface(0, 0, 1, 1));
    c.extend([0x0D, 0x24, 1, 0x00, 0x01, 0x4D, 0, 0x80, 0x8D, 0x5B, 0, 1, 1]); // VC header, UVC 1.00
    c.extend([0x12, 0x24, 2, 1, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0x0E, 0, 0]); // camera terminal
    c.extend([9, 0x24, 3, 2, 1, 1, 0, 3, 0]); // output terminal
    c.extend(endpoint(0x83, 3, 16, 6)); c.extend([5, 0x25, 3, 16, 0]);
    c.extend(interface(1, 0, 0, 2));
    c.extend([0x0E, 0x24, 1, 2, 0, 0, 0x81, 0, 2, 0, 0, 0, 1, 0]); // input header, endpoint 81
    let mut yuy2 = vec![0x1B, 0x24, 4, 1, 2]; yuy2.extend(GUID_YUY2); yuy2.extend([16, 1, 0, 0, 0, 0]);
    c.extend(yuy2);
    c.extend(frame(5, 1, 640, 480, 333_333, &[333_333, 666_666], None));
    c.extend(frame(5, 2, 320, 240, 333_333, &[], Some((333_333, 2_000_000, 333_333))));
    c.extend([0x0B, 0x24, 6, 2, 1, 1, 1, 0, 0, 0, 0]); // MJPEG format
    c.extend(frame(7, 1, 1280, 720, 333_333, &[333_333], None));
    c.extend([6, 0x24, 0x0D, 1, 1, 4]); // colour matching
    for (alternate, packet) in [(1u8, 128u16), (2, 0x1400), (3, 0x0B20)] { c.extend(interface(1, alternate, 1, 2)); c.extend(endpoint(0x81, 5, packet, 1)); }
    c.extend([9, 4, 2, 0, 0, 0xFE, 1, 1, 0]); // another class's interface after it (DFU)
    let total = c.len() as u16; c[2..4].copy_from_slice(&le16(total));
    c
}

#[test]
fn the_descriptors_give_the_interfaces_formats_frames_and_alternates() {
    let camera = parse(&camera_descriptor()).expect("a camera");
    assert_eq!((camera.version, camera.control, camera.streaming, camera.endpoint, camera.bulk), (0x0100, 0, 1, 0x81, false));
    assert_eq!(camera.formats, vec![(1, Encoding::Yuy2), (2, Encoding::Mjpeg)]);
    let sizes: Vec<_> = camera.frames.iter().map(|f| (f.format, f.frame, f.encoding, f.width, f.height)).collect();
    assert_eq!(sizes, vec![(1, 1, Encoding::Yuy2, 640, 480), (1, 2, Encoding::Yuy2, 320, 240), (2, 1, Encoding::Mjpeg, 1280, 720)]);
    assert_eq!((camera.frames[0].intervals.clone(), camera.frames[0].step), (vec![333_333, 666_666], 0));
    assert_eq!((camera.frames[1].intervals.clone(), camera.frames[1].step), (vec![333_333, 2_000_000], 333_333));
    assert_eq!(camera.frames[0].exact_bytes(), Some(640 * 480 * 2));
    assert_eq!(camera.frames[2].exact_bytes(), None);
    let alternates: Vec<_> = camera.alternates.iter().map(|a| (a.setting, a.endpoint, a.bytes())).collect();
    assert_eq!(alternates, vec![(1, 0x81, 128), (2, 0x81, 3072), (3, 0x81, 1600)]);
}

#[test]
fn not_a_camera() {
    assert_eq!(parse(&[]), None);
    assert_eq!(parse(&[9, 2, 18, 0, 1, 1, 0, 0x80, 0xFA, 9, 4, 0, 0, 1, 3, 1, 1, 0]), None, "a keyboard");
    let mut cut = camera_descriptor();
    cut.truncate(120); // the frames are gone
    assert_eq!(parse(&cut), None);
    let mut broken = camera_descriptor();
    broken[9] = 0; // a zero length stops the walk
    assert_eq!(parse(&broken), None);
}

#[test]
fn a_bulk_camera_streams_through_alternate_setting_zero() {
    let mut c = camera_descriptor();
    let at = c.windows(9).position(|w| w == interface(1, 0, 0, 2).as_slice()).unwrap();
    c[at + 4] = 1;
    let header = at + 9;
    let bulk = endpoint(0x82, 2, 512, 0);
    let tail = c.split_off(header);
    c.extend(bulk); c.extend(tail);
    let camera = parse(&c).unwrap();
    assert!(camera.bulk);
    assert_eq!(camera.endpoint, 0x81, "the input header after it names the endpoint");
}

#[test]
fn the_frame_size_holds_the_picture_asked_for_and_is_yuy2() {
    let camera = parse(&camera_descriptor()).unwrap();
    let pick = |w, h| camera.choose(w, h).map(|f| (f.width, f.height));
    assert_eq!(pick(320, 240), Some((320, 240)));
    assert_eq!(pick(100, 80), Some((320, 240)), "the smallest that holds it");
    assert_eq!(pick(400, 300), Some((640, 480)));
    assert_eq!(pick(1280, 720), Some((640, 480)), "the largest YUY2, not the MJPEG one");
    assert_eq!(camera.largest(640).map(|f| f.width), Some(640));
    assert_eq!(camera.largest(400).map(|f| f.width), Some(320));
    assert_eq!(camera.largest(100).map(|f| f.width), Some(320), "the smallest when all are wider");
}

#[test]
fn the_rate_asked_for_or_the_nearest_faster() {
    let camera = parse(&camera_descriptor()).unwrap();
    let (listed, range) = (&camera.frames[0], &camera.frames[1]);
    assert_eq!(listed.interval(1_000_000), 666_666, "10/s: 15/s is the slowest that is fast enough");
    assert_eq!(listed.interval(333_333), 333_333);
    assert_eq!(listed.interval(100_000), 333_333, "100/s: the fastest there is");
    assert_eq!(range.interval(1_000_000), 999_999);
    assert_eq!(range.interval(5_000_000), 1_999_998);
    assert_eq!(range.interval(1), 333_333);
    assert_eq!((listed.max_rate(), range.max_rate()), (30, 30));
}

#[test]
fn the_alternate_setting_carries_the_payload() {
    let camera = parse(&camera_descriptor()).unwrap();
    let setting = |bytes| camera.alternate(bytes).map(|a| a.setting);
    assert_eq!(setting(0), Some(1));
    assert_eq!(setting(1000), Some(3), "1600 bytes: the least that carries 1000");
    assert_eq!(setting(3000), Some(2));
    assert_eq!(setting(5000), Some(2), "none carries it: the largest");
    assert_eq!(Camera::default().alternate(100), None);
}

#[test]
fn the_probe_by_version_and_its_answer() {
    assert_eq!((probe_length(0x0100), probe_length(0x0110), probe_length(0x0150)), (26, 34, 48));
    assert_eq!((SET_CUR, GET_CUR, PROBE, COMMIT), (0x01, 0x81, 1, 2), "the requests and controls of UVC 1.5's tables A-8 and A-9");
    let probe = Probe::new(0x0100, 1, 2, 666_666);
    assert_eq!(probe.as_bytes().len(), 26);
    assert_eq!((probe.bytes[0], probe.format(), probe.frame(), probe.interval()), (1, 1, 2, 666_666));
    let mut answer = probe.bytes;
    answer[18..22].copy_from_slice(&le32(153_600)); answer[22..26].copy_from_slice(&le32(3072));
    let answer = Probe::from_bytes(&answer[..34]);
    assert_eq!((answer.as_bytes().len(), answer.max_frame(), answer.max_payload(), answer.format()), (34, 153_600, 3072, 1));
}

fn payload(info: u8, data: &[u8]) -> Vec<u8> { let mut p = vec![2, info]; p.extend(data); p }

#[test]
fn frames_end_at_eof_or_a_new_frame_id() {
    let mut a = Assembler::new(8, true);
    assert_eq!(a.feed(&payload(0, &[1, 2, 3, 4]), false), None);
    assert_eq!(a.feed(&payload(EOF, &[5, 6, 7, 8]), false), Some(true));
    assert_eq!(a.ready(), &[1, 2, 3, 4, 5, 6, 7, 8]);
    // No EOF: the next frame's ID ends it.
    assert_eq!(a.feed(&payload(FID, &[9, 9, 9, 9]), false), None);
    assert_eq!(a.feed(&payload(FID, &[8, 8, 8, 8]), false), None);
    assert_eq!(a.feed(&payload(0, &[7, 7]), false), Some(true));
    assert_eq!(a.ready(), &[9, 9, 9, 9, 8, 8, 8, 8]);
    // That frame is short: broken, and the good one before it stays ready.
    assert_eq!(a.feed(&payload(FID, &[1; 8]), false), Some(false));
    assert_eq!(a.ready(), &[9, 9, 9, 9, 8, 8, 8, 8]);
    assert_eq!(a.feed(&payload(FID | EOF, &[]), false), Some(true));
    assert_eq!(a.ready(), &[1; 8]);
    assert_eq!((a.good, a.broken), (3, 1));
}

#[test]
fn errors_lost_payloads_and_odd_headers() {
    let mut a = Assembler::new(4, true);
    assert_eq!(a.feed(&[], false), None, "an empty packet");
    assert_eq!(a.feed(&[2], false), None);
    assert_eq!(a.feed(&payload(ERR, &[1, 2]), false), None);
    assert_eq!(a.feed(&payload(EOF, &[3, 4]), false), Some(false), "an error in the frame");
    assert_eq!(a.feed(&payload(FID, &[1, 2]), true), None);
    assert_eq!(a.feed(&payload(FID | EOF, &[3, 4]), false), Some(false), "damaged on the bus");
    assert_eq!(a.feed(&payload(0, &[1, 2]), false), None);
    assert_eq!(a.feed(&[9, 0, 1, 2], false), None, "a header longer than the payload");
    assert_eq!(a.feed(&payload(EOF, &[3, 4]), false), Some(false));
    assert_eq!(a.feed(&payload(FID, &[1, 2, 3]), false), None);
    assert_eq!(a.feed(&payload(FID | EOF, &[4, 5]), false), Some(false), "more bytes than a frame has");
    // A header of 12 bytes (PTS and SCR) before the data.
    let mut long = vec![12, EOF | 0x0C]; long.extend([0; 10]); long.extend([5, 6, 7, 8]);
    assert_eq!(a.feed(&long, false), Some(true));
    assert_eq!(a.ready(), &[5, 6, 7, 8]);
    assert_eq!((a.good, a.broken, a.last_bytes), (1, 4, 4));
}

#[test]
fn a_frame_of_any_size_for_motion_jpeg() {
    let mut a = Assembler::new(100, false);
    assert_eq!(a.feed(&payload(EOF, &[0xFF, 0xD8, 0xFF, 0xD9]), false), Some(true));
    assert_eq!(a.ready().len(), 4);
    assert_eq!(a.feed(&payload(FID | EOF, &[]), false), None, "a new ID with nothing in the frame before it ends nothing");
    assert_eq!(a.good, 1);
}

// 158-DRV-0072: a bulk camera's payloads span transfers; the header is at a payload's start only.
#[test]
fn bulk_payloads_span_transfers_and_end_short_or_at_their_size() {
    // Payloads of up to 6 bytes: a 2-byte header and 4 of data.
    let mut a = Assembler::new(8, true);
    let mut b = Bulk::new(6);
    assert_eq!(b.next(4), 4);
    assert_eq!(b.transfer(&mut a, &[2, 0, 1, 2], 4), None); // the first part of a payload, full
    assert_eq!(b.next(4), 2); // no more than the payload has left
    assert_eq!(b.transfer(&mut a, &[3, 4], 2), None); // the payload reached its size: ended, no EOF
    assert_eq!(b.next(64), 6);
    assert_eq!(b.transfer(&mut a, &[2, EOF, 5, 6, 7, 8], 6), Some(true)); // a whole payload with EOF ends the frame
    assert_eq!(a.ready(), &[1, 2, 3, 4, 5, 6, 7, 8]);
    // A short transfer ends its payload early; the next transfer starts with a header.
    let mut b = Bulk::new(1000);
    assert_eq!(b.transfer(&mut a, &[2, FID, 9, 9, 9, 9], 64), None);
    assert_eq!(b.transfer(&mut a, &[2, FID | EOF, 9, 9, 9, 9], 64), Some(true));
    assert_eq!(a.good, 2);
}

#[test]
fn bulk_payloads_split_anywhere_and_a_failed_transfer_spoils_its_frame() {
    // The data split across three transfers of 3 bytes, then a short one, like 512-byte packets.
    let mut a = Assembler::new(8, true);
    let mut b = Bulk::new(100);
    assert_eq!(b.transfer(&mut a, &[2, 0, 1], 3), None);
    assert_eq!(b.transfer(&mut a, &[2, 3, 4], 3), None);
    assert_eq!(b.transfer(&mut a, &[5, 6, 7], 3), None);
    assert_eq!(b.transfer(&mut a, &[8], 3), None); // short: the payload ends, no EOF yet
    assert_eq!(b.transfer(&mut a, &[2, EOF], 64), Some(true)); // an empty payload with EOF ends the frame
    assert_eq!(a.ready(), &[1, 2, 3, 4, 5, 6, 7, 8]);
    // A transfer fails mid-payload: the frame is broken, and the next transfer is read as a new payload's start.
    assert_eq!(b.transfer(&mut a, &[2, FID, 1, 2, 3], 5), None);
    b.failed(&mut a);
    assert_eq!(b.transfer(&mut a, &[2, FID | EOF, 4, 5, 6, 7, 8], 64), Some(false));
    assert_eq!((a.good, a.broken), (1, 1));
    // The frame ID's change ends the frame before it; a zero-length transfer between payloads carries nothing.
    assert_eq!(b.transfer(&mut a, &[], 64), None);
    assert_eq!(b.transfer(&mut a, &[2, 0, 1, 2, 3, 4, 5, 6, 7, 8], 64), None);
    assert_eq!(b.transfer(&mut a, &[2, FID, 1], 64), Some(true));
    // A bad header at a payload's start spoils the frame.
    let mut a = Assembler::new(2, true);
    let mut b = Bulk::new(10);
    assert_eq!(b.transfer(&mut a, &[9, 0, 1], 3), None);
    assert_eq!(b.transfer(&mut a, &[2, EOF, 1, 2], 64), Some(false));
}

#[test]
fn a_bulk_camera_is_parsed_with_its_endpoint_and_no_settings() {
    let mut c = vec![9, 2, 0, 0, 2, 1, 0, 0x80, 0xFA];
    c.extend(interface(0, 0, 1, 1));
    c.extend([13, 0x24, 1, 0x00, 0x01, 13, 0, 0x80, 0x8D, 0x5B, 0, 1, 1]); // VC header, UVC 1.00
    c.extend(interface(1, 0, 1, 2));
    c.extend([14, 0x24, 1, 1, 0, 0, 0x81, 0, 0, 0, 0, 0, 0, 0]); // VS input header, endpoint 81
    let mut format = vec![27, 0x24, 4, 1, 1];
    format.extend(GUID_YUY2); format.extend([16, 1, 0, 0, 0, 0]);
    c.extend(format);
    c.extend(frame(5, 1, 640, 480, 400_000, &[400_000], None));
    c.extend(endpoint(0x81, 2, 512, 0)); // bulk IN in setting 0
    let len = c.len() as u16; c[2..4].copy_from_slice(&le16(len));
    let cam = parse(&c).expect("a bulk camera");
    assert!(cam.bulk);
    assert_eq!(cam.endpoint, 0x81);
    assert!(cam.alternates.is_empty());
    assert_eq!(cam.choose(640, 480).map(|f| (f.width, f.height)), Some((640, 480)));
}
