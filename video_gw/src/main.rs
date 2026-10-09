#![no_std]
#![no_main]
extern crate alloc;
// Ring 3 video gateway (issue 158): serves idl/video.wit. Its only client is the shell's, which lends it to a program
// started for a camera (REQUEST_CAMERA, MC-11.4). A camera has one owner at a time. Every open and close is logged with
// who asked, and while a stream is open the compositor shows the camera mark (display.wit `camera`), which no program
// can hide. The cameras: a USB video class camera through usb_host (mind::uvc; its YUY2 frames scaled to the size asked
// for), then the synthetic test pattern of mind::video when the boot disk asks for it with `video/synthetic`.
use alloc::format;
use alloc::string::String;
use mind::abi::{BootInfo, CAP_KIND_ENDPOINT, ERR_TIMEOUT, SLOT_DEV0, SLOT_DEV1};
use mind::idl::codec::Text;
use mind::idl::{display, video as idl, wire};
use mind::ipc::Endpoint;
use mind::mem::{Mapping, Pages};
use mind::usb::{Host, Interface, CONTROL_MAX};
use mind::{uvc, video};

const RECEIVED_CAP: usize = 9;
const DISPLAY: Endpoint = Endpoint(SLOT_DEV0); // the compositor's client, for the camera mark
const USB: Endpoint = Endpoint(SLOT_DEV1); // usb_host's client for video class interfaces
const MARK_MS: usize = 500; // the mark's heartbeat while a stream is open (the compositor keeps it 1.5 s)
const PUMP_MS: u32 = 5; // a camera's packets are taken this often while its stream is open (a tick, in practice)
const USB_BUFFER: usize = 512 * 1024;
const LOOK_MS: usize = 2000; // a missing camera is looked for again at most this often
const WIDEST: u16 = 640; // the size a camera is offered at: its largest YUY2 frame up to this width
const FIRST_MS: u64 = 5000; // the wait for a camera's first good frame, and later for the next one

// A UVC camera (its control interface stays claimed): the streaming interface's handle, what its descriptors say, its
// name and what is offered.
struct Camera { streaming: u32, info: uvc::Camera, name: Text<24>, offered: (u16, u16, u8) }

// A camera's open stream: the frame size it sends, the frames assembled from its packets, and counts for the log.
struct Live {
    size: uvc::FrameSize, assembler: uvc::Assembler, endpoint: u8, setting: u8, interval_ns: u64,
    packets: u64, bytes: u64, damaged: u64, header: [u8; 12], header_len: usize, given: u32, good_ns: u64, opened_ns: u64,
}

enum Source { Pattern, Camera(Live) }

struct Stream { owner: u64, width: usize, height: usize, rate: u8, start_ns: u64, sequence: u32, frame: Pages, last: Option<idl::Frame>, source: Source }

struct Usb { host: Option<Host>, camera: Option<Camera>, looked: usize }

impl Stream {
    fn period(&self) -> u64 { 1_000_000_000 / self.rate as u64 }

    // The next frame of the pattern: due at start + (n - 1) / rate; a reader that fell behind gets the latest due one.
    fn next_pattern(&mut self) -> idl::Frame {
        let (sequence, due) = self.due();
        // One reading of the clock a pass: a second one already past `due` wrapped the difference into a 60 s sleep.
        loop {
            let now = mind::time::monotonic_ns();
            if now >= due { break; }
            let _ = mind::time::sleep(((due - now) / 1_000_000) as usize + 1);
        }
        video::fill(sequence, self.width, self.height, self.pixels());
        self.sequence = sequence;
        idl::Frame { sequence, timestamp_us: due / 1000, width: self.width as u16, height: self.height as u16, bytes: 0 }
    }

    fn due(&self) -> (u32, u64) {
        let latest = ((mind::time::monotonic_ns() - self.start_ns) / self.period()) as u32 + 1;
        let sequence = (self.sequence + 1).max(latest);
        (sequence, self.start_ns + (sequence - 1) as u64 * self.period())
    }

    fn pixels(&mut self) -> &mut [u32] {
        unsafe { core::slice::from_raw_parts_mut(self.frame.as_mut_slice().as_mut_ptr() as *mut u32, self.width * self.height) }
    }
}

// The camera's packets into its frames; Ok(bytes taken), Err once the camera or usb_host is gone.
fn pump(host: &mut Host, camera: &Camera, live: &mut Live) -> mind::Result<usize> {
    let mut good = false;
    let written = host.isochronous(camera.streaming, live.endpoint, |packet, damaged| {
        live.packets += 1; live.bytes += packet.len() as u64;
        if damaged { live.damaged += 1; }
        if live.header_len == 0 && packet.len() >= 2 { live.header_len = packet.len().min(12); live.header[..live.header_len].copy_from_slice(&packet[..live.header_len]); }
        if live.assembler.feed(packet, damaged) == Some(true) { good = true; }
    })?;
    if good {
        if live.good_ns == 0 { mind::println!("[VIDEO] FIRST FRAME AFTER {} MS: {} PACKETS, {} BYTES", (mind::time::monotonic_ns() - live.opened_ns) / 1_000_000, live.packets, live.bytes); }
        live.good_ns = mind::time::monotonic_ns();
    }
    Ok(written)
}

// What a camera stream got, for the log.
fn report(live: &Live) -> String {
    format!("{} GOOD FRAMES, {} BROKEN (THE LAST {} BYTES OF {}), {} PACKETS OF {} BYTES, {} DAMAGED, FIRST HEADER {:02X?}",
        live.assembler.good, live.assembler.broken, live.assembler.last_bytes, live.size.exact_bytes().unwrap_or(0), live.packets, live.bytes, live.damaged, &live.header[..live.header_len])
}

impl Usb {
    // Claims a camera's control and streaming interfaces if usb_host has one and none is held; at most every LOOK_MS.
    fn look(&mut self) {
        let now = mind::time::uptime_ms();
        if self.camera.is_some() || (self.looked != 0 && now.wrapping_sub(self.looked) < LOOK_MS) { return; }
        self.looked = now.max(1);
        let Some(host) = self.host.as_mut() else { return };
        let mut claimed: [Option<(u32, Interface)>; 8] = [None; 8];
        for slot in claimed.iter_mut() { match host.claim() { Ok(c) => *slot = Some(c), Err(_) => break } }
        let all = || claimed.iter().flatten();
        // Two interfaces of one device differ only in a handle's low byte.
        let pair = all().find(|(_, i)| i.subclass == uvc::SUBCLASS_CONTROL).and_then(|&(control, info)| {
            all().find(|(h, i)| i.subclass == uvc::SUBCLASS_STREAMING && h & !0xFF == control & !0xFF).map(|&(streaming, _)| (control, streaming, info))
        });
        for &(handle, _) in all() { if pair.is_none_or(|(c, s, _)| handle != c && handle != s) { let _ = host.release(handle); } }
        let Some((control, streaming, info)) = pair else { return };
        // One that cannot be read stays claimed, so it is not looked at again until it is plugged in again.
        let Some(described) = describe(host, control, &info) else { return };
        let offered = described.largest(WIDEST).map_or((0, 0, 0), |f| (f.width, f.height, f.max_rate().min(255) as u8));
        let name = product(host, control).unwrap_or_else(|| Text::new(&format!("USB CAMERA {:04X}:{:04X}", info.vendor, info.product)).unwrap_or_default());
        mind::println!("[VIDEO] CAMERA {:04X}:{:04X} \"{}\": UVC {:X}.{:02X}, INTERFACES {} AND {}, ENDPOINT {:02X} {}; OFFERED AT {}X{} UP TO {}/S",
            info.vendor, info.product, name, described.version >> 8, described.version & 0xFF, described.control, described.streaming, described.endpoint,
            if described.bulk { "BULK" } else { "ISOCHRONOUS" }, offered.0, offered.1, offered.2);
        log_formats(&described);
        self.camera = Some(Camera { streaming, info: described, name, offered });
    }

    // The camera is gone (unplugged, or usb_host restarted): it is looked for again.
    fn lost(&mut self) { if let Some(c) = self.camera.take() { mind::println!("[VIDEO] CAMERA \"{}\" GONE", c.name); } self.looked = 0; }
}

// The camera's descriptors, read through its control interface (the device's GET_DESCRIPTOR).
fn describe(host: &mut Host, control: u32, info: &Interface) -> Option<uvc::Camera> {
    let read = host.control(control, 0x80, 6, 0x0200, 0, 9).ok().and_then(|_| {
        let total = u16::from_le_bytes([host.buffer()[2], host.buffer()[3]]).clamp(9, CONTROL_MAX as u16);
        host.control(control, 0x80, 6, 0x0200, 0, total).ok()
    });
    let Some(got) = read else { mind::println!("[VIDEO] VIDEO CLASS DEVICE {:04X}:{:04X}: ITS DESCRIPTORS CANNOT BE READ", info.vendor, info.product); return None };
    let camera = uvc::parse(&host.buffer()[..got]);
    if camera.is_none() { mind::println!("[VIDEO] VIDEO CLASS DEVICE {:04X}:{:04X}: NO CONTROL AND STREAMING INTERFACES WITH A FRAME SIZE IN {} BYTES OF DESCRIPTORS", info.vendor, info.product, got); }
    camera
}

// The device's product string (the first 24 characters, ASCII).
fn product(host: &mut Host, control: u32) -> Option<Text<24>> {
    host.control(control, 0x80, 6, 0x0100, 0, 18).ok()?;
    let index = host.buffer()[15];
    if index == 0 { return None; }
    let got = host.control(control, 0x80, 6, 0x0300 | index as u16, 0x0409, 255).ok()?;
    let mut name = String::new();
    for pair in host.buffer().get(2..got)?.chunks_exact(2).take(24) {
        let c = u16::from_le_bytes([pair[0], pair[1]]);
        name.push(if (0x20..0x7F).contains(&c) { c as u8 as char } else { '?' });
    }
    Text::new(name.trim_end())
}

// Every format's frame sizes and highest rates, and the streaming settings, a line each (log records are short).
fn log_formats(camera: &uvc::Camera) {
    for &(index, encoding) in &camera.formats {
        let mut line = format!("[VIDEO]   FORMAT {} {}:", index, match encoding { uvc::Encoding::Yuy2 => String::from("YUY2"), uvc::Encoding::Mjpeg => String::from("MJPEG"), uvc::Encoding::Other(f) => format!("{:02X?}", f) });
        for f in camera.frames.iter().filter(|f| f.format == index) {
            if line.len() > 140 { mind::println!("{}", line); line = String::from("[VIDEO]    "); }
            line += &format!(" {}X{} ({}/S)", f.width, f.height, f.max_rate());
        }
        mind::println!("{}", line);
    }
    let mut line = String::from("[VIDEO]   SETTINGS (BYTES A MICROFRAME):");
    for a in &camera.alternates { line += &format!(" {}={}", a.setting, a.bytes()); }
    mind::println!("{}", line);
}

// Probes and commits the frame size nearest `width` × `height` at `rate`, and selects the setting that carries it.
fn start(host: &mut Host, camera: &Camera, width: u16, height: u16, rate: u8) -> Result<Live, idl::Error> {
    let info = &camera.info;
    let Some(size) = info.choose(width, height).cloned() else { mind::println!("[VIDEO] \"{}\" HAS NO YUY2 FORMAT (MJPEG IS NOT DECODED YET)", camera.name); return Err(idl::Error::Invalid) };
    if info.bulk { mind::println!("[VIDEO] \"{}\" STREAMS IN BULK: NOT DONE YET", camera.name); return Err(idl::Error::Invalid); }
    let interval = size.interval(10_000_000 / rate as u32);
    let probe = uvc::Probe::new(info.version, size.format, size.frame, interval);
    let (number, length) = (info.streaming as u16, probe.length as u16);
    let failed = |what: &str, e: mind::Error| { mind::println!("[VIDEO] \"{}\": {} FAILED ({:?})", camera.name, what, e); idl::Error::NotFound };
    host.buffer_mut()[..probe.length].copy_from_slice(probe.as_bytes());
    host.control(camera.streaming, 0x21, uvc::SET_CUR, (uvc::PROBE as u16) << 8, number, length).map_err(|e| failed("PROBE", e))?;
    let got = host.control(camera.streaming, 0xA1, uvc::GET_CUR, (uvc::PROBE as u16) << 8, number, length).map_err(|e| failed("PROBE'S ANSWER", e))?;
    let answer = uvc::Probe::from_bytes(&host.buffer()[..got]);
    host.control(camera.streaming, 0x21, uvc::SET_CUR, (uvc::COMMIT as u16) << 8, number, answer.length as u16).map_err(|e| failed("COMMIT", e))?;
    mind::println!("[VIDEO] \"{}\": ASKED FORMAT {} FRAME {} ({}X{}) INTERVAL {}; COMMITTED FORMAT {} FRAME {} INTERVAL {}, FRAMES UP TO {} BYTES, PAYLOADS UP TO {}",
        camera.name, size.format, size.frame, size.width, size.height, interval, answer.format(), answer.frame(), answer.interval(), answer.max_frame(), answer.max_payload());
    // The camera may have chosen another frame size of the format: its answer counts.
    let size = info.frames.iter().find(|f| f.format == answer.format() && f.frame == answer.frame() && f.encoding == uvc::Encoding::Yuy2).cloned().unwrap_or(size);
    let Some(alternate) = info.alternate(answer.max_payload()) else { mind::println!("[VIDEO] \"{}\": NO ISOCHRONOUS SETTING", camera.name); return Err(idl::Error::Invalid) };
    host.select(camera.streaming, alternate.setting).map_err(|e| failed("SETTING", e))?;
    let bytes = size.exact_bytes().unwrap_or(size.max_bytes as usize);
    let now = mind::time::monotonic_ns();
    Ok(Live { assembler: uvc::Assembler::new(bytes, true), size, endpoint: alternate.endpoint, setting: alternate.setting, interval_ns: answer.interval().max(1) as u64 * 100,
        packets: 0, bytes: 0, damaged: 0, header: [0; 12], header_len: 0, given: 0, good_ns: 0, opened_ns: now })
}

// The camera back to setting 0 (its stream stops), with what the stream got.
fn stop(usb: &mut Usb, live: &Live) -> String {
    if let (Some(host), Some(camera)) = (usb.host.as_mut(), usb.camera.as_ref()) {
        if host.select(camera.streaming, 0).is_err() { usb.lost(); }
    }
    report(live)
}

// The next frame of the camera's stream into the stream's pixels: when it is due and a newer picture came, or half a
// camera frame later than the camera's next one was expected. Err(NotFound) without a good frame for FIRST_MS, or when
// the camera is gone.
fn next_camera(stream: &mut Stream, usb: &mut Usb) -> Result<idl::Frame, idl::Error> {
    let (sequence, due) = stream.due();
    let Source::Camera(live) = &mut stream.source else { return Err(idl::Error::NotOpen) };
    let (Some(host), Some(camera)) = (usb.host.as_mut(), usb.camera.as_ref()) else { return Err(idl::Error::NotFound) };
    loop {
        let taken = match pump(host, camera, live) { Ok(n) => n, Err(e) => { mind::println!("[VIDEO] \"{}\" STOPPED ANSWERING ({:?}): {}", camera.name, e, report(live)); usb.lost(); return Err(idl::Error::NotFound) } };
        let now = mind::time::monotonic_ns();
        let fresh = live.assembler.good > live.given;
        if now >= due && live.assembler.good > 0 && (fresh || now >= due.max(live.good_ns) + live.interval_ns * 3 / 2) { break; }
        if now - live.good_ns.max(live.opened_ns) > FIRST_MS * 1_000_000 {
            mind::println!("[VIDEO] \"{}\": NO GOOD FRAME FOR {} MS: {}", camera.name, FIRST_MS, report(live));
            live.good_ns = now; // the next read waits as long again
            return Err(idl::Error::NotFound);
        }
        if taken == 0 { let _ = mind::time::sleep(1); }
    }
    live.given = live.assembler.good;
    let (width, height, size) = (stream.width, stream.height, (live.size.width as usize, live.size.height as usize));
    let pixels = unsafe { core::slice::from_raw_parts_mut(stream.frame.as_mut_slice().as_mut_ptr() as *mut u32, width * height) };
    let Source::Camera(live) = &stream.source else { return Err(idl::Error::NotOpen) };
    video::yuy2_scaled(live.assembler.ready(), size.0, size.1, pixels, width, height);
    stream.sequence = sequence;
    Ok(idl::Frame { sequence, timestamp_us: due / 1000, width: width as u16, height: height as u16, bytes: 0 })
}

fn synthetic() -> bool { mind::fs::File::open("video/synthetic").is_ok() }

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let test_source = synthetic();
    let host = if mind::dev::cap_info(SLOT_DEV1).0 == CAP_KIND_ENDPOINT { Host::with_buffer(USB, USB_BUFFER).ok() } else { None };
    let mut usb = Usb { host, camera: None, looked: 0 };
    usb.look();
    if test_source { mind::println!("[VIDEO] SYNTHETIC SOURCE: video/synthetic on the boot disk"); }
    if usb.camera.is_none() && !test_source { mind::println!("[VIDEO] NO CAMERA{}", if usb.host.is_none() { " (NO USB)" } else { " ON USB NOW" }); }
    let mut stream: Option<Stream> = None;
    let mut marked = 0usize;
    loop {
        // The mark is lit while a stream is open; a stream whose owner ended is closed.
        if stream.as_ref().is_some_and(|s| !mind::process::alive(s.owner)) {
            let s = stream.take().unwrap();
            let got = if let Source::Camera(live) = &s.source { stop(&mut usb, live) } else { String::new() };
            mind::println!("[VIDEO] PID {} ENDED: STREAM CLOSED AFTER {} FRAMES {}", s.owner, s.sequence, got);
        }
        if stream.is_some() && mind::time::uptime_ms().wrapping_sub(marked) >= MARK_MS {
            let _ = display::camera(DISPLAY);
            marked = mind::time::uptime_ms();
        }
        // A camera's packets are taken between requests too, so usb_host's queue does not fill.
        let live = matches!(stream.as_ref().map(|s| &s.source), Some(Source::Camera(_)));
        if let (true, Some(s)) = (live, stream.as_mut()) {
            if let (Source::Camera(l), Some(host), Some(camera)) = (&mut s.source, usb.host.as_mut(), usb.camera.as_ref()) { if pump(host, camera, l).is_err() { usb.lost(); } }
        }
        let wait = if live { PUMP_MS } else if stream.is_some() { MARK_MS as u32 } else { 0 };
        let request = match Endpoint::SERVICE.recv_timeout(RECEIVED_CAP, wait) {
            Ok(request) => request,
            Err(mind::Error::Other(ERR_TIMEOUT)) => continue,
            Err(_) => continue,
        };
        let sender = request.sender;
        let (request, call) = match idl::decode(&request, RECEIVED_CAP) { Ok(decoded) => decoded, Err(reason) => { if request.is_call { let _ = wire::reject(reason); } continue; } };
        let mine = stream.as_ref().is_some_and(|s| s.owner == sender);
        let _ = match request {
            idl::Request::Cameras => {
                usb.look();
                let mut list: [idl::Camera; 2] = Default::default();
                let mut count = 0;
                if let Some(c) = usb.camera.as_ref() { list[count] = idl::Camera { name: c.name, width: c.offered.0, height: c.offered.1, rate: c.offered.2, synthetic: false }; count += 1; }
                if test_source { list[count] = idl::Camera { name: Text::new("test pattern").unwrap_or_default(), width: video::MAX_WIDTH as u16, height: video::MAX_HEIGHT as u16, rate: video::MAX_RATE, synthetic: true }; count += 1; }
                idl::reply_cameras(call, &list[..count])
            }
            idl::Request::Open { camera, width, height, rate } => {
                if usb.camera.is_none() { usb.look(); }
                // The cameras in the order `cameras` lists them: the USB one, then the pattern.
                let real = usb.camera.is_some();
                let pattern = test_source && camera == real as u8;
                let result = if !(real && camera == 0) && !pattern { Err(idl::Error::NotFound) }
                    else if stream.as_ref().is_some_and(|s| s.owner != sender) { Err(idl::Error::Busy) }
                    else if pattern && !video::supported(width as usize, height as usize, rate) { Err(idl::Error::Invalid) }
                    else if !pattern && !((16..=1280).contains(&width) && (16..=1024).contains(&height) && (1..=60).contains(&rate)) { Err(idl::Error::Invalid) }
                    else {
                        // An earlier stream of the same owner is closed first.
                        if let Some(Stream { source: Source::Camera(live), .. }) = stream.take() { let _ = stop(&mut usb, &live); }
                        let source = if pattern { Ok(Source::Pattern) } else {
                            let (Some(host), Some(c)) = (usb.host.as_mut(), usb.camera.as_ref()) else { unreachable!() };
                            start(host, c, width, height, rate).map(Source::Camera)
                        };
                        match (source, Pages::new(width as usize * height as usize * 4)) {
                            (Err(e), _) => Err(e),
                            (Ok(Source::Camera(live)), None) => { let _ = stop(&mut usb, &live); Err(idl::Error::NoMemory) }
                            (Ok(_), None) => Err(idl::Error::NoMemory),
                            (Ok(source), Some(frame)) => {
                                let what = match &source { Source::Pattern => String::from("test pattern"), Source::Camera(l) => format!("\"{}\" (ITS {}X{} YUY2, SETTING {})", usb.camera.as_ref().map_or("", |c| c.name.as_str()), l.size.width, l.size.height, l.setting) };
                                mind::println!("[VIDEO] PID {} OPENED {} {}x{} AT {}/S", sender, what, width, height, rate);
                                stream = Some(Stream { owner: sender, width: width as usize, height: height as usize, rate, start_ns: mind::time::monotonic_ns(), sequence: 0, frame, last: None, source });
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
                            let next = match s.source { Source::Pattern => Ok(s.next_pattern()), Source::Camera(_) => next_camera(s, &mut usb) };
                            next.map(|mut frame| {
                                let bytes = (capacity as usize).min(out.len()).min(s.width * s.height * 4);
                                out.as_mut_slice()[..bytes].copy_from_slice(&s.frame.as_mut_slice()[..bytes]);
                                frame.bytes = bytes as u32;
                                s.last = Some(frame);
                                frame.sequence
                            })
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
                    let got = if let Source::Camera(live) = &s.source { stop(&mut usb, live) } else { String::new() };
                    mind::println!("[VIDEO] PID {} CLOSED AFTER {} FRAMES {}", sender, s.sequence, got);
                    Ok(())
                } else { Err(idl::Error::NotOpen) };
                idl::reply_close(call, result)
            }
        };
    }
}
