#![no_std]
#![no_main]
//! Test-only console program (QEMU `cameras` suite, 158-DRV-0082): opens one camera of the video gateway through the
//! client the shell lends for REQUEST_CAMERA, reads its frames for a time and says what came: how many, their numbers,
//! their size, and for a test pattern how far its counter runs ahead of the frame numbers. Never packaged into the
//! normal OS; the programs that show a camera are APP's (`camera`, `wm`).
//! Usage: videostub CAMERA SECONDS [WxH] [RATE]   (default 320x240 at 10 a second)
use mind::abi::*;
use mind::idl::video as idl;
use mind::ipc::Endpoint;
use mind::mem::Pages;

mind::request!(REQUEST_CAMERA);

const CAMERA: Endpoint = Endpoint(SLOT_CAMERA);

fn fail(message: core::fmt::Arguments) -> ! { mind::println!("VIDEO STUB: {}", message); mind::process::exit_with(1) }

// The words of the arguments: camera, seconds, an optional WxH and an optional rate.
fn options(args: &str) -> Option<(u8, u32, u16, u16, u8)> {
    let mut words = args.split_whitespace();
    let (camera, seconds) = (words.next()?.parse().ok()?, words.next()?.parse().ok()?);
    let (mut width, mut height, mut rate) = (320, 240, 10);
    if let Some(size) = words.next() { let (w, h) = size.split_once(['x', 'X'])?; (width, height) = (w.parse().ok()?, h.parse().ok()?); }
    if let Some(r) = words.next() { rate = r.parse().ok()?; }
    words.next().is_none().then_some((camera, seconds, width, height, rate))
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let Some((camera, seconds, width, height, rate)) = options(mind::process::args_str()) else { fail(format_args!("usage: videostub CAMERA SECONDS [WxH] [RATE]")) };
    if mind::dev::cap_info(SLOT_CAMERA).0 != CAP_KIND_ENDPOINT { fail(format_args!("NO CAMERA WAS LENT")) }
    let cameras = idl::cameras(CAMERA).unwrap_or_default();
    let listed = cameras.as_slice().get(camera as usize).copied();
    let opened = Pages::new(width as usize * height as usize * 4).ok_or(idl::Error::NoMemory);
    let started = mind::time::monotonic_ns();
    match idl::open(CAMERA, camera, width, height, rate) {
        Ok(Ok(())) => {}
        Ok(Err(e)) => fail(format_args!("OPEN CAMERA {} {}X{} AT {}/S: {:?} ({} CAMERAS LISTED)", camera, width, height, rate, e, cameras.len())),
        Err(e) => fail(format_args!("THE GATEWAY DOES NOT ANSWER: {:?}", e)),
    }
    let name = listed.map(|c| c.name).unwrap_or_default();
    mind::println!("VIDEO STUB: OPENED CAMERA {} \"{}\" {}X{} AT {}/S", camera, name.as_str(), width, height, rate);
    let (Ok(pages), synthetic) = (opened, listed.is_some_and(|c| c.synthetic)) else { let _ = idl::close(CAMERA); fail(format_args!("NO MEMORY FOR A FRAME")) };
    let Ok(cap) = pages.share() else { let _ = idl::close(CAMERA); fail(format_args!("THE FRAME BUFFER CANNOT BE SHARED")) };
    let bytes = width as usize * height as usize * 4;
    let (mut frames, mut first, mut last, mut first_ns, mut sized, mut ahead) = (0u32, 0u32, 0u32, 0u64, true, None::<Option<u32>>);
    let until = started + seconds as u64 * 1_000_000_000;
    while mind::time::monotonic_ns() < until {
        let lent = mind::ipc::mint(cap, CAP_READ | CAP_WRITE | CAP_GRANT, 0, 0);
        let read = lent.map(|lent| idl::read(CAMERA, bytes as u32, lent));
        let _ = mind::ipc::revoke(cap);
        let sequence = match read {
            Ok(Ok(Ok(sequence))) => sequence,
            other => { let _ = idl::close(CAMERA); fail(format_args!("READ AFTER {} FRAMES: {:?}", frames, other)) }
        };
        if frames == 0 { first = sequence; first_ns = mind::time::monotonic_ns() - started; }
        frames += 1;
        last = sequence;
        let frame = idl::frame(CAMERA).ok().and_then(|f| f.ok());
        sized &= frame.is_some_and(|f| f.width == width && f.height == height && f.bytes as usize == bytes);
        if synthetic {
            // The pattern's counter against the frame's number: the same offset in every frame, or None once it is not.
            let pixels = unsafe { core::slice::from_raw_parts(pages.as_slice().as_ptr() as *const u32, bytes / 4) };
            let w = width as usize;
            let offset = mind::video::counter(w, height as usize, |x, y| pixels[y * w + x]).map(|c| c.wrapping_sub(sequence));
            ahead = Some(match ahead { None => offset, Some(seen) => seen.filter(|&s| offset == Some(s)) });
        }
    }
    let _ = idl::close(CAMERA);
    let elapsed = (mind::time::monotonic_ns() - started) / 1_000_000;
    let counter = match ahead { Some(Some(n)) => CounterText::Ahead(n), Some(None) => CounterText::Differs, None => CounterText::None };
    mind::println!("VIDEO STUB: CAMERA {}: {} FRAMES IN {} MS, SEQUENCE {}..{}, FIRST AFTER {} MS, {}X{}{}{}", camera, frames, elapsed, first, last,
        first_ns / 1_000_000, width, height, if sized { "" } else { ", A FRAME OF ANOTHER SIZE" }, counter);
    // A program's output goes with it: a shell's `logs` of a run in the background gets 3 s to read the last line.
    mind::time::sleep(3000);
    mind::process::exit_with(0)
}

// What the counters of a test pattern's frames said.
enum CounterText { None, Ahead(u32), Differs }

impl core::fmt::Display for CounterText {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self { CounterText::None => Ok(()), CounterText::Ahead(n) => write!(f, ", THE COUNTER {} AHEAD", n), CounterText::Differs => write!(f, ", THE COUNTER OFF IN A FRAME") }
    }
}
