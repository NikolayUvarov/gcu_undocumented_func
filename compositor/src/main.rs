#![no_std]
#![no_main]
// Ring 3 compositor: copies changed pixels of the active screen into the GOP framebuffer. Between frames it serves
// idl/display.wit on its service endpoint: the mode, and a sealed copy of the screen in front (the shell's
// `screenshot`, issue 086; `record`, issue 093). While the screen is being captured a red dot in its top right corner
// says so (issue 165), and while a camera stream is open a green camera mark left of it (issue 158); left of that, the
// marks of who can reach the machine from outside, the link and the Effector agent (800-KRN-0156): drawn here, on the
// framebuffer only, so no capture holds them and no program can hide them. It logs the screen, each screen put in
// front and its first seconds of frames (213-KRN-0098), and whose screen is in front (750-KRN-0192).
use mind::abi::{pixel_to_device, BootInfo, ERR_TIMEOUT, PIXEL_BGR, PIXEL_BITMASK, PIXEL_RGB, SLOT_MEM};
use mind::dev::{compositor_pull, compositor_pull_frame, compositor_release, Frame};
use mind::display::{remote_center, remote_of, remote_pixel, Remote};
use mind::idl::display::{self, Error, Mode, RemoteState, Request};
use mind::idl::wire;
use mind::ipc::Endpoint;
use mind::mem::{Mapping, Pages};

const RECEIVED_CAP: usize = 10;
const FRAME_MS: u32 = 15;
// A screen whose task has not waited for this long is copied while it runs, torn or not (000-KRN-0086).
const FORCE_MS: usize = 250;
// The dot stays this long after a capture: a recording at one frame a second keeps it lit.
const DOT_MS: usize = 1500;
const DOT_RADIUS: usize = 6;
// A remote mark stays this long after its last call: a service that stops or hangs loses it (800-KRN-0156).
const REMOTE_MS: usize = 3000;

// The pixels of the dot: a red disc in the top right corner of a `width`-wide screen.
fn dot(width: usize, height: usize) -> impl Iterator<Item = (usize, usize, u32)> {
    let (cx, cy, r) = (width.saturating_sub(4 * DOT_RADIUS), 3 * DOT_RADIUS, DOT_RADIUS as isize);
    (cy - DOT_RADIUS..=cy + DOT_RADIUS).flat_map(move |y| (cx.saturating_sub(DOT_RADIUS)..=cx + DOT_RADIUS).map(move |x| (x, y)))
        .filter(move |&(x, y)| x < width && y < height && (x as isize - cx as isize).pow(2) + (y as isize - cy as isize).pow(2) <= r * r)
        .map(|(x, y)| (x, y, 0x00E0_2020))
}

// What the log says of the screen in front (213-KRN-0098): its first frame, then each second it changed in WATCH_MS.
const WATCH_MS: usize = 10_000;
const LOG_LINES: u32 = 24; // in all, so screens that change often cannot fill the system log's ring

#[derive(Default)]
struct Watch { screens: u32, since: usize, second: usize, frames: u32, longest: usize, held: usize, written: usize, lines: u32 }

// One line of the watch while lines are left.
fn say(lines: &mut u32, line: core::fmt::Arguments) { if *lines < LOG_LINES { *lines += 1; mind::println!("{}", line); } }

// Pixels of the visible screen that are not black, in the shadow copy.
#[inline(never)]
fn lit(info: &BootInfo, shadow: *const u32) -> usize {
    (0..info.height).map(|y| (y * info.stride..y * info.stride + info.width).filter(|&i| unsafe { core::ptr::read(shadow.add(i)) } & 0xFF_FFFF != 0).count()).sum()
}

// The camera mark left of the dot: a green body 16 × 12 with a dark lens in its middle.
fn camera(width: usize, height: usize) -> impl Iterator<Item = (usize, usize, u32)> {
    let (cx, cy) = (width.saturating_sub(8 * DOT_RADIUS), 3 * DOT_RADIUS);
    (cy - 6..cy + 6).flat_map(move |y| (cx.saturating_sub(8)..cx + 8).map(move |x| (x, y))).filter(move |&(x, y)| x < width && y < height)
        .map(move |(x, y)| (x, y, if (x as isize - cx as isize).pow(2) + (y as isize - cy as isize).pow(2) <= 9 { 0x0010_2010 } else { 0x0020_C040 }))
}

// A mark over the framebuffer while `lit`, else the screen under it put back from the shadow copy. Kept out of the
// main loop, whose copy of the screen it would otherwise slow down (about half again on one CPU under TCG).
#[inline(never)]
fn mark(info: &BootInfo, gop: *mut u32, shadow: *const u32, valid: bool, lit: bool, shape: impl Iterator<Item = (usize, usize, u32)>) {
    let device = |pixel: u32| if info.pixel_format == PIXEL_BGR { pixel } else { pixel_to_device(pixel, info.pixel_format, info.pixel_masks) };
    for (x, y, color) in shape {
        let at = y * info.stride + x;
        let pixel = if lit { color } else if valid { unsafe { core::ptr::read(shadow.add(at)) } } else { 0 };
        unsafe { core::ptr::write_volatile(gop.add(at), device(pixel)); }
    }
}

// A remote mark's pixels: grey, yellow or red for what its path may do, in its fixed place.
fn remote(mark: Remote, state: RemoteState, width: usize, height: usize) -> impl Iterator<Item = (usize, usize, u32)> {
    let ((cx, cy), r) = (remote_center(mark, width, DOT_RADIUS), DOT_RADIUS as isize);
    let color = match state { RemoteState::Changing => 0x00E0_2020, RemoteState::Reading => 0x00E0_C020, RemoteState::Unconnected => 0x0080_8080 };
    (cy - DOT_RADIUS..=cy + DOT_RADIUS).flat_map(move |y| (cx.saturating_sub(DOT_RADIUS)..=cx + DOT_RADIUS).map(move |x| (x, y)))
        .filter(move |&(x, y)| x < width && y < height && remote_pixel(mark, x as isize - cx as isize, y as isize - cy as isize, r))
        .map(move |(x, y)| (x, y, color))
}

// What a request lit: the capture dot, the camera mark or a remote mark.
enum Lit { Capture, Camera, Remote(Remote, RemoteState) }

// One display.wit request: the mode, a sealed read-only copy of what `source` shows, the camera mark, or the caller's
// remote mark, which its badge from init names (MC-3.11).
fn serve(info: &BootInfo, source: Option<&Mapping>, request: Request, call: wire::Call, badge: u16, sender: u64, lines: &mut u32) -> Option<Lit> {
    let lit = match request {
        Request::Capture => Some(Lit::Capture), Request::Camera => Some(Lit::Camera), Request::Mode => None,
        Request::Remote { state } => remote_of(badge).map(|mark| Lit::Remote(mark, state)),
    };
    let _ = match request {
        Request::Camera => display::reply_camera(call),
        Request::Remote { .. } if lit.is_none() => {
            say(lines, format_args!("[COMPOSITOR] REMOTE MARK REFUSED: PID {} (BADGE {}) HAS NO MARK (MC-3.11)", sender, badge));
            display::reply_remote(call, Err(Error::Denied))
        }
        Request::Remote { .. } => display::reply_remote(call, Ok(())),
        Request::Mode => display::reply_mode(call, &Mode { width: info.width as u32, height: info.height as u32, stride: info.stride as u32 }),
        Request::Capture => {
            let bytes = info.stride * info.height * 4;
            let copy = match source {
                Some(screen) if screen.len() >= bytes => mind::mem::sealed_copy(&screen.as_slice()[..bytes]).map_err(|_| Error::NoMemory),
                _ => Err(Error::NoScreen),
            };
            display::reply_capture(call, copy)
        }
    };
    lit
}

const SOURCE_SLOT: usize = 9; // the kernel puts the active screen capability here

mind::entry!(main);
fn main(info: &'static BootInfo) {
    let pixels = info.stride * info.height;
    // Mapping errors no longer turn into writes to address usize::MAX.
    let Ok(gop) = Mapping::new(SLOT_MEM) else { mind::println!("[COMPOSITOR] NO FRAMEBUFFER MAPPING"); return };
    let Some(mut shadow) = Pages::new(pixels * 4) else { mind::println!("[COMPOSITOR] NO MEMORY FOR SHADOW"); return };
    let Some(mut spans) = Pages::new(info.height * 8) else { mind::println!("[COMPOSITOR] NO MEMORY FOR SHADOW"); return };
    let (gop, shadow) = (gop.as_ptr::<u32>(), shadow.as_mut_slice().as_mut_ptr() as *mut u32);
    let spans = spans.as_mut_slice().as_mut_ptr() as *mut [u32; 2]; // each row's changed columns, first and last
    let mut source: Option<Mapping> = None;
    let mut valid = false;
    let native = info.pixel_format == PIXEL_BGR; // screens already hold the framebuffer's layout
    let mut captured = None::<usize>; // when the last capture was, while the dot is shown
    let mut filmed = None::<usize>; // when the video gateway last said a stream is open, while the mark is shown
    let mut remotes = [None::<(usize, RemoteState)>; 2]; // the link's and the Effector's: the last call and its state
    let mut remote_lines = 0u32; // the remote marks' changes and refusals logged, LOG_LINES at most
    let mut busy = None::<usize>; // since when the screen changed while its task ran
    let (mut front, mut front_lines) = (0u64, 0u32); // the PID whose screen is in front (750-KRN-0192), and its lines logged
    let mut watch = Watch::default();
    let format = match info.pixel_format { PIXEL_BGR => "BGR", PIXEL_RGB => "RGB", PIXEL_BITMASK => "BIT MASKS", _ => "UNKNOWN" };
    say(&mut watch.lines, format_args!("[COMPOSITOR] SCREEN {}X{}, STRIDE {}, FORMAT {}", info.width, info.height, info.stride, format));
    loop {
        // Whole frames only: a changed screen is copied while its task waits, and the task is held meanwhile; a task
        // found drawing is held at its next wait (000-KRN-0186); one that never waits is copied as it is after FORCE_MS
        // (000-KRN-0086).
        let (mut frame, mut owner) = compositor_pull_frame(SOURCE_SLOT).unwrap_or((Frame::Unchanged, front));
        if frame == Frame::Busy {
            let since = *busy.get_or_insert(mind::time::uptime_ms());
            if mind::time::uptime_ms() - since >= FORCE_MS { (frame, owner) = compositor_pull(SOURCE_SLOT).unwrap_or((Frame::Unchanged, owner)); }
        } else {
            busy = None;
        }
        if owner != front {
            front = owner;
            if front == 0 { say(&mut front_lines, format_args!("[COMPOSITOR] NO SCREEN IN FRONT")); }
            else { say(&mut front_lines, format_args!("[COMPOSITOR] SCREEN OF PID {} IN FRONT", front)); }
        }
        // Released after any dirty copy: a task that waited after a busy answer is held even when FORCE_MS's pull finds it.
        let mut held = frame == Frame::Dirty;
        if frame == Frame::NewSource {
            drop(source.take()); // unmap the old mapping first (shared memory quota)
            source = Mapping::new(SOURCE_SLOT).ok();
            let _ = mind::ipc::drop_cap(SOURCE_SLOT); // the capability is no longer needed: the mapping keeps the memory
        }
        if let (Some(screen), Frame::Dirty | Frame::NewSource) = (source.as_ref(), frame) {
            let screen = screen.as_ptr::<u32>();
            let (start, mut written) = (mind::time::monotonic_ns(), 0usize);
            // The visible width of each row only: the stride may be far wider (4096 for 2880 on the MacBook Pro). Pixels
            // are compared one by one, as a slice comparison is a byte-wise memcmp here (000-KRN-0081).
            // While the task is held its changes go to the shadow only; the framebuffer, far slower to write, is
            // written after the release, so the hold never ends mid-copy (000-KRN-0186).
            for y in 0..info.height {
                let row = y * info.stride;
                let mut span = [info.width as u32, 0];
                for x in 0..info.width {
                    let pixel = unsafe { core::ptr::read_volatile(screen.add(row + x)) };
                    if !valid || pixel != unsafe { core::ptr::read(shadow.add(row + x)) } {
                        unsafe { core::ptr::write(shadow.add(row + x), pixel); }
                        span = [span[0].min(x as u32), x as u32];
                    }
                }
                unsafe { core::ptr::write(spans.add(y), span); }
            }
            let held_ms = if held { (mind::time::monotonic_ns() - start) / 1_000_000 } else { 0 };
            if core::mem::take(&mut held) { let _ = compositor_release(SOURCE_SLOT); }
            for y in 0..info.height {
                let ([first, last], row) = (unsafe { core::ptr::read(spans.add(y)) }, y * info.stride);
                for i in row + first as usize..row + last as usize + 1 {
                    let pixel = unsafe { core::ptr::read(shadow.add(i)) };
                    let device = if native { pixel } else { pixel_to_device(pixel, info.pixel_format, info.pixel_masks) };
                    unsafe { core::ptr::write_volatile(gop.add(i), device); }
                    written += 1;
                }
            }
            valid = true;
            let (now, took) = (mind::time::uptime_ms(), (mind::time::monotonic_ns() - start) / 1_000_000);
            if frame == Frame::NewSource {
                watch.screens += 1; watch.since = now; watch.second = now; watch.frames = 0; watch.longest = 0; watch.held = 0; watch.written = 0;
                let shown = lit(info, shadow);
                say(&mut watch.lines, format_args!("[COMPOSITOR] SCREEN IN FRONT #{}: FIRST FRAME IN {} MS, {} OF {} PIXELS NOT BLACK", watch.screens, took, shown, info.width * info.height));
            } else {
                watch.frames += 1; watch.longest = watch.longest.max(took as usize); watch.held = watch.held.max(held_ms as usize); watch.written += written;
            }
        }
        if held { let _ = compositor_release(SOURCE_SLOT); }
        // Each watched second the screen changed, after the release: its frames, the longest copy, what it shows.
        let now = mind::time::uptime_ms();
        if watch.screens > 0 && now - watch.since < WATCH_MS + 1000 && now - watch.second >= 1000 {
            if watch.written > 0 {
                let shown = lit(info, shadow);
                say(&mut watch.lines, format_args!("[COMPOSITOR] SCREEN IN FRONT #{}: {} FRAMES IN {} MS, LONGEST {} MS (THE TASK HELD {} MS AT MOST), {} PIXELS WRITTEN, {} NOT BLACK", watch.screens, watch.frames, now - watch.second, watch.longest, watch.held, watch.written, shown));
            }
            watch.second = now; watch.frames = 0; watch.longest = 0; watch.held = 0; watch.written = 0;
        }
        // The capture dot: drawn over every frame while captures come, then the screen under it put back once.
        if let Some(at) = captured {
            let capturing = mind::time::uptime_ms() - at < DOT_MS;
            mark(info, gop, shadow, valid, capturing, dot(info.width, info.height));
            if !capturing { captured = None; }
        }
        if let Some(at) = filmed {
            let streaming = mind::time::uptime_ms() - at < DOT_MS;
            mark(info, gop, shadow, valid, streaming, camera(info.width, info.height));
            if !streaming { filmed = None; }
        }
        for (slot, which) in [Remote::Link, Remote::Effector].into_iter().enumerate() {
            if let Some((at, state)) = remotes[slot] {
                let shown = mind::time::uptime_ms() - at < REMOTE_MS;
                mark(info, gop, shadow, valid, shown, remote(which, state, info.width, info.height));
                if !shown { remotes[slot] = None; say(&mut remote_lines, format_args!("[COMPOSITOR] REMOTE MARK {:?}: OFF", which)); }
            }
        }
        // Wait for the next frame, answering requests meanwhile (without a service endpoint: just wait).
        match Endpoint::SERVICE.recv_timeout(RECEIVED_CAP, if frame == Frame::Busy { 1 } else { FRAME_MS }) {
            Ok(request) => match display::decode(&request, RECEIVED_CAP) {
                Ok((decoded, call)) => match serve(info, source.as_ref(), decoded, call, request.badge, request.sender, &mut remote_lines) {
                    Some(Lit::Capture) => captured = Some(mind::time::uptime_ms()),
                    Some(Lit::Camera) => filmed = Some(mind::time::uptime_ms()),
                    Some(Lit::Remote(mark, state)) => {
                        let slot = mark as usize;
                        if remotes[slot].map(|(_, was)| was) != Some(state) { say(&mut remote_lines, format_args!("[COMPOSITOR] REMOTE MARK {:?}: {:?}", mark, state)); }
                        remotes[slot] = Some((mind::time::uptime_ms(), state));
                    }
                    None => {}
                },
                Err(reason) => if request.is_call { let _ = wire::reject(reason); },
            },
            Err(mind::Error::Other(ERR_TIMEOUT)) => {}
            Err(_) => { mind::time::sleep(FRAME_MS as usize); }
        }
    }
}
