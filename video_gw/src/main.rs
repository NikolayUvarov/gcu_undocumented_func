#![no_std]
#![no_main]
extern crate alloc;
// Ring 3 video gateway (issue 158): serves idl/video.wit. Its only client is the shell's, which lends it to a program
// started for a camera (REQUEST_CAMERA, MC-11.4). A camera has one owner at a time. Every open and close is logged with
// who asked, and while a stream is open the compositor shows the camera mark (display.wit `camera`), which no program
// can hide. The cameras: a USB video class camera through usb_host (mind::uvc; its YUY2 frames scaled to the size asked
// for), then the synthetic test pattern of mind::video when the boot disk asks for it with `video/synthetic` (two when
// it begins with `2`). Every USB camera is listed, by its handle's order (158-DRV-0072), and each has a stream of its
// own: two programs show two cameras at once, and a program's `open` of another camera switches it (158-DRV-0082).
mod choices;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use mind::abi::{BootInfo, CAP_KIND_ENDPOINT, ERR_TIMEOUT, SLOT_DEV0, SLOT_DEV1};
use mind::idl::codec::Text;
use mind::idl::{display, video as idl, wire};
use mind::ipc::Endpoint;
use mind::mem::{Mapping, Pages};
use mind::usb::{Host, Interface, BULK_MAX, BULK_STREAM_BYTES, CONTROL_MAX};
use mind::{uvc, video};
use choices::Key;

const RECEIVED_CAP: usize = 9;
const DISPLAY: Endpoint = Endpoint(SLOT_DEV0); // the compositor's client, for the camera mark
const USB: Endpoint = Endpoint(SLOT_DEV1); // usb_host's client for video class interfaces
const MARK_MS: usize = 500; // the mark's heartbeat while a stream is open (the compositor keeps it 1.5 s)
const PUMP_MS: u32 = 5; // a camera's packets are taken this often while its stream is open (a tick, in practice)
const USB_BUFFER: usize = 512 * 1024;
const LOOK_MS: usize = 2000; // a missing camera is looked for again at most this often
const WIDEST: u16 = 640; // the size a camera is offered at: its largest YUY2 frame up to this width
const FIRST_MS: u64 = 5000; // the wait for a camera's first good frame, and later for the next one
const LISTED: usize = 4; // video.wit's list of cameras, the test pattern among them
const LISTED_MODES: usize = 256; // video.wit 1.1's list of a camera's modes
const DESCRIBED: usize = 72; // a configuration descriptor's bytes a log line
const NO_MODE: u16 = 0xFFFF; // `opened`'s index for a pattern drawn at a size none of its modes has
const CLAIMED: usize = 16; // the interfaces one look claims: a camera has a control and a streaming one, some more
const BULK_ROUND_NS: u64 = 20_000_000; // a bulk camera's transfers are taken this long at most a pump

// A UVC camera (its control interface stays claimed): the streaming interface's handle, what its descriptors say, its
// name, what `cameras` says it gives, and its modes as `modes` lists them (158-DRV-0083), each with the probe's indexes.
struct Camera { streaming: u32, info: uvc::Camera, name: Text<24>, offered: (u16, u16, u8), modes: Vec<(uvc::Mode, idl::Mode)> }

// A camera's open stream: the camera (its streaming handle), the frame size it sends, the frames assembled from its
// packets (or bulk transfers), and counts for the log.
struct Live {
    camera: u32, size: uvc::FrameSize, assembler: uvc::Assembler, bulk: Option<uvc::Bulk>, endpoint: u8, setting: u8, interval_ns: u64,
    packets: u64, bytes: u64, damaged: u64, header: [u8; 12], header_len: usize, given: u32, good_ns: u64, opened_ns: u64,
    // Its camera went (or usb_host restarted): no more packets are asked for, and its reader gets not-found.
    gone: bool,
    // A bulk camera's endpoint kept polled by usb_host (usb.wit 1.6), its transfers taken without waiting; else read
    // by waiting transfers (EHCI). Its pumps, and the longest time between two, for the log (158-DRV-0082).
    streamed: bool, pumps: u64, pumped_ns: u64, longest_gap_ns: u64,
}

// The test pattern has a number: `video/synthetic` may ask for two, so the tests have two cameras to open at once.
enum Source { Pattern(u8), Camera(Live) }

impl Source {
    fn gone(&self) -> bool { matches!(self, Source::Camera(live) if live.gone) }
}

// `opened`: what `opened` answers its owner (video.wit 1.1).
struct Stream { owner: u64, width: usize, height: usize, rate: u8, start_ns: u64, sequence: u32, frame: Pages, last: Option<idl::Frame>, source: Source, opened: idl::Opened }

struct Usb { host: Option<Host>, cameras: Vec<Camera>, looked: usize }

impl Stream {
    fn period(&self) -> u64 { 1_000_000_000 / self.rate as u64 }
    fn key(&self) -> Key { match &self.source { Source::Pattern(n) => Key::Pattern(*n), Source::Camera(live) => Key::Camera(live.camera) } }

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
fn pump(host: &mut Host, live: &mut Live) -> mind::Result<usize> {
    let mut good = false;
    let now = mind::time::monotonic_ns();
    if live.pumped_ns != 0 { live.longest_gap_ns = live.longest_gap_ns.max(now - live.pumped_ns); }
    (live.pumps, live.pumped_ns) = (live.pumps + 1, now);
    let written = if live.bulk.is_some() && !live.streamed { pump_bulk(host, live, &mut good)? } else if let Some(bulk) = live.bulk.as_mut() {
        // The transfers usb_host took since the last pump, a page each at most, in order (158-DRV-0082).
        let Live { assembler, packets, bytes, damaged, header, header_len, .. } = live;
        host.isochronous(live.camera, live.endpoint, |data, failed| {
            *packets += 1; *bytes += data.len() as u64;
            if failed { *damaged += 1; bulk.failed(assembler); return; }
            if *header_len == 0 && data.len() >= 2 { *header_len = data.len().min(12); header[..*header_len].copy_from_slice(&data[..*header_len]); }
            if bulk.transfer(assembler, data, BULK_STREAM_BYTES) == Some(true) { good = true; }
        })?
    } else {
        host.isochronous(live.camera, live.endpoint, |packet, damaged| {
            live.packets += 1; live.bytes += packet.len() as u64;
            if damaged { live.damaged += 1; }
            if live.header_len == 0 && packet.len() >= 2 { live.header_len = packet.len().min(12); live.header[..live.header_len].copy_from_slice(&packet[..live.header_len]); }
            if live.assembler.feed(packet, damaged) == Some(true) { good = true; }
        })?
    };
    if good {
        if live.good_ns == 0 { mind::println!("[VIDEO] FIRST FRAME AFTER {} MS: {} PACKETS, {} BYTES", (mind::time::monotonic_ns() - live.opened_ns) / 1_000_000, live.packets, live.bytes); }
        live.good_ns = mind::time::monotonic_ns();
    }
    Ok(written)
}

// A bulk camera's transfers where usb_host keeps none queued (EHCI, 158-DRV-0072): until a frame ends, or for
// BULK_ROUND_NS. usb_host waits for each, and the camera streams from the commit on at its sensor's pace, so a transfer
// waits for its bytes to be made; a second camera beside it starves (158-DRV-0082). A transfer that times out or fails
// (usb_host resets the endpoint) drops its payload; Err only when the camera or usb_host is gone.
fn pump_bulk(host: &mut Host, live: &mut Live, good: &mut bool) -> mind::Result<usize> {
    let started = mind::time::monotonic_ns();
    let limit = BULK_MAX.min(host.buffer().len());
    let mut taken = 0;
    let Live { bulk: Some(bulk), assembler, camera, endpoint, packets, bytes, damaged, header, header_len, .. } = live else { return Ok(0) };
    loop {
        let asked = bulk.next(limit);
        match host.bulk(*camera, *endpoint, 0, asked) {
            Ok(got) => {
                *packets += 1; *bytes += got as u64; taken += got;
                let data = &host.buffer()[..got];
                if *header_len == 0 && got >= 2 { *header_len = got.min(12); header[..*header_len].copy_from_slice(&data[..*header_len]); }
                if bulk.transfer(assembler, data, asked) == Some(true) { *good = true; }
            }
            Err(mind::Error::Other(ERR_TIMEOUT)) | Err(mind::Error::Invalid) => { bulk.failed(assembler); *damaged += 1; break; }
            Err(e) => return Err(e),
        }
        if *good || mind::time::monotonic_ns() - started > BULK_ROUND_NS { break; }
    }
    Ok(taken)
}

// A stream's pumps and the longest time between two, for the log: usb_host's queue for it must outlast that.
fn timing(live: &Live) -> String {
    format!("[VIDEO]   {} PUMPS IN {} MS, THE LONGEST GAP {} MS{}", live.pumps, (mind::time::monotonic_ns() - live.opened_ns) / 1_000_000, live.longest_gap_ns / 1_000_000,
        if live.bulk.is_none() { "" } else if live.streamed { ", BULK KEPT POLLED BY USB_HOST" } else { ", BULK READ BY WAITING TRANSFERS" })
}

// What a camera stream got, for the log.
fn report(live: &Live) -> String {
    format!("{} GOOD FRAMES, {} BROKEN (THE LAST {} BYTES OF {}), {} PACKETS OF {} BYTES, {} DAMAGED, FIRST HEADER {:02X?}",
        live.assembler.good, live.assembler.broken, live.assembler.last_bytes, live.size.exact_bytes().unwrap_or(0), live.packets, live.bytes, live.damaged, &live.header[..live.header_len])
}

impl Usb {
    // Claims the control and streaming interfaces of every camera usb_host has and this gateway does not hold yet, at most
    // every LOOK_MS; the cameras are kept in their handles' order (the controller, then the device).
    fn look(&mut self) {
        let now = mind::time::uptime_ms();
        if self.looked != 0 && now.wrapping_sub(self.looked) < LOOK_MS { return; }
        self.looked = now.max(1);
        let Some(host) = self.host.as_mut() else { return };
        let mut claimed: Vec<(u32, Interface)> = Vec::new();
        while claimed.len() < CLAIMED { match host.claim() { Ok(c) => claimed.push(c), Err(_) => break } }
        // Two interfaces of one device differ only in a handle's low byte.
        let pairs: Vec<(u32, u32, Interface)> = claimed.iter().filter(|(_, i)| i.subclass == uvc::SUBCLASS_CONTROL).filter_map(|&(control, info)| {
            claimed.iter().find(|(h, i)| i.subclass == uvc::SUBCLASS_STREAMING && h & !0xFF == control & !0xFF).map(|&(streaming, _)| (control, streaming, info))
        }).collect();
        for &(handle, _) in &claimed { if !pairs.iter().any(|&(c, s, _)| handle == c || handle == s) { let _ = host.release(handle); } }
        for (control, streaming, info) in pairs {
            if self.cameras.len() >= LISTED { let _ = host.release(control); let _ = host.release(streaming); continue; }
            // One that cannot be read stays claimed, so it is not looked at again until it is plugged in again.
            let Some(described) = describe(host, control, &info) else { continue };
            let offered = described.largest(WIDEST).map_or((0, 0, 0), |f| (f.width, f.height, f.max_rate().min(255) as u8));
            let name = product(host, control).unwrap_or_else(|| Text::new(&format!("USB CAMERA {:04X}:{:04X}", info.vendor, info.product)).unwrap_or_default());
            mind::println!("[VIDEO] CAMERA {:04X}:{:04X} \"{}\": UVC {:X}.{:02X}, INTERFACES {} AND {}, ENDPOINT {:02X} {}; OFFERED AT {}X{} UP TO {}/S",
                info.vendor, info.product, name, described.version >> 8, described.version & 0xFF, described.control, described.streaming, described.endpoint,
                if described.bulk { "BULK" } else { "ISOCHRONOUS" }, offered.0, offered.1, offered.2);
            log_formats(&described);
            let modes = camera_modes(&described, info.speed);
            log_modes(&modes);
            self.cameras.push(Camera { streaming, info: described, name, offered, modes });
            self.cameras.sort_by_key(|c| c.streaming);
            let order: Vec<String> = self.cameras.iter().enumerate().map(|(n, c)| format!("{} \"{}\"", n, c.name)).collect();
            mind::println!("[VIDEO] CAMERAS: {}", order.join(", "));
        }
    }

    fn camera(&self, streaming: u32) -> Option<&Camera> { self.cameras.iter().find(|c| c.streaming == streaming) }

    // A camera is gone (unplugged), or with None every one (usb_host restarted): they are looked for again.
    fn lost(&mut self, streaming: Option<u32>) {
        self.cameras.retain(|c| {
            let gone = streaming.is_none_or(|s| s == c.streaming);
            if gone { mind::println!("[VIDEO] CAMERA \"{}\" GONE", c.name); }
            !gone
        });
        self.looked = 0;
    }
}

// Whether an error from usb_host means it restarted (every handle gone) rather than this camera alone.
fn host_gone(e: mind::Error) -> bool { matches!(e, mind::Error::Peer) }

// The camera's descriptors, read through its control interface (the device's GET_DESCRIPTOR).
fn describe(host: &mut Host, control: u32, info: &Interface) -> Option<uvc::Camera> {
    let read = host.control(control, 0x80, 6, 0x0200, 0, 9).ok().and_then(|_| {
        let total = u16::from_le_bytes([host.buffer()[2], host.buffer()[3]]).clamp(9, CONTROL_MAX as u16);
        host.control(control, 0x80, 6, 0x0200, 0, total).ok()
    });
    let Some(got) = read else { mind::println!("[VIDEO] VIDEO CLASS DEVICE {:04X}:{:04X}: ITS DESCRIPTORS CANNOT BE READ", info.vendor, info.product); return None };
    log_descriptors(&host.buffer()[..got]);
    let camera = uvc::parse(&host.buffer()[..got]);
    if camera.is_none() { mind::println!("[VIDEO] VIDEO CLASS DEVICE {:04X}:{:04X}: NO CONTROL AND STREAMING INTERFACES WITH A FRAME SIZE IN {} BYTES OF DESCRIPTORS", info.vendor, info.product, got); }
    camera
}

// A camera's configuration descriptor in hex, DESCRIBED bytes a line and a little apart (a burst of lines is dropped):
// the host cases of its modes are made from them (158-DRV-0083).
fn log_descriptors(bytes: &[u8]) {
    let lines = bytes.len().div_ceil(DESCRIBED);
    for (n, chunk) in bytes.chunks(DESCRIBED).enumerate() {
        let mut line = String::with_capacity(2 * DESCRIBED);
        for b in chunk { line += &format!("{:02X}", b); }
        mind::println!("[VIDEO]   DESCRIPTORS {}/{}: {}", n + 1, lines, line);
        let _ = mind::time::sleep(2);
    }
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
    // Characters outside ASCII at its end (a camera's name in another script) are left out.
    Text::new(name.trim_end_matches(['?', ' ']))
}

// Every format's frame sizes and their rates (each listed interval's, 158-DRV-0083), and the streaming settings, a line
// each (log records are short).
fn log_formats(camera: &uvc::Camera) {
    for &(index, encoding) in &camera.formats {
        let mut line = format!("[VIDEO]   FORMAT {} {}:", index, match encoding { uvc::Encoding::Yuy2 => String::from("YUY2"), uvc::Encoding::Mjpeg => String::from("MJPEG"), uvc::Encoding::Other(f) => format!("{:02X?}", f) });
        for f in camera.frames.iter().filter(|f| f.format == index) {
            let rates: Vec<String> = f.listed().iter().map(|&i| format!("{}", 10_000_000u32.checked_div(i).unwrap_or(0))).collect();
            let size = format!(" {}X{} ({}/S)", f.width, f.height, rates.join(","));
            if line.len() + size.len() > 150 { mind::println!("{}", line); line = String::from("[VIDEO]    "); }
            line += &size;
        }
        mind::println!("{}", line);
    }
    let mut line = String::from("[VIDEO]   SETTINGS (BYTES A MICROFRAME):");
    for a in &camera.alternates { line += &format!(" {}={}", a.setting, a.bytes()); }
    mind::println!("{}", line);
}

// A camera's modes as `modes` lists them (video.wit 1.1, 158-DRV-0083): the descriptors' order, the first LISTED_MODES,
// each offered or not by choices::offered against what its streaming settings carry at the device's `speed`.
fn camera_modes(camera: &uvc::Camera, speed: u8) -> Vec<(uvc::Mode, idl::Mode)> {
    let capacity = camera.capacity(speed);
    camera.modes().into_iter().take(LISTED_MODES).map(|m| {
        let encoding = match m.encoding { uvc::Encoding::Yuy2 => idl::Encoding::Yuy2, uvc::Encoding::Mjpeg => idl::Encoding::Mjpeg, uvc::Encoding::Other(_) => idl::Encoding::Uncompressed };
        let offered = choices::offered(m.encoding == uvc::Encoding::Yuy2, m.width, m.height, m.rate(), m.bytes_per_second(), capacity);
        (m, idl::Mode { encoding, width: m.width, height: m.height, rate: m.rate().min(255) as u8, interval: m.interval, offered })
    }).collect()
}

// How many modes a camera has and offers, and why the others are not: a line for the log.
fn log_modes(modes: &[(uvc::Mode, idl::Mode)]) {
    let count = |f: &dyn Fn(&idl::Mode) -> bool| modes.iter().filter(|(_, m)| f(m)).count();
    let yuy2 = |m: &idl::Mode| m.encoding == idl::Encoding::Yuy2;
    mind::println!("[VIDEO]   MODES: {}, {} OFFERED; NOT: {} YUY2 BEYOND {}X{} AT {}/S OR THE SETTINGS, {} MJPEG (NOT DECODED YET), {} OTHER", modes.len(),
        count(&|m| m.offered), count(&|m| yuy2(m) && !m.offered), choices::WIDTHS.end(), choices::HEIGHTS.end(), choices::RATES.end(),
        count(&|m| m.encoding == idl::Encoding::Mjpeg), count(&|m| m.encoding == idl::Encoding::Uncompressed));
}

// The modes a test pattern lists (video.wit 1.1): drawn by the gateway, each offered.
fn pattern_modes() -> Vec<idl::Mode> {
    choices::PATTERN_MODES.iter().map(|&(width, height, rate)| idl::Mode { encoding: idl::Encoding::Synthetic, width, height, rate, interval: 10_000_000 / rate as u32, offered: true }).collect()
}

// What `opened` says of a camera stream: the listed mode of the frame size it streams whose interval is nearest the
// committed one, and whether its frames are scaled to the size asked for.
fn camera_opened(number: u8, camera: &Camera, live: &Live, width: u16, height: u16) -> idl::Opened {
    let committed = (live.interval_ns / 100) as u32;
    let found = camera.modes.iter().enumerate().filter(|(_, (m, _))| m.format == live.size.format && m.frame == live.size.frame)
        .min_by_key(|(_, (m, _))| m.interval.abs_diff(committed));
    let (index, mode) = found.map_or((NO_MODE, idl::Mode::default()), |(i, (_, m))| (i as u16, *m));
    idl::Opened { camera: number, index, mode, scaled: (width, height) != (live.size.width, live.size.height) }
}

// What `opened` says of a pattern stream: its listed mode of that size and rate, or NO_MODE for another one it draws.
fn pattern_opened(number: u8, width: u16, height: u16, rate: u8) -> idl::Opened {
    let index = choices::PATTERN_MODES.iter().position(|&m| m == (width, height, rate)).map_or(NO_MODE, |i| i as u16);
    let mode = idl::Mode { encoding: idl::Encoding::Synthetic, width, height, rate, interval: 10_000_000 / rate.max(1) as u32, offered: true };
    idl::Opened { camera: number, index, mode, scaled: false }
}

// Probes and commits the frame size nearest `width` × `height` at `rate`, and selects the setting that carries it.
fn start(host: &mut Host, camera: &Camera, width: u16, height: u16, rate: u8) -> Result<Live, idl::Error> {
    let info = &camera.info;
    let Some(size) = info.choose(width, height).cloned() else { mind::println!("[VIDEO] \"{}\" HAS NO YUY2 FORMAT (MJPEG IS NOT DECODED YET)", camera.name); return Err(idl::Error::Invalid) };
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
    let bytes = size.exact_bytes().unwrap_or(size.max_bytes as usize);
    // A bulk camera has no setting to choose: it streams from the commit on, in payloads of up to its committed size,
    // and usb_host keeps its endpoint polled where it can (usb.wit 1.6), so other cameras' streams do not starve it.
    let streamed = info.bulk && match host.bulk_stream(camera.streaming, info.endpoint, true) {
        Ok(()) => true,
        Err(e) => { mind::println!("[VIDEO] \"{}\": NOT KEPT POLLED BY USB_HOST ({:?}): READ BY WAITING TRANSFERS, A CAMERA BESIDE IT MAY STARVE", camera.name, e); false }
    };
    let (endpoint, setting, bulk) = if info.bulk { (info.endpoint, 0, Some(uvc::Bulk::new(answer.max_payload()))) } else {
        let Some(alternate) = info.alternate(answer.max_payload()) else { mind::println!("[VIDEO] \"{}\": NO ISOCHRONOUS SETTING", camera.name); return Err(idl::Error::Invalid) };
        host.select(camera.streaming, alternate.setting).map_err(|e| failed("SETTING", e))?;
        (alternate.endpoint, alternate.setting, None)
    };
    let now = mind::time::monotonic_ns();
    Ok(Live { camera: camera.streaming, assembler: uvc::Assembler::new(bytes, true), bulk, size, endpoint, setting, interval_ns: answer.interval().max(1) as u64 * 100,
        packets: 0, bytes: 0, damaged: 0, header: [0; 12], header_len: 0, given: 0, good_ns: 0, opened_ns: now, gone: false, streamed, pumps: 0, pumped_ns: 0, longest_gap_ns: 0 })
}

// The camera's stream stopped, with what it got: an isochronous camera back to setting 0; a bulk one's endpoint
// cleared of its halt, as Linux stops a camera of one setting. A camera gone is not asked.
fn stop(usb: &mut Usb, live: &Live) -> String {
    if let Some(host) = usb.host.as_mut().filter(|_| !live.gone) {
        if live.bulk.is_some() {
            if live.streamed { if let Err(e) = host.bulk_stream(live.camera, live.endpoint, false) { mind::println!("[VIDEO] THE BULK STREAM DID NOT STOP ({:?})", e); } }
            if let Err(e) = host.control(live.camera, 0x02, 1, 0, live.endpoint as u16, 0) { mind::println!("[VIDEO] THE BULK ENDPOINT'S CLEAR-HALT FAILED ({:?})", e); }
        } else if let Err(e) = host.select(live.camera, 0) { usb.lost(if host_gone(e) { None } else { Some(live.camera) }); }
    }
    mind::println!("{}", timing(live));
    report(live)
}

// Every camera stream's packets are taken, but `except`'s and those whose camera went: between requests, so
// usb_host's queue does not fill, and while one reader waits for its frame, so the other cameras' streams go on
// (158-DRV-0082). True when a camera stream is open.
fn pump_all(streams: &mut [Stream], except: Option<usize>, usb: &mut Usb) -> bool {
    let (mut any, mut failed) = (false, None);
    for (i, s) in streams.iter_mut().enumerate() {
        let Source::Camera(live) = &mut s.source else { continue };
        any = true;
        if Some(i) == except || live.gone { continue; }
        let Some(host) = usb.host.as_mut() else { continue };
        if let Err(e) = pump(host, live) { mind::println!("[VIDEO] PID {}'S CAMERA STOPPED ANSWERING ({:?}): {}", s.owner, e, report(live)); failed = Some((e, live.camera)); break; }
    }
    if let Some((e, handle)) = failed { lost(streams, usb, e, handle); }
    any
}

// A camera stream's packets could not be taken: its camera went, or with usb_host restarted every one. Their streams
// stay their owners' until closed, and take no more packets.
fn lost(streams: &mut [Stream], usb: &mut Usb, e: mind::Error, handle: u32) {
    let all = host_gone(e);
    usb.lost(if all { None } else { Some(handle) });
    for s in streams.iter_mut() { if let Source::Camera(live) = &mut s.source { if all || live.camera == handle { live.gone = true; } } }
}

// The next frame of pattern stream `at`: due at start + (n - 1) / rate; a reader that fell behind gets the latest due
// one. The cameras' streams are pumped while it is not due yet.
fn next_pattern(streams: &mut [Stream], at: usize, usb: &mut Usb) -> idl::Frame {
    let (sequence, due) = streams[at].due();
    // One reading of the clock a pass: a second one already past `due` wrapped the difference into a 60 s sleep.
    loop {
        let now = mind::time::monotonic_ns();
        if now >= due { break; }
        let left = ((due - now) / 1_000_000) as u32 + 1;
        let cameras = pump_all(streams, Some(at), usb);
        let _ = mind::time::sleep(if cameras { left.min(PUMP_MS) } else { left } as usize);
    }
    let s = &mut streams[at];
    let Source::Pattern(n) = s.source else { return idl::Frame::default() };
    let (width, height) = (s.width, s.height);
    video::fill(choices::pattern_frame(n, sequence, s.rate), width, height, s.pixels());
    s.sequence = sequence;
    idl::Frame { sequence, timestamp_us: due / 1000, width: width as u16, height: height as u16, bytes: 0 }
}

// The next frame of camera stream `at` into its pixels: when it is due and a newer picture came, or half a camera
// frame later than the camera's next one was expected; the other cameras' streams are pumped meanwhile. Err(NotFound)
// without a good frame for FIRST_MS, or when the camera is gone.
fn next_camera(streams: &mut [Stream], at: usize, usb: &mut Usb) -> Result<idl::Frame, idl::Error> {
    let (sequence, due) = streams[at].due();
    let Source::Camera(live) = &streams[at].source else { return Err(idl::Error::NotOpen) };
    let handle = live.camera;
    let Some(name) = usb.camera(handle).map(|c| c.name).filter(|_| !live.gone) else { return Err(idl::Error::NotFound) };
    let camera = &name;
    loop {
        let Some(host) = usb.host.as_mut() else { return Err(idl::Error::NotFound) };
        let Source::Camera(live) = &mut streams[at].source else { return Err(idl::Error::NotOpen) };
        let taken = match pump(host, live) {
            Ok(n) => n,
            Err(e) => {
                mind::println!("[VIDEO] \"{}\" STOPPED ANSWERING ({:?}): {}", camera, e, report(live));
                lost(streams, usb, e, handle);
                return Err(idl::Error::NotFound);
            }
        };
        let now = mind::time::monotonic_ns();
        let fresh = live.assembler.good > live.given;
        if now >= due && live.assembler.good > 0 && (fresh || now >= due.max(live.good_ns) + live.interval_ns * 3 / 2) { break; }
        if now - live.good_ns.max(live.opened_ns) > FIRST_MS * 1_000_000 {
            mind::println!("[VIDEO] \"{}\": NO GOOD FRAME FOR {} MS: {}", camera, FIRST_MS, report(live));
            mind::println!("{}", timing(live));
            live.good_ns = now; // the next read waits as long again
            return Err(idl::Error::NotFound);
        }
        pump_all(streams, Some(at), usb);
        if streams[at].source.gone() { return Err(idl::Error::NotFound); }
        if taken == 0 { let _ = mind::time::sleep(1); }
    }
    let stream = &mut streams[at];
    let (width, height) = (stream.width, stream.height);
    let pixels = unsafe { core::slice::from_raw_parts_mut(stream.frame.as_mut_slice().as_mut_ptr() as *mut u32, width * height) };
    let Source::Camera(live) = &mut stream.source else { return Err(idl::Error::NotOpen) };
    live.given = live.assembler.good;
    video::yuy2_scaled(live.assembler.ready(), live.size.width as usize, live.size.height as usize, pixels, width, height);
    stream.sequence = sequence;
    Ok(idl::Frame { sequence, timestamp_us: due / 1000, width: width as u16, height: height as u16, bytes: 0 })
}

// How many test patterns `video/synthetic` on the boot disk asks for: one, or two when its text begins with "2".
fn synthetic() -> usize {
    let first = mind::fs::File::open("video/synthetic").ok().map(|mut file| {
        let mut first = [0u8; 1];
        matches!(file.read(&mut first), Ok(1)).then_some(first[0])
    });
    choices::patterns(first)
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let patterns = synthetic();
    let host = if mind::dev::cap_info(SLOT_DEV1).0 == CAP_KIND_ENDPOINT { Host::with_buffer(USB, USB_BUFFER).ok() } else { None };
    let mut usb = Usb { host, cameras: Vec::new(), looked: 0 };
    usb.look();
    if patterns > 0 { mind::println!("[VIDEO] SYNTHETIC SOURCE: video/synthetic on the boot disk{}", if patterns > 1 { ", TWO PATTERNS" } else { "" }); }
    if usb.cameras.is_empty() && patterns == 0 { mind::println!("[VIDEO] NO CAMERA{}", if usb.host.is_none() { " (NO USB)" } else { " ON USB NOW" }); }
    // A stream for each camera that has an owner (158-DRV-0082): two programs show two cameras at once.
    let mut streams: Vec<Stream> = Vec::new();
    let mut marked = 0usize;
    loop {
        // The mark is lit while a stream is open; a stream whose owner ended is closed.
        while let Some(i) = streams.iter().position(|s| !mind::process::alive(s.owner)) {
            let s = streams.swap_remove(i);
            let got = if let Source::Camera(live) = &s.source { stop(&mut usb, live) } else { String::new() };
            mind::println!("[VIDEO] PID {} ENDED: STREAM CLOSED AFTER {} FRAMES {}", s.owner, s.sequence, got);
        }
        if !streams.is_empty() && mind::time::uptime_ms().wrapping_sub(marked) >= MARK_MS {
            let _ = display::camera(DISPLAY);
            marked = mind::time::uptime_ms();
        }
        let live = pump_all(&mut streams, None, &mut usb);
        let wait = if live { PUMP_MS } else if !streams.is_empty() { MARK_MS as u32 } else { 0 };
        let request = match Endpoint::SERVICE.recv_timeout(RECEIVED_CAP, wait) {
            Ok(request) => request,
            Err(mind::Error::Other(ERR_TIMEOUT)) => continue,
            Err(_) => continue,
        };
        let sender = request.sender;
        let (request, call) = match idl::decode(&request, RECEIVED_CAP) { Ok(decoded) => decoded, Err(reason) => { if request.is_call { let _ = wire::reject(reason); } continue; } };
        let mine = streams.iter().position(|s| s.owner == sender);
        let _ = match request {
            idl::Request::Cameras => {
                usb.look();
                let mut list: [idl::Camera; LISTED] = Default::default();
                let mut count = 0;
                for c in usb.cameras.iter().take(choices::real(usb.cameras.len(), patterns, LISTED)) { list[count] = idl::Camera { name: c.name, width: c.offered.0, height: c.offered.1, rate: c.offered.2, synthetic: false }; count += 1; }
                for n in 0..patterns {
                    let name = if n == 0 { "test pattern" } else { "test pattern 2" };
                    list[count] = idl::Camera { name: Text::new(name).unwrap_or_default(), width: video::MAX_WIDTH as u16, height: video::MAX_HEIGHT as u16, rate: video::MAX_RATE, synthetic: true }; count += 1;
                }
                idl::reply_cameras(call, &list[..count])
            }
            idl::Request::Open { camera, width, height, rate } => {
                usb.look();
                // The cameras in the order `cameras` lists them: the USB ones, then the patterns. Busy while another
                // task owns this camera (video.wit); another camera may stream beside it.
                let handles: Vec<u32> = usb.cameras.iter().map(|c| c.streaming).collect();
                let key = choices::key(camera, &handles, patterns, LISTED);
                let held: Vec<(u64, Key)> = streams.iter().filter(|s| !s.source.gone()).map(|s| (s.owner, s.key())).collect();
                let admitted = choices::admit(&held, sender, key).map_err(|refused| match refused { choices::Refused::NotFound => idl::Error::NotFound, choices::Refused::Busy => idl::Error::Busy });
                let result = if let Err(e) = admitted { Err(e) }
                    else if matches!(key, Some(Key::Pattern(_))) && !video::supported(width as usize, height as usize, rate) { Err(idl::Error::Invalid) }
                    else if matches!(key, Some(Key::Camera(_))) && !choices::opens(width, height, rate as u32) { Err(idl::Error::Invalid) }
                    else {
                        // An earlier stream of the same owner is closed first (`held` and `streams` differ by the gone ones).
                        if let Some(i) = mine { if let Source::Camera(live) = &streams.swap_remove(i).source { let _ = stop(&mut usb, live); } }
                        // The camera's place may have changed if one was lost while the earlier stream stopped.
                        let source = match key {
                            Some(Key::Pattern(n)) => Ok(Source::Pattern(n)),
                            Some(Key::Camera(handle)) => match (usb.host.as_mut(), usb.cameras.iter().find(|c| c.streaming == handle)) {
                                (Some(host), Some(c)) => start(host, c, width, height, rate).map(Source::Camera),
                                _ => Err(idl::Error::NotFound),
                            },
                            None => Err(idl::Error::NotFound),
                        };
                        match (source, Pages::new(width as usize * height as usize * 4)) {
                            (Err(e), _) => Err(e),
                            (Ok(Source::Camera(live)), None) => { let _ = stop(&mut usb, &live); Err(idl::Error::NoMemory) }
                            (Ok(_), None) => Err(idl::Error::NoMemory),
                            (Ok(source), Some(frame)) => {
                                let what = match &source {
                                    Source::Pattern(0) => String::from("test pattern"),
                                    Source::Pattern(_) => String::from("test pattern 2"),
                                    Source::Camera(l) => format!("CAMERA {} \"{}\" (ITS {}X{} YUY2, {})", camera, usb.camera(l.camera).map_or("", |c| c.name.as_str()), l.size.width, l.size.height, if l.bulk.is_some() { String::from("BULK") } else { format!("SETTING {}", l.setting) }),
                                };
                                let opened = match &source {
                                    Source::Pattern(_) => pattern_opened(camera, width, height, rate),
                                    Source::Camera(l) => usb.camera(l.camera).map_or_else(Default::default, |c| camera_opened(camera, c, l, width, height)),
                                };
                                mind::println!("[VIDEO] PID {} OPENED {} {}x{} AT {}/S{}; MODE {}", sender, what, width, height, rate, if streams.is_empty() { String::new() } else { format!(", BESIDE {} MORE", streams.len()) },
                                    if opened.index == NO_MODE { String::from("NONE LISTED") } else { format!("{}{}", opened.index, if opened.scaled { ", SCALED" } else { "" }) });
                                streams.push(Stream { owner: sender, width: width as usize, height: height as usize, rate, start_ns: mind::time::monotonic_ns(), sequence: 0, frame, last: None, source, opened });
                                let _ = display::camera(DISPLAY);
                                marked = mind::time::uptime_ms();
                                Ok(())
                            }
                        }
                    };
                idl::reply_open(call, result)
            }
            idl::Request::Read { capacity, pixels } => {
                let result = match mine {
                    None => Err(idl::Error::NotOpen),
                    Some(i) => match Mapping::new(pixels) {
                        Err(_) => Err(idl::Error::Invalid),
                        Ok(mut out) => {
                            let next = if matches!(streams[i].source, Source::Pattern(_)) { Ok(next_pattern(&mut streams, i, &mut usb)) } else { next_camera(&mut streams, i, &mut usb) };
                            let s = &mut streams[i];
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
                let last = mine.ok_or(idl::Error::NotOpen).and_then(|i| streams[i].last.ok_or(idl::Error::NotOpen));
                idl::reply_frame(call, last.as_ref().map_err(|e| *e))
            }
            idl::Request::Modes { camera } => {
                usb.look();
                let handles: Vec<u32> = usb.cameras.iter().map(|c| c.streaming).collect();
                let listed: Result<Vec<idl::Mode>, idl::Error> = match choices::key(camera, &handles, patterns, LISTED) {
                    Some(Key::Pattern(_)) => Ok(pattern_modes()),
                    Some(Key::Camera(handle)) => usb.camera(handle).map(|c| c.modes.iter().map(|(_, m)| *m).collect()).ok_or(idl::Error::NotFound),
                    None => Err(idl::Error::NotFound),
                };
                idl::reply_modes(call, listed.as_deref().map_err(|e| *e))
            }
            idl::Request::Opened => {
                let opened = mine.map(|i| streams[i].opened).ok_or(idl::Error::NotOpen);
                idl::reply_opened(call, opened.as_ref().map_err(|e| *e))
            }
            idl::Request::Close => {
                let result = if let Some(i) = mine {
                    let s = streams.swap_remove(i);
                    let got = if let Source::Camera(live) = &s.source { stop(&mut usb, live) } else { String::new() };
                    mind::println!("[VIDEO] PID {} CLOSED AFTER {} FRAMES {}", sender, s.sequence, got);
                    Ok(())
                } else { Err(idl::Error::NotOpen) };
                idl::reply_close(call, result)
            }
        };
    }
}
