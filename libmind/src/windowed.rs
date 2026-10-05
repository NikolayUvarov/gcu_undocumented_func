//! A program in a window (issue 088). A program started with a client of the window broker in `SLOT_WINDOW` (the
//! window manager lends one to the programs it starts) shows itself in a window instead of on a screen of its own:
//! `mind::tui::Terminal::open` draws into a text window, `pixels` gives a framebuffer in a pixel window. From then on
//! `mind::input` reads the keys the manager queues in the surface, and `mind::time::sleep` waits on the window's wake
//! endpoint, so a key ends it early as it does on a screen. A window the manager closes ends the program.
use crate::abi::{BootInfo, CAP_KIND_ENDPOINT, SLOT_WINDOW, SYSCALL_WAIT, ERR_TIMEOUT};
use crate::idl::window as api;
use crate::ipc::Endpoint;
use crate::mem::Mapping;
use crate::window::{Kind, Surface, STATE_CLOSE};
use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

const BROKER: Endpoint = Endpoint(SLOT_WINDOW);
// Capabilities arrive in a fixed slot; 21 is the shell's broker client and no application uses it.
const RECEIVE: usize = 21;
// Waits are cut into slices this long: a wake message missed while the program was busy costs at most this.
const SLICE_MS: usize = 50;

static BASE: AtomicUsize = AtomicUsize::new(0);
static LEN: AtomicUsize = AtomicUsize::new(0);
static WAKER: AtomicUsize = AtomicUsize::new(0);
static PIXELS: AtomicUsize = AtomicUsize::new(0); // 1: a pixel window, published at every wait
static RESIZE: AtomicU32 = AtomicU32::new(0); // width | height << 16 the manager asked for, not yet taken
static mut INFO: core::mem::MaybeUninit<BootInfo> = core::mem::MaybeUninit::uninit();

/// The launcher lent a broker client and no window is open yet.
pub fn requested() -> bool { !active() && crate::dev::cap_info(SLOT_WINDOW).0 == CAP_KIND_ENDPOINT }

/// The program shows itself in a window.
pub fn active() -> bool { BASE.load(Ordering::Acquire) != 0 }

/// The program's window surface.
pub fn surface() -> Option<Surface> {
    let base = BASE.load(Ordering::Acquire);
    (base != 0).then(|| unsafe { Surface::new(base as *mut u8, LEN.load(Ordering::Acquire)) })
}

/// Opens the program's window: memory for `capacity` (cells or pixels), drawn at `size`, with a title. None if the
/// program was not started in a window or the broker refuses.
pub fn open(kind: Kind, capacity: (usize, usize), size: (usize, usize), title: &str) -> Option<Surface> {
    if !requested() { return None; }
    let wanted = if kind == Kind::Pixels { api::Kind::Pixels } else { api::Kind::Text };
    let id = match api::create(BROKER, wanted, capacity.0 as u16, capacity.1 as u16) {
        Ok(Ok(id)) => id,
        other => { crate::println!("[WINDOW] NOT OPENED: {:?}", other); return None; }
    };
    let mapping = match api::surface(BROKER, id, RECEIVE) { Ok(Ok(())) => Mapping::new(RECEIVE).ok(), _ => None };
    let _ = crate::ipc::drop_cap(RECEIVE); // the mapping keeps the memory
    let Some(mapping) = mapping else { let _ = api::remove(BROKER, id); return None };
    let (base, len) = (mapping.as_ptr::<u8>(), mapping.len());
    let surface = unsafe { Surface::new(base, len) };
    core::mem::forget(mapping); // mapped for the rest of the program
    surface.set_title(title);
    surface.set_size(size.0.min(capacity.0).max(1), size.1.min(capacity.1).max(1));
    if let Ok(waker) = Endpoint::create() {
        if matches!(api::wake(BROKER, id, waker.0), Ok(Ok(()))) { WAKER.store(waker.0, Ordering::Release); }
    }
    LEN.store(len, Ordering::Release);
    PIXELS.store((kind == Kind::Pixels) as usize, Ordering::Release);
    BASE.store(base as usize, Ordering::Release);
    Some(surface)
}

/// `info` with the framebuffer of a pixel window of `width` × `height` instead of the screen when the program was
/// started in a window; else `info` itself. Programs that draw through `info` (or `gfx::Screen::new(info)`) then
/// draw into the window unchanged.
pub fn pixels(info: &'static BootInfo, width: usize, height: usize, title: &str) -> &'static BootInfo {
    let Some(surface) = open(Kind::Pixels, (width, height), (width, height), title) else { return info };
    let mut copy = *info;
    copy.fb_ptr = surface.content().cast::<u32>();
    copy.width = width; copy.height = height; copy.stride = width;
    unsafe {
        let slot = &mut *core::ptr::addr_of_mut!(INFO);
        slot.write(copy);
        &*slot.as_ptr()
    }
}

// What the manager asked for since the last look: a closed window ends the program, a new size is kept for `resize`.
fn look(surface: &Surface) {
    if surface.state() == STATE_CLOSE { crate::println!("[WINDOW] CLOSED"); crate::process::exit(); }
    if let Some((width, height)) = surface.wanted() { RESIZE.store((width.min(0xFFFF) | height.min(0xFFFF) << 16) as u32, Ordering::Release); }
}

/// The next input event the manager queued (a `common/abi.rs` word), or 0.
pub fn event() -> usize {
    let Some(surface) = surface() else { return 0 };
    look(&surface);
    surface.event().unwrap_or(0)
}

/// The manager asked for another size that `resize` has not taken yet.
pub fn resize_pending() -> bool { RESIZE.load(Ordering::Acquire) != 0 }

/// The size the manager asked for, once.
pub fn resize() -> Option<(usize, usize)> {
    let v = RESIZE.swap(0, Ordering::AcqRel);
    (v != 0).then_some(((v & 0xFFFF) as usize, (v >> 16) as usize))
}

/// A pixel window says it may have changed (a text window says so when `Terminal::present` draws).
pub fn flush() {
    if PIXELS.load(Ordering::Relaxed) == 1 { if let Some(surface) = surface() { surface.changed(None); } }
}

/// Sleeps up to `ms`, ending early when the manager queues input, asks for another size or closes the window; returns
/// the time slept. Draws in a pixel window are published first.
pub fn wait(ms: usize) -> usize {
    let start = crate::time::uptime_ms();
    flush();
    let Some(surface) = surface() else { return crate::sys::call(SYSCALL_WAIT, ms, 0) };
    let waker = WAKER.load(Ordering::Acquire);
    loop {
        look(&surface);
        let elapsed = crate::time::uptime_ms() - start;
        if surface.queued() > 0 || resize_pending() || elapsed >= ms { return elapsed; }
        let slice = (ms - elapsed).min(SLICE_MS).max(1);
        if waker == 0 { crate::sys::call(SYSCALL_WAIT, slice, 0); continue; }
        match Endpoint(waker).recv_timeout(RECEIVE, slice as u32) {
            Ok(received) => { if received.cap_received { let _ = crate::ipc::drop_cap(RECEIVE); } }
            Err(crate::Error::Other(ERR_TIMEOUT)) => {}
            Err(_) => { crate::sys::call(SYSCALL_WAIT, slice, 0); }
        }
    }
}
