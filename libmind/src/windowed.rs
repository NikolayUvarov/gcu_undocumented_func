//! A program in a window (issue 088). A program started with a client of the window broker in `SLOT_WINDOW` (the
//! window manager lends one to the programs it starts) shows itself in a window instead of on a screen of its own:
//! `mind::tui::Terminal::open` draws into a text window, `pixels` gives a framebuffer in a pixel window. From then on
//! `mind::input` reads the keys the manager queues in the surface, and `mind::time::sleep` waits on the window's wake
//! endpoint, so a key ends it early as it does on a screen. A window the manager closes ends the program. A pixel
//! window has room for the screen's pixels and takes the size of its frame as a text window does (`pixels_resized`,
//! issue u009). A program in a window that ends with a nonzero status leaves its last lines on view until a key
//! (`ended`, 211-APP-0039).
use crate::abi::{event_key, event_pressed, pointer_absolute_fields, BootInfo, CAP_KIND_ENDPOINT, ERR_TIMEOUT, KEY_POINTER, SLOT_WINDOW, SYSCALL_WAIT};
use crate::idl::window as api;
use crate::ipc::Endpoint;
use crate::mem::{Mapping, Pages};
use crate::tui::ended::{self, Tail};
use crate::tui::{Cell, Grid, DARK};
use crate::window::{Kind, Surface, MAX_PIXELS, STATE_CLOSE};
use core::fmt::Write;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, AtomicUsize, Ordering};

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
static ROOM: AtomicU32 = AtomicU32::new(0); // width | height << 16 a pixel window has memory for
static mut INFO: core::mem::MaybeUninit<BootInfo> = core::mem::MaybeUninit::uninit();
// What a program started with a broker client printed last, for `ended`; whether it was started so (0: not looked).
const KEPT: usize = 2048;
static PRINTED: Tail<KEPT> = Tail::new();
static STARTED_IN_WINDOW: AtomicU8 = AtomicU8::new(0);
// The program's name from its `about!` text; the window it opens when it ends with no window yet has this much room.
static NAME: AtomicUsize = AtomicUsize::new(0);
static NAME_LEN: AtomicUsize = AtomicUsize::new(0);
const ENDED_ROOM: (usize, usize) = (160, 64);
// While `ended` waits, a closed window ends the wait, not the program.
static ENDING: AtomicBool = AtomicBool::new(false);

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
/// draw into the window unchanged. The window has room for the screen (when the broker has the memory), so the
/// manager may give it another size: the program takes it with `pixels_resized` in its loop (while a size it was
/// asked for is not taken, its waits end at once, as a text program's do until it draws).
pub fn pixels(info: &'static BootInfo, width: usize, height: usize, title: &str) -> &'static BootInfo {
    let (width, height) = (width.clamp(1, MAX_PIXELS.0), height.clamp(1, MAX_PIXELS.1));
    let room = (info.width.clamp(width, MAX_PIXELS.0), info.height.clamp(height, MAX_PIXELS.1));
    let Some((surface, room)) = open_pixels(room, (width, height), title) else { return info };
    ROOM.store((room.0 | room.1 << 16) as u32, Ordering::Release);
    let mut copy = *info;
    copy.fb_ptr = surface.content().cast::<u32>();
    copy.width = width; copy.height = height; copy.stride = width;
    unsafe {
        let slot = &mut *core::ptr::addr_of_mut!(INFO);
        slot.write(copy);
        &*slot.as_ptr()
    }
}

// A pixel window with room for `room`; one of `size` only when the broker has no memory for that.
fn open_pixels(room: (usize, usize), size: (usize, usize), title: &str) -> Option<(Surface, (usize, usize))> {
    if let Some(surface) = open(Kind::Pixels, room, size, title) { return Some((surface, room)); }
    if room == size || !requested() { return None; }
    open(Kind::Pixels, size, size, title).map(|surface| (surface, size))
}

/// The framebuffer of a pixel window at a new size, once the manager asked for one (issue u009): the window is drawn
/// at that size from now on (as much of it as the window has room for), and the program draws everything again into
/// what this returns — `pixels`' info with the new width, height and stride. None: no new size asked, or no pixel
/// window.
pub fn pixels_resized() -> Option<BootInfo> {
    if PIXELS.load(Ordering::Acquire) != 1 { return None; }
    let (width, height) = resize()?;
    let room = ROOM.load(Ordering::Acquire) as usize;
    let (width, height) = (width.clamp(1, room & 0xFFFF), height.clamp(1, room >> 16));
    let surface = surface()?;
    if !surface.set_size(width, height) { return None; }
    let mut copy = unsafe { *(*core::ptr::addr_of!(INFO)).as_ptr() };
    copy.width = width; copy.height = height; copy.stride = width;
    Some(copy)
}

// What the manager asked for since the last look: a closed window ends the program, a new size is kept for `resize`.
fn look(surface: &Surface) {
    if surface.state() == STATE_CLOSE && !ENDING.load(Ordering::Acquire) { crate::println!("[WINDOW] CLOSED"); crate::process::exit(); }
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
        if surface.queued() > 0 || resize_pending() || elapsed >= ms || surface.state() == STATE_CLOSE { return elapsed; }
        let slice = (ms - elapsed).min(SLICE_MS).max(1);
        if waker == 0 { crate::sys::call(SYSCALL_WAIT, slice, 0); continue; }
        match Endpoint(waker).recv_timeout(RECEIVE, slice as u32) {
            Ok(received) => { if received.cap_received { let _ = crate::ipc::drop_cap(RECEIVE); } }
            Err(crate::Error::Other(ERR_TIMEOUT)) => {}
            Err(_) => { crate::sys::call(SYSCALL_WAIT, slice, 0); }
        }
    }
}

/// Keeps what the program prints (`process::log`) when it was started with a broker client, for `ended`.
pub(crate) fn keep(bytes: &[u8]) {
    let started = match STARTED_IN_WINDOW.load(Ordering::Relaxed) {
        0 => { let yes = active() || requested(); STARTED_IN_WINDOW.store(if yes { 1 } else { 2 }, Ordering::Relaxed); yes }
        known => known == 1,
    };
    if started { PRINTED.push(bytes); }
}

/// The program's name, the first word of its `about!` text (the title of the window `ended` opens).
pub fn name(about: &'static str) {
    let name = about.split([' ', '\n']).next().unwrap_or("");
    NAME_LEN.store(name.len(), Ordering::Relaxed);
    NAME.store(name.as_ptr() as usize, Ordering::Release);
}

fn program_name() -> &'static str {
    let at = NAME.load(Ordering::Acquire);
    if at == 0 { return ""; }
    let bytes = unsafe { core::slice::from_raw_parts(at as *const u8, NAME_LEN.load(Ordering::Relaxed)) };
    core::str::from_utf8(bytes).unwrap_or("")
}

/// A program in a window that ends with a nonzero `status` (211-APP-0039) leaves its window on view until a key, a
/// click or the window's close: the last lines it printed and `ENDED (STATUS n): PRESS A KEY`, in its own text or
/// pixel window, or in a text window opened for them when it had none. `process::exit_with` calls it.
pub(crate) fn ended(status: u32) {
    if !active() {
        if !requested() { return; }
        let mut title = crate::util::FixedBuf::<64>::new();
        let _ = write!(title, "{}{}ended", program_name(), if program_name().is_empty() { "" } else { " " });
        if open(Kind::Text, ENDED_ROOM, (80, 25), title.as_str()).is_none() { return; }
    }
    let Some(surface) = surface() else { return };
    ENDING.store(true, Ordering::Release);
    while surface.event().is_some() {} // what was queued before the end is not an answer
    let mut kept = [0u8; KEPT];
    let len = PRINTED.copy(&mut kept);
    let printed = &kept[..len];
    show(&surface, printed, status);
    crate::println!("[WINDOW] ENDED (STATUS {}): WAITING FOR A KEY", status);
    loop {
        wait(60_000);
        if surface.state() == STATE_CLOSE { return; }
        if let Some((width, height)) = resize() {
            if surface.set_size(width, height) { show(&surface, printed, status); }
        }
        let mut answered = false;
        while let Some(word) = surface.event() {
            answered |= if event_key(word) == KEY_POINTER { pointer_absolute_fields(word).is_some_and(|(buttons, ..)| buttons != 0) }
                        else { event_pressed(word) && !crate::keys::is_modifier(event_key(word)) };
        }
        if answered { return; }
    }
}

// Draws `ended`'s lines into the window: cells in a text window, glyphs in a pixel window.
fn show(surface: &Surface, printed: &[u8], status: u32) {
    let pixels = PIXELS.load(Ordering::Acquire) == 1;
    let (width, height) = surface.size();
    let (cols, rows) = if pixels { (width / 8, height / 16) } else { (width, height) };
    if cols == 0 || rows == 0 { return; }
    let Some(mut pages) = Pages::new(cols * rows * core::mem::size_of::<Cell>()) else { return };
    let cells = unsafe { core::slice::from_raw_parts_mut(pages.as_mut_slice().as_mut_ptr() as *mut Cell, cols * rows) };
    let mut grid = Grid::new(cells, cols, rows);
    ended::draw(&mut grid, printed, status, &DARK);
    let screen = unsafe { crate::gfx::Screen::at(surface.content().cast::<u32>(), width, height, width) };
    if pixels { screen.clear(DARK.panel.bg); }
    for y in 0..rows {
        for x in 0..cols {
            let cell = grid.get(x, y);
            if pixels { screen.glyph16(x * 8, y * 16, cell.ch, cell.style.fg, Some(cell.style.bg)); } else { surface.set_cell(x, y, cell.ch, cell.style.fg, cell.style.bg); }
        }
    }
    surface.set_cursor(None);
    surface.changed(None);
}

/// A window opened through a broker client the program names, beside the program's own (`open`): the shell's window
/// in `wm`, a session of the shell (211-APP-0040). The manager's sizes are taken with `wanted`, its keys with `event`.
pub struct Window { broker: Endpoint, id: u32, surface: Surface, mapping: Option<Mapping> }

impl Window {
    /// A window of `kind` with memory for `capacity`, drawn at `size`; the surface arrives in the free slot `receive`,
    /// which is free again on return.
    pub fn open(broker: Endpoint, receive: usize, kind: Kind, capacity: (usize, usize), size: (usize, usize), title: &str) -> Option<Self> {
        let wanted = if kind == Kind::Pixels { api::Kind::Pixels } else { api::Kind::Text };
        let Ok(Ok(id)) = api::create(broker, wanted, capacity.0 as u16, capacity.1 as u16) else { return None };
        let mapping = match api::surface(broker, id, receive) { Ok(Ok(())) => Mapping::new(receive).ok(), _ => None };
        let _ = crate::ipc::drop_cap(receive); // the mapping keeps the memory
        let Some(mapping) = mapping else { let _ = api::remove(broker, id); return None };
        let surface = unsafe { Surface::new(mapping.as_ptr::<u8>(), mapping.len()) };
        surface.set_title(title);
        surface.set_size(size.0.min(capacity.0).max(1), size.1.min(capacity.1).max(1));
        Some(Self { broker, id, surface, mapping: Some(mapping) })
    }
    pub fn surface(&self) -> Surface { self.surface }
    /// Its id in the broker, as the manager knows it.
    pub fn id(&self) -> u32 { self.id }
    /// The next input event the manager queued (a `common/abi.rs` word).
    pub fn event(&self) -> Option<usize> { self.surface.event() }
    /// The manager closed it.
    pub fn closed(&self) -> bool { self.surface.state() == STATE_CLOSE }
}

impl Drop for Window {
    // Unmapped first: the broker revokes the program's lease as it removes the window.
    fn drop(&mut self) { drop(self.mapping.take()); let _ = api::remove(self.broker, self.id); }
}
