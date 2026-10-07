#![no_std]
#![no_main]
// Ring 3 video gateway (issue 158): serves idl/video.wit. Its only client is the shell's, which lends it to a program
// the user let use a camera (REQUEST_CAMERA, MC-11.4). A camera has one owner at a time. Every open and close is
// logged with who asked, and while a stream is open the compositor shows the camera mark (display.wit `camera`), which
// no program can hide. Without a camera driver yet (UVC is the next step) it serves the synthetic test pattern of
// mind::video, and only when the boot disk asks for it with `video/synthetic`.
use mind::abi::{BootInfo, ERR_TIMEOUT, SLOT_DEV0};
use mind::idl::codec::Text;
use mind::idl::{display, video as idl, wire};
use mind::ipc::Endpoint;
use mind::mem::{Mapping, Pages};
use mind::video;

const RECEIVED_CAP: usize = 9;
const DISPLAY: Endpoint = Endpoint(SLOT_DEV0); // the compositor's client, for the camera mark
const MARK_MS: usize = 500; // the mark's heartbeat while a stream is open (the compositor keeps it 1.5 s)

struct Stream { owner: u64, width: usize, height: usize, rate: u8, start_ns: u64, sequence: u32, frame: Pages, last: Option<idl::Frame> }

impl Stream {
    // The next frame: due at start + (n - 1) / rate; a reader that fell behind gets the latest due one (the numbers skip).
    fn next(&mut self) -> idl::Frame {
        let period = 1_000_000_000 / self.rate as u64;
        let now = mind::time::monotonic_ns();
        let latest = ((now - self.start_ns) / period) as u32 + 1;
        let sequence = (self.sequence + 1).max(latest);
        let due = self.start_ns + (sequence - 1) as u64 * period;
        // One reading of the clock a pass: a second one already past `due` wrapped the difference into a 60 s sleep.
        loop {
            let now = mind::time::monotonic_ns();
            if now >= due { break; }
            let _ = mind::time::sleep(((due - now) / 1_000_000) as usize + 1);
        }
        let pixels = unsafe { core::slice::from_raw_parts_mut(self.frame.as_mut_slice().as_mut_ptr() as *mut u32, self.width * self.height) };
        video::fill(sequence, self.width, self.height, pixels);
        self.sequence = sequence;
        idl::Frame { sequence, timestamp_us: due / 1000, width: self.width as u16, height: self.height as u16, bytes: 0 }
    }
}

fn synthetic() -> bool { mind::fs::File::open("video/synthetic").is_ok() }

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let test_source = synthetic();
    if test_source { mind::println!("[VIDEO] SYNTHETIC SOURCE: video/synthetic on the boot disk"); } else { mind::println!("[VIDEO] NO CAMERA"); }
    let mut stream: Option<Stream> = None;
    let mut marked = 0usize;
    loop {
        // The mark is lit while a stream is open; a stream whose owner ended is closed.
        if stream.as_ref().is_some_and(|s| !mind::process::alive(s.owner)) {
            let s = stream.take().unwrap();
            mind::println!("[VIDEO] PID {} ENDED: STREAM CLOSED AFTER {} FRAMES", s.owner, s.sequence);
        }
        if stream.is_some() && mind::time::uptime_ms().wrapping_sub(marked) >= MARK_MS {
            let _ = display::camera(DISPLAY);
            marked = mind::time::uptime_ms();
        }
        let request = match Endpoint::SERVICE.recv_timeout(RECEIVED_CAP, if stream.is_some() { MARK_MS as u32 } else { 0 }) {
            Ok(request) => request,
            Err(mind::Error::Other(ERR_TIMEOUT)) => continue,
            Err(_) => continue,
        };
        let sender = request.sender;
        let (request, call) = match idl::decode(&request, RECEIVED_CAP) { Ok(decoded) => decoded, Err(reason) => { if request.is_call { let _ = wire::reject(reason); } continue; } };
        let mine = stream.as_ref().is_some_and(|s| s.owner == sender);
        let _ = match request {
            idl::Request::Cameras => {
                let camera = idl::Camera { name: Text::new("test pattern").unwrap_or_default(), width: video::MAX_WIDTH as u16, height: video::MAX_HEIGHT as u16, rate: video::MAX_RATE, synthetic: true };
                idl::reply_cameras(call, if test_source { core::slice::from_ref(&camera) } else { &[] })
            }
            idl::Request::Open { camera, width, height, rate } => {
                let result = if camera != 0 || !test_source { Err(idl::Error::NotFound) }
                    else if stream.as_ref().is_some_and(|s| s.owner != sender) { Err(idl::Error::Busy) }
                    else if !video::supported(width as usize, height as usize, rate) { Err(idl::Error::Invalid) }
                    else {
                        match Pages::new(width as usize * height as usize * 4) {
                            None => Err(idl::Error::NoMemory),
                            Some(frame) => {
                                mind::println!("[VIDEO] PID {} OPENED test pattern {}x{} AT {}/S", sender, width, height, rate);
                                stream = Some(Stream { owner: sender, width: width as usize, height: height as usize, rate, start_ns: mind::time::monotonic_ns(), sequence: 0, frame, last: None });
                                let _ = display::camera(DISPLAY);
                                marked = mind::time::uptime_ms();
                                Ok(())
                            }
                        }
                    };
                idl::reply_open(call, result)
            }
            idl::Request::Read { capacity, pixels } => {
                let result = match stream.as_mut().filter(|_| mine) {
                    None => Err(idl::Error::NotOpen),
                    Some(s) => match Mapping::new(pixels) {
                        Err(_) => Err(idl::Error::Invalid),
                        Ok(mut out) => {
                            let mut frame = s.next();
                            let bytes = (capacity as usize).min(out.len()).min(s.width * s.height * 4);
                            out.as_mut_slice()[..bytes].copy_from_slice(&s.frame.as_mut_slice()[..bytes]);
                            frame.bytes = bytes as u32;
                            s.last = Some(frame);
                            Ok(frame.sequence)
                        }
                    },
                };
                idl::reply_read(call, result)
            }
            idl::Request::Frame => {
                let last = stream.as_ref().filter(|_| mine).ok_or(idl::Error::NotOpen).and_then(|s| s.last.ok_or(idl::Error::NotOpen));
                idl::reply_frame(call, last.as_ref().map_err(|e| *e))
            }
            idl::Request::Close => {
                let result = if mine {
                    let s = stream.take().unwrap();
                    mind::println!("[VIDEO] PID {} CLOSED AFTER {} FRAMES", sender, s.sequence);
                    Ok(())
                } else { Err(idl::Error::NotOpen) };
                idl::reply_close(call, result)
            }
        };
    }
}
