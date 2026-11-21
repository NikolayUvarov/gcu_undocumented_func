#![no_std]
#![no_main]
//! Test-only console program (QEMU `cameras` suite, 158-DRV-0082 and 0083): opens a camera of the video gateway through
//! the client the shell lends for REQUEST_CAMERA, reads its frames for a time and says what came: how many, their
//! numbers, their size, the mode the camera sends, and for a test pattern how far its counter runs ahead of the frame
//! numbers. It lists a camera's modes, and opens each offered one in turn. Never packaged into the normal OS; the
//! programs that show a camera are APP's (`camera`, `wm`).
//! Usage: videostub CAMERA SECONDS [WxH] [RATE]   (default 320x240 at 10 a second)
//!        videostub modes CAMERA
//!        videostub step CAMERA SECONDS            (each offered mode for SECONDS)
use mind::abi::*;
use mind::idl::video as idl;
use mind::ipc::Endpoint;
use mind::mem::Pages;

mind::request!(REQUEST_CAMERA);

const CAMERA: Endpoint = Endpoint(SLOT_CAMERA);
const USAGE: &str = "usage: videostub CAMERA SECONDS [WxH] [RATE] | modes CAMERA | step CAMERA SECONDS";

fn fail(message: core::fmt::Arguments) -> ! { mind::println!("VIDEO STUB: {}", message); mind::process::exit_with(1) }

// The words of a read's arguments: camera, seconds, an optional WxH and an optional rate.
fn options(args: &str) -> Option<(u8, u32, u16, u16, u8)> {
    let mut words = args.split_whitespace();
    let (camera, seconds) = (words.next()?.parse().ok()?, words.next()?.parse().ok()?);
    let (mut width, mut height, mut rate) = (320, 240, 10);
    if let Some(size) = words.next() { let (w, h) = size.split_once(['x', 'X'])?; (width, height) = (w.parse().ok()?, h.parse().ok()?); }
    if let Some(r) = words.next() { rate = r.parse().ok()?; }
    words.next().is_none().then_some((camera, seconds, width, height, rate))
}

fn encoding(e: idl::Encoding) -> &'static str {
    match e { idl::Encoding::Yuy2 => "YUY2", idl::Encoding::Mjpeg => "MJPEG", idl::Encoding::Uncompressed => "UNCOMPRESSED", idl::Encoding::Synthetic => "SYNTHETIC" }
}

// What the counters of a test pattern's frames said.
enum Counter { None, Ahead(u32), Differs }

impl core::fmt::Display for Counter {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self { Counter::None => Ok(()), Counter::Ahead(n) => write!(f, ", THE COUNTER {} AHEAD", n), Counter::Differs => write!(f, ", THE COUNTER OFF IN A FRAME") }
    }
}

// What a stream gave: frames, the first and last numbers, the time to the first, and whether every frame had the size.
struct Got { frames: u32, first: u32, last: u32, first_ms: u64, elapsed_ms: u64, sized: bool, counter: Counter }

// Camera `camera` opened at `width` × `height` and `rate`, read for `seconds` and closed. Err: the open's refusal.
fn stream(camera: u8, seconds: u32, width: u16, height: u16, rate: u8, synthetic: bool, name: &str) -> Result<Got, idl::Error> {
    let bytes = width as usize * height as usize * 4;
    let Some(pages) = Pages::new(bytes) else { fail(format_args!("NO MEMORY FOR A FRAME")) };
    let Ok(cap) = pages.share() else { fail(format_args!("THE FRAME BUFFER CANNOT BE SHARED")) };
    let started = mind::time::monotonic_ns();
    match idl::open(CAMERA, camera, width, height, rate) {
        Ok(Ok(())) => {}
        Ok(Err(e)) => return Err(e),
        Err(e) => fail(format_args!("THE GATEWAY DOES NOT ANSWER: {:?}", e)),
    }
    match idl::opened(CAMERA) {
        Ok(Ok(o)) => mind::println!("VIDEO STUB: OPENED CAMERA {} \"{}\" {}X{} AT {}/S: MODE {}, {} {}X{} AT {}/S{}", camera, name, width, height, rate,
            o.index, encoding(o.mode.encoding), o.mode.width, o.mode.height, o.mode.rate, if o.scaled { ", SCALED" } else { "" }),
        other => mind::println!("VIDEO STUB: OPENED CAMERA {} \"{}\" {}X{} AT {}/S; OPENED: {:?}", camera, name, width, height, rate, other),
    }
    let mut got = Got { frames: 0, first: 0, last: 0, first_ms: 0, elapsed_ms: 0, sized: true, counter: Counter::None };
    let mut ahead = None::<Option<u32>>;
    let until = started + seconds as u64 * 1_000_000_000;
    while mind::time::monotonic_ns() < until {
        let lent = mind::ipc::mint(cap, CAP_READ | CAP_WRITE | CAP_GRANT, 0, 0);
        let read = lent.map(|lent| idl::read(CAMERA, bytes as u32, lent));
        let _ = mind::ipc::revoke(cap);
        let sequence = match read {
            Ok(Ok(Ok(sequence))) => sequence,
            other => { let _ = idl::close(CAMERA); fail(format_args!("READ AFTER {} FRAMES: {:?}", got.frames, other)) }
        };
        if got.frames == 0 { got.first = sequence; got.first_ms = (mind::time::monotonic_ns() - started) / 1_000_000; }
        got.frames += 1;
        got.last = sequence;
        let frame = idl::frame(CAMERA).ok().and_then(|f| f.ok());
        got.sized &= frame.is_some_and(|f| f.width == width && f.height == height && f.bytes as usize == bytes);
        if synthetic {
            // The pattern's counter against the frame's number: the same offset in every frame, or None once it is not.
            let pixels = unsafe { core::slice::from_raw_parts(pages.as_slice().as_ptr() as *const u32, bytes / 4) };
            let w = width as usize;
            let offset = mind::video::counter(w, height as usize, |x, y| pixels[y * w + x]).map(|c| c.wrapping_sub(sequence));
            ahead = Some(match ahead { None => offset, Some(seen) => seen.filter(|&s| offset == Some(s)) });
        }
    }
    let _ = idl::close(CAMERA);
    got.elapsed_ms = (mind::time::monotonic_ns() - started) / 1_000_000;
    got.counter = match ahead { Some(Some(n)) => Counter::Ahead(n), Some(None) => Counter::Differs, None => Counter::None };
    Ok(got)
}

fn said(camera: u8, width: u16, height: u16, got: &Got) {
    mind::println!("VIDEO STUB: CAMERA {}: {} FRAMES IN {} MS, SEQUENCE {}..{}, FIRST AFTER {} MS, {}X{}{}{}", camera, got.frames, got.elapsed_ms, got.first, got.last,
        got.first_ms, width, height, if got.sized { "" } else { ", A FRAME OF ANOTHER SIZE" }, got.counter);
}

fn modes(camera: u8) -> mind::idl::codec::List<idl::Mode, 256> {
    match idl::modes(CAMERA, camera) {
        Ok(Ok(list)) => list,
        Ok(Err(e)) => fail(format_args!("MODES OF CAMERA {}: {:?}", camera, e)),
        Err(e) => fail(format_args!("MODES OF CAMERA {}: THE GATEWAY DOES NOT ANSWER: {:?}", camera, e)),
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let args = mind::process::args_str();
    if mind::dev::cap_info(SLOT_CAMERA).0 != CAP_KIND_ENDPOINT { fail(format_args!("NO CAMERA WAS LENT")) }
    let cameras = idl::cameras(CAMERA).unwrap_or_default();
    let mut words = args.split_whitespace();
    match words.next() {
        Some("modes") => {
            let Some(camera) = words.next().and_then(|w| w.parse().ok()) else { fail(format_args!("{}", USAGE)) };
            let list = modes(camera);
            for (i, m) in list.as_slice().iter().enumerate() {
                mind::println!("VIDEO STUB: MODE {}: {} {}X{} AT {}/S (INTERVAL {}){}", i, encoding(m.encoding), m.width, m.height, m.rate, m.interval, if m.offered { ", OFFERED" } else { "" });
            }
            mind::println!("VIDEO STUB: CAMERA {}: {} MODES, {} OFFERED", camera, list.len(), list.as_slice().iter().filter(|m| m.offered).count());
        }
        Some("step") => {
            let (Some(camera), Some(seconds)) = (words.next().and_then(|w| w.parse::<u8>().ok()), words.next().and_then(|w| w.parse::<u32>().ok())) else { fail(format_args!("{}", USAGE)) };
            let listed = cameras.as_slice().get(camera as usize).copied().unwrap_or_default();
            let (mut opened, mut refused) = (0, 0);
            for (i, m) in modes(camera).as_slice().iter().enumerate().filter(|(_, m)| m.offered) {
                match stream(camera, seconds, m.width, m.height, m.rate, listed.synthetic, listed.name.as_str()) {
                    Ok(got) => { opened += 1; mind::println!("VIDEO STUB: MODE {}: {}X{} AT {}/S", i, m.width, m.height, m.rate); said(camera, m.width, m.height, &got) }
                    Err(e) => { refused += 1; mind::println!("VIDEO STUB: MODE {}: {}X{} AT {}/S REFUSED: {:?}", i, m.width, m.height, m.rate, e) }
                }
            }
            mind::println!("VIDEO STUB: CAMERA {}: {} OFFERED MODES OPENED, {} REFUSED", camera, opened, refused);
        }
        _ => {
            let Some((camera, seconds, width, height, rate)) = options(args) else { fail(format_args!("{}", USAGE)) };
            let listed = cameras.as_slice().get(camera as usize).copied();
            let name = listed.map(|c| c.name).unwrap_or_default();
            match stream(camera, seconds, width, height, rate, listed.is_some_and(|c| c.synthetic), name.as_str()) {
                Ok(got) => said(camera, width, height, &got),
                Err(e) => fail(format_args!("OPEN CAMERA {} {}X{} AT {}/S: {:?} ({} CAMERAS LISTED)", camera, width, height, rate, e, cameras.len())),
            }
        }
    }
    // A program's output goes with it: a shell's `logs` of a run in the background gets 3 s to read the last line.
    mind::time::sleep(3000);
    mind::process::exit_with(0)
}
