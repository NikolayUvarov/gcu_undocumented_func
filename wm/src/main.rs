#![no_std]
#![no_main]
// wm: the window manager (issue 088). It attaches to the window broker (157) with the manager client the shell lends
// it, shows every window on its own screen — text windows cell by cell, pixel windows over the cells — and passes the
// keys to the window in front. Programs it starts get a plain broker client, so they open a window instead of a
// screen, and only what wm holds and they ask for: the user's files, system information. The windows belong to the
// broker: when wm leaves (Alt+Q) or dies, the programs keep running and the next wm shows them where they were.
extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use mind::abi::*;
use mind::gfx::Screen;
use mind::idl::loader;
use mind::idl::window::{self as api, Placement};
use mind::input::Input;
use mind::ipc::{self, Endpoint, Message};
use mind::keys::Key;
use mind::mem::Mapping;
use mind::tui::{Rect, Terminal, DARK};
use mind::window::{Kind, Surface, STATE_CLOSE, TITLE};
use wm::desk::{Action, Content, Mode, Win, Wm};
use wm::menu::{self, Kind as ProgramKind};

// REQUEST_GPIO: the pin controller's client where the board has one, passed on to pins and pinmap (issue u017).
// REQUEST_CAMERA: the video gateway's client, passed on to camera (158-APP-0043; the shell lends it without a question).
mind::request!(REQUEST_WINDOW_MANAGER | REQUEST_FILES | REQUEST_SYSINFO | REQUEST_GPIO | REQUEST_CAMERA);

const BROKER: Endpoint = Endpoint(SLOT_WINDOW);
const RECEIVE: usize = 9; // leases, wake endpoints and the program client arrive here
const SCOPE: usize = 13; // a file client confined to one directory, for a program that asks for one file
const SYNC_MS: usize = 250;

// A window wm shows: its lease of the surface, the program's wake endpoint, what was last drawn.
struct Live { id: u32, _lease: Mapping, surface: Surface, waker: Option<usize>, changes: u32, asked: Option<(usize, usize)>, modifiers: u8 }

impl Live {
    fn wake(&self) { if let Some(waker) = self.waker { let _ = Endpoint(waker).send_timeout(&Message::new(0, 0), 2); } }
}

fn holds(slot: usize) -> bool { mind::dev::cap_info(slot).0 == CAP_KIND_ENDPOINT }

// The lease of window `id`'s surface and its wake endpoint.
fn adopt(id: u32) -> Option<Live> {
    if !matches!(api::surface(BROKER, id, RECEIVE), Ok(Ok(()))) { return None; }
    let lease = Mapping::new(RECEIVE).ok();
    let _ = ipc::drop_cap(RECEIVE); // the mapping keeps the memory
    let lease = lease?;
    let surface = unsafe { Surface::new(lease.as_ptr::<u8>(), lease.len()) };
    Some(Live { id, _lease: lease, surface, waker: waker(id), changes: 0, asked: None, modifiers: 0 })
}

fn waker(id: u32) -> Option<usize> {
    if !matches!(api::waker(BROKER, id, RECEIVE), Ok(Ok(()))) { return None; }
    let copy = ipc::mint(RECEIVE, u8::MAX, 0, 0).ok();
    let _ = ipc::drop_cap(RECEIVE);
    copy
}

// A read-only lease of window `id`'s surface, for a program the user starts to record it (issue u014). It goes with
// the window: the broker revokes it when the window ends.
fn lease(id: u32) -> Option<usize> {
    if !matches!(api::surface(BROKER, id, RECEIVE), Ok(Ok(()))) { return None; }
    let copy = ipc::mint(RECEIVE, CAP_READ | CAP_GRANT, 0, 0).ok();
    let _ = ipc::drop_cap(RECEIVE);
    copy
}

fn title(surface: &Surface) -> String {
    let mut bytes = [0u8; TITLE];
    let len = surface.title(&mut bytes);
    String::from(core::str::from_utf8(&bytes[..len]).unwrap_or(""))
}

// The desktop menu's programs (issue u003): those on the boot disk that are not services, by what they ask for. The
// loader reads each one's request from its file (about 30 ms each), so a few are looked at between events.
struct Programs { pending: Vec<String>, found: Vec<(String, ProgramKind)>, since: usize }

impl Programs {
    fn new() -> Self {
        let pending = loader::list(Endpoint::LOADER).map(|list| list.as_slice().iter().filter(|p| !p.service).map(|p| String::from(p.name.as_str())).collect()).unwrap_or_default();
        Self { pending, found: Vec::new(), since: mind::time::uptime_ms() }
    }
    // Looks at up to `count` more; true when the last one was looked at.
    fn step(&mut self, count: usize) -> bool {
        for name in self.pending.drain(..count.min(self.pending.len())) {
            let Ok(Ok(requests)) = loader::inspect_requests(Endpoint::LOADER, &name) else { continue };
            let kind = if requests & mind::process::REQUEST_WINDOW_MANAGER != 0 { ProgramKind::Manager }
                       else if requests & mind::process::REQUEST_CONSOLE != 0 { ProgramKind::Console } else { ProgramKind::Window };
            self.found.push((name, kind));
        }
        self.pending.is_empty()
    }
}

// A program wm started: what to tell the user, its process, the window lent to it to see (issue u014).
struct Started { message: String, pid: u64, window: Option<u32> }

// Starts `command` in a new window with a plain broker client and, of what it asks for, what wm holds. A program that
// asks for the display (`record -w`) sees window `front`, the one in front when the user started it.
fn launch(command: &str, front: Option<u32>) -> Result<Started, String> { start(command, front, false) }

// `pass`: `console` is started for a console program that asks for the display; it gets the lease to pass on.
fn start(command: &str, front: Option<u32>, pass: bool) -> Result<Started, String> {
    let (name, args) = command.split_once(' ').map_or((command, ""), |(n, a)| (n, a.trim()));
    let failed = |error: loader::Error| format!("cannot start {}: {:?}", name, error);
    let lost = || format!("cannot start {}: the loader does not answer", name);
    let needs = loader::inspect(Endpoint::LOADER, name).map_err(|_| lost())?.map_err(failed)?;
    let requests = loader::inspect_requests(Endpoint::LOADER, name).map_err(|_| lost())?.map_err(failed)?;
    // A console program runs in a window of `console`, which shows what it prints (issue u004).
    let display = requests & mind::process::REQUEST_DISPLAY != 0;
    if needs.console || mind::process::console_run(requests, args) {
        return start(&format!("console {}", command), front.filter(|_| display), true)
            .map_err(|_| format!("{} is a console program, and there is no console to run it in: run it in the shell", name));
    }
    if requests & mind::process::REQUEST_WINDOW_MANAGER != 0 { return Err(format!("{} is a window manager", name)); }
    if !matches!(api::client(BROKER, RECEIVE), Ok(Ok(()))) { return Err(String::from("the window broker gives no client")); }
    let session = match loader::begin(Endpoint::LOADER, name, args) {
        Ok(Ok(session)) => session,
        Ok(Err(error)) => { let _ = ipc::drop_cap(RECEIVE); return Err(failed(error)); }
        Err(_) => { let _ = ipc::drop_cap(RECEIVE); return Err(lost()); }
    };
    let grant = |slot: usize, cap: usize| matches!(loader::grant(Endpoint::LOADER, session, slot as u8, cap), Ok(Ok(())));
    let window = grant(SLOT_WINDOW, RECEIVE);
    let _ = ipc::drop_cap(RECEIVE); // the loader holds its copy
    if !window { let _ = loader::abort(Endpoint::LOADER, session); return Err(format!("cannot start {}: no window client for it", name)); }
    let (mut lent, mut missing) = (Vec::from(["window"]), Vec::new());
    if needs.files {
        if holds(SLOT_FILE) && grant(SLOT_FILE, SLOT_FILE) { lent.push("files"); } else { missing.push("files"); }
    } else if needs.file {
        // One file: a client confined to its directory, as the shell makes it, from wm's own files client.
        let path = args.split_whitespace().next().unwrap_or("");
        let (volume, rest) = if path.is_empty() { ("ram", "") } else { mind::fs::split(path) };
        let parent = rest.rfind('/').map_or("", |i| &rest[..i]);
        let scoped = holds(SLOT_FILE) && mind::fs::Dir::root(volume).and_then(|root| root.dir(parent, false)).and_then(|dir| dir.scope(true, SCOPE)).is_ok();
        if scoped && grant(SLOT_FILE, SCOPE) { lent.push("a directory"); } else { missing.push("the file's directory"); }
        if scoped { let _ = ipc::drop_cap(SCOPE); }
    }
    if requests & mind::process::REQUEST_AUTHORITY != 0 { missing.push("authority"); }
    else if needs.sysinfo { if holds(SLOT_SYSINFO) && grant(SLOT_SYSINFO, SLOT_SYSINFO) { lent.push("sysinfo"); } else { missing.push("sysinfo"); } }
    if needs.lifecycle { missing.push("lifecycle"); }
    if needs.log { missing.push("log"); }
    if requests & mind::process::REQUEST_NETWORK != 0 { missing.push("network"); }
    if requests & mind::process::REQUEST_TLS != 0 { missing.push("tls"); }
    if requests & mind::process::REQUEST_PARSE != 0 { missing.push("parse"); }
    if requests & mind::process::REQUEST_GPIO != 0 { if holds(SLOT_GPIO) && grant(SLOT_GPIO, SLOT_GPIO) { lent.push("gpio"); } else { missing.push("gpio"); } }
    if requests & mind::process::REQUEST_CAMERA != 0 { if holds(SLOT_CAMERA) && grant(SLOT_CAMERA, SLOT_CAMERA) { lent.push("camera"); } else { missing.push("camera"); } }
    // Not the screen: a read-only lease of the window in front, nothing else of it.
    let mut window = None;
    if let Some(id) = front.filter(|_| pass || display) {
        match lease(id) {
            Some(handle) => {
                if matches!(loader::grant_memory(Endpoint::LOADER, session, SLOT_DISPLAY as u8, handle), Ok(Ok(()))) { lent.push("a window to see"); window = Some(id); } else { missing.push("the display"); }
                let _ = ipc::drop_cap(handle); // the loader holds its copy
            }
            None => missing.push("the display"),
        }
    } else if display { missing.push("the display (no window in front)"); }
    let pid = match loader::commit(Endpoint::LOADER, session) { Ok(Ok(pid)) => pid, Ok(Err(error)) => return Err(failed(error)), Err(_) => return Err(lost()) };
    mind::println!("[WM] STARTED {} PID {} WITH {}{}{}{}", name, pid, lent.join(","), if missing.is_empty() { "" } else { " WITHOUT " }, missing.join(","),
                   window.map_or(String::new(), |id| format!(" (WINDOW {} TO SEE)", id)));
    let message = format!("Started {} (PID {}) with {}{}", name, pid, lent.join(", "),
                          if missing.is_empty() { String::new() } else { format!("; without {}: wm does not hold it", missing.join(", ")) });
    Ok(Started { message, pid, window })
}

// How long `wm` shows that a window is recorded: the recording's -t (10 s by default) and 3 s more; less if the
// recorder ends first.
fn recording_ms(command: &str) -> usize {
    let mut words = command.split_whitespace();
    let mut seconds = 10;
    while let Some(word) = words.next() { if word == "-t" { seconds = words.next().and_then(|v| v.parse().ok()).unwrap_or(seconds); } }
    (seconds + 3) * 1000
}

// `recordings`: windows lent to a recorder — window, the recorder's process, until when (uptime ms).
struct Manager { wm: Wm, lives: Vec<Live>, generation: u64, saved: Vec<(u32, Rect, usize)>, recordings: Vec<(u32, u64, usize)> }

impl Manager {
    fn live(&self, id: u32) -> Option<&Live> { self.lives.iter().find(|l| l.id == id) }

    // The windows the broker has now: new ones are adopted (at their saved place, or a new one), gone ones dropped.
    fn sync(&mut self) -> bool {
        let Ok(generation) = api::generation(BROKER) else { return false };
        // A program may lend its wake endpoint after wm saw its window.
        for live in self.lives.iter_mut().filter(|l| l.waker.is_none()) { live.waker = waker(live.id); }
        if generation == self.generation { return false; }
        self.generation = generation;
        let mut infos = Vec::new();
        let mut start = 0;
        while let Ok(Ok(list)) = api::list(BROKER, start) {
            infos.extend_from_slice(list.as_slice());
            if list.as_slice().len() < 16 { break; }
            start += 16;
        }
        let gone: Vec<u32> = self.lives.iter().map(|l| l.id).filter(|id| !infos.iter().any(|i| i.id == *id)).collect();
        for id in gone {
            mind::println!("[WM] GONE {}", id);
            self.wm.desk.remove(id);
            self.lives.retain(|l| l.id != id);
        }
        infos.sort_by_key(|i| i.place.z);
        let known: Vec<u32> = self.lives.iter().map(|l| l.id).collect();
        for info in infos.iter().filter(|i| !known.contains(&i.id)) {
            let Some(live) = adopt(info.id) else { continue };
            let content = if info.kind == api::Kind::Pixels { Content::Pixels } else { Content::Text };
            let size = live.surface.check().map_or((info.width as usize, info.height as usize), |(_, w, h)| (w, h));
            let mut win = Win::new(info.id, info.owner, content, size, info.title.as_str());
            let p = info.place;
            if p.columns > 0 && p.rows > 0 { win.rect = Rect::new(p.x as usize, p.y as usize, p.columns as usize, p.rows as usize); }
            self.wm.desk.add(win);
            let r = self.wm.desk.get(info.id).map_or(Rect::default(), |w| w.rect);
            mind::println!("[WM] WINDOW {} PID {} {} {}X{} \"{}\" AT {},{} {}X{}", info.id, info.owner, if content == Content::Pixels { "PIXELS" } else { "TEXT" },
                           size.0, size.1, info.title.as_str(), r.x, r.y, r.w, r.h);
            self.lives.push(live);
        }
        true
    }

    // Titles, sizes and changes of the programs' surfaces; the sizes wm wants of the windows; places saved in the
    // broker; the windows being recorded. Returns (something to draw, pixel windows to draw again).
    fn follow(&mut self) -> (bool, Vec<u32>) {
        let mut dirty = false;
        let now = mind::time::uptime_ms();
        self.recordings.retain(|&(_, pid, until)| now < until && mind::process::alive(pid));
        for win in self.wm.desk.windows.iter_mut() {
            let recording = self.recordings.iter().any(|&(id, ..)| id == win.id);
            if win.recording != recording {
                mind::println!("[WM] {} {}", if recording { "RECORDING" } else { "RECORDED" }, win.id);
                win.recording = recording; dirty = true;
            }
        }
        let mut pixels = Vec::new();
        let (full, screen) = (self.wm.desk.full_screen(), Rect::new(0, 0, self.wm.desk.cols, self.wm.desk.rows));
        for live in self.lives.iter_mut() {
            let changes = live.surface.changes();
            let Some(index) = self.wm.desk.index(live.id) else { continue };
            let win = &mut self.wm.desk.windows[index];
            if changes != live.changes {
                live.changes = changes; dirty = true;
                if win.content == Content::Pixels { pixels.push(live.id); }
            }
            if let Some((_, w, h)) = live.surface.check() {
                if win.size != (w, h) {
                    if win.content == Content::Pixels { mind::println!("[WM] PIXELS {} {}X{}", live.id, w, h); }
                    win.size = (w, h); dirty = true;
                }
            }
            let name = title(&live.surface);
            if !name.is_empty() && name != win.title { win.title = name; dirty = true; }
            // A window draws at the size of its frame's inside (issue u009), or of the screen when full (211-APP-0014).
            let inner = if full == Some(live.id) { screen } else { win.rect.inner() };
            let wanted = match win.content { Content::Text => (inner.w, inner.h), Content::Pixels => (inner.w * 8, inner.h * 16) };
            if wanted != live.surface.size() && live.asked != Some(wanted) && inner.w > 0 && inner.h > 0 {
                live.surface.ask_size(wanted.0, wanted.1);
                live.asked = Some(wanted);
                live.wake();
            }
        }
        let area = self.wm.desk.area();
        for (z, w) in self.wm.desk.windows.iter().enumerate() {
            if self.saved.iter().any(|&(id, rect, at)| id == w.id && rect == w.rect && at == z) { continue; }
            let place = Placement { x: w.rect.x as u16, y: w.rect.y as u16, columns: w.rect.w as u16, rows: w.rect.h as u16, z: z as u16, minimized: false, maximized: w.rect == area };
            let _ = api::place(BROKER, w.id, &place);
            self.saved.retain(|&(id, ..)| id != w.id);
            self.saved.push((w.id, w.rect, z));
        }
        self.saved.retain(|&(id, ..)| self.lives.iter().any(|l| l.id == id));
        (dirty, pixels)
    }

    // Shift, Ctrl and Alt as the window in front should see them: held as they are, and nothing held in the others
    // (a window that lost the focus in the middle of Alt+Tab must not keep Alt). Sent as modifier events.
    fn modifiers(&mut self, held: u8) {
        let focus = self.wm.desk.focus();
        let normal = matches!(self.wm.mode, Mode::Normal);
        for live in self.lives.iter_mut() {
            let wanted = if normal && Some(live.id) == focus { held } else { 0 };
            if live.modifiers == wanted { continue; }
            let word = input_event(0, KEY_ALT, wanted, wanted & MOD_ALT != 0, 0);
            if live.surface.push_event(word) { live.modifiers = wanted; live.wake(); }
        }
    }

    // Keys not for wm go to the window in front only.
    fn forward(&self, word: usize) {
        let Some(id) = self.wm.desk.focus() else { return };
        let Some(live) = self.live(id) else { return };
        if live.surface.push_event(word) { live.wake(); }
    }

    // A mouse event for window `id`'s program (issue u001), whether or not it is in front.
    fn pointer(&self, id: u32, word: usize) {
        let Some(live) = self.live(id) else { return };
        if live.surface.push_event(word) { live.wake(); }
    }

    fn close(&mut self, id: u32) {
        if let Some(live) = self.live(id) {
            live.surface.set_state(STATE_CLOSE);
            live.wake();
            let name = self.wm.desk.get(id).map_or(String::new(), |w| w.title.clone());
            mind::println!("[WM] CLOSE {}", id);
            self.wm.notice = Some(format!("Closing \"{}\": its program is asked to end", name));
        }
    }

    // Alt+X: every program is asked to end; those still running after 3 s are named.
    fn close_all(&mut self) {
        let asked = api::close_all(BROKER).ok().and_then(Result::ok).unwrap_or(0);
        mind::println!("[WM] CLOSE ALL: {} WINDOWS", asked);
        let start = mind::time::uptime_ms();
        while mind::time::uptime_ms() - start < 3000 && !self.lives.is_empty() { mind::time::sleep(100); self.sync(); }
        for live in &self.lives { mind::println!("[WM] STILL RUNNING: WINDOW {} PID {}", live.id, self.wm.desk.get(live.id).map_or(0, |w| w.owner)); }
    }
}

// Copies the visible cells of pixel window `index` from its surface to the screen.
fn blit(screen: &Screen, origin: (usize, usize), owner: &[u16], cols: usize, index: usize, inner: Rect, surface: &Surface) {
    let Some((Kind::Pixels, width, height)) = surface.check() else { return };
    let pixels = surface.content() as *const u32;
    let tag = index as u16 + 1;
    for cy in inner.y..inner.bottom() {
        for cx in inner.x..inner.right() {
            if owner.get(cy * cols + cx) != Some(&tag) { continue; }
            let (sx, sy) = ((cx - inner.x) * 8, (cy - inner.y) * 16);
            for row in 0..16 {
                let y = sy + row;
                for col in 0..8 {
                    let x = sx + col;
                    // Inside the size the header had when it was checked: within the surface's memory.
                    let color = if x < width && y < height { unsafe { core::ptr::read_volatile(pixels.add(y * width + x)) } } else { 0 };
                    screen.pixel(origin.0 + cx * 8 + col, origin.1 + cy * 16 + row, color);
                }
            }
        }
    }
}

// The mouse pointer: an arrow, black outline (X) and white inside (W).
const ARROW: [&[u8; 8]; 12] = [b"X       ", b"XX      ", b"XWX     ", b"XWWX    ", b"XWWWX   ", b"XWWWWX  ", b"XWWWWWX ", b"XWWWWWWX", b"XWWWXXXX", b"XWXWX   ", b"XX XWX  ", b"    XX  "];

fn draw_pointer(screen: &Screen, x: usize, y: usize) {
    for (row, line) in ARROW.iter().enumerate() {
        for (col, &b) in line.iter().enumerate() {
            match b { b'X' => screen.pixel(x + col, y + row, 0), b'W' => screen.pixel(x + col, y + row, 0xFFFFFF), _ => {} }
        }
    }
}

mind::entry!(main);
fn main(info: &'static BootInfo) {
    mind::about!("wm — window manager: programs in windows side by side (text and pixel windows), moved, resized and snapped.\nUsage: wm [program ...]   (wm fm fm clock dzen-clock; with arguments: wm fm data, edit ram:a.txt)\nAlt+Tab next window, Alt+arrows halves, Alt+1..4 quarters, Alt+Enter maximize, Alt+F full screen, Alt+L the windows,\nAlt+M move/resize, Alt+W close, Alt+R run, Alt+H keys, Alt+Q leave (the programs keep running), Alt+X close all.\nThe mouse drags titles and corners.");
    if !holds(SLOT_WINDOW) { mind::println!("[WM] NO WINDOW BROKER CLIENT: START WM FROM THE SHELL"); return; }
    match api::attach(BROKER) {
        Ok(Ok(_)) => {}
        Ok(Err(api::Error::Busy)) => { mind::println!("[WM] ANOTHER WINDOW MANAGER IS ATTACHED"); return; }
        other => { mind::println!("[WM] ATTACH FAILED: {:?}", other); return; }
    }
    if holds(SLOT_FILE) { mind::fs::use_endpoint(Endpoint(SLOT_FILE)); }
    let Some(screen) = Screen::new(info) else { return };
    let Some(mut term) = Terminal::new(screen) else { return };
    let (_, x0, y0) = term.screen().unwrap();
    let mut manager = Manager { wm: Wm::new(term.cols(), term.rows()), lives: Vec::new(), generation: u64::MAX, saved: Vec::new(), recordings: Vec::new() };
    manager.sync();
    mind::println!("[WM] READY {}X{} WINDOWS {}", term.cols(), term.rows(), manager.lives.len());
    // `wm fm, fm data, clock` or `wm fm fm clock`: the programs to start.
    let args = mind::process::args_str().trim();
    let commands: Vec<&str> = if args.contains(',') { args.split(',').map(str::trim).filter(|c| !c.is_empty()).collect() } else { args.split_whitespace().collect() };
    for command in commands {
        let result = launch(command, None);
        if let Err(error) = &result { mind::println!("[WM] {}", error); }
        manager.wm.notice = Some(result.map_or_else(|e| e, |s| s.message));
    }
    let mut programs = Some(Programs::new());
    manager.wm.programs = vec![menu::Item { label: String::from("Looking for programs…"), command: None, children: Vec::new() }];
    mind::input::pointer(true);
    // The pointer's pixel on the screen: mind::input follows a mouse's movement or a tablet's position (issue 161).
    let (mut px, mut py) = (screen.width / 2, screen.height / 2);
    let mut shown_pointer: Option<(usize, usize)> = None;
    let mut buttons = 0u8;
    let mut last_sync = 0usize;
    let mut first = true;
    loop {
        let mut relayout = first;
        let mut busy = false;
        first = false;
        while let Some(input) = mind::input::read_input() {
            busy = true; relayout = true;
            let action = match input {
                Input::Key(event) => {
                    let word = event.to_word();
                    match Key::from_event(word) {
                        Some(key) => { manager.wm.notice = None; manager.wm.key(key) }
                        // Releases go to the window in front; modifiers are given to it below, as they are held.
                        None => { if matches!(manager.wm.mode, Mode::Normal) && !mind::keys::is_modifier(event.key) { manager.forward(word); } continue; }
                    }
                }
                Input::Pointer(p) => {
                    if let Some((gx, gy)) = mind::input::pointer_pixel() { (px, py) = (x0 + gx, y0 + gy); }
                    manager.wm.pointer(p.x, p.y, p.buttons, p.wheel)
                }
            };
            // Keys, clicks and the wheel are logged; moves of the mouse are not (a drag is logged when it ends).
            let log = match input {
                Input::Key(_) => true,
                Input::Pointer(p) => p.buttons != buttons || p.wheel != 0 || !matches!(action, Action::Redraw | Action::Pointer { .. }),
            };
            buttons = if let Input::Pointer(p) = input { p.buttons } else { buttons };
            match action {
                Action::Forward => { if let Input::Key(event) = input { manager.forward(event.to_word()); } }
                Action::Redraw => {}
                Action::Close(id) => manager.close(id),
                Action::Run(command) => {
                    let result = launch(&command, manager.wm.desk.focus());
                    if let Err(error) = &result { mind::println!("[WM] {}", error); }
                    if let Ok(Started { pid, window: Some(id), .. }) = &result { manager.recordings.push((*id, *pid, mind::time::uptime_ms() + recording_ms(&command))); }
                    manager.wm.notice = Some(result.map_or_else(|e| e, |s| s.message));
                    last_sync = 0;
                }
                Action::Detach => {
                    let _ = api::detach(BROKER);
                    mind::println!("[WM] DETACHED: {} WINDOWS KEPT", manager.lives.len());
                    mind::println!("[WM] DONE");
                    return;
                }
                Action::CloseAll => { manager.close_all(); mind::println!("[WM] DONE"); return; }
                Action::Pointer { id, x, y, buttons, wheel } => {
                    manager.pointer(id, mind::window::pointer_at(buttons, x, y, wheel));
                    if log { mind::println!("[WM] POINTER {} AT {},{} BUTTONS={} WHEEL={}", id, x, y, buttons, wheel); }
                }
            }
            if log { mind::println!("[WM] {}", manager.wm.status()); }
        }
        // The menu's programs, a few at a time while nothing else happens (the menu is not changed while it is open).
        if !busy && !matches!(manager.wm.mode, Mode::Menu(_)) {
            if let Some(p) = programs.as_mut() {
                if p.step(2) {
                    manager.wm.programs = menu::catalogue(&p.found);
                    mind::println!("[WM] PROGRAMS: {} IN {} CATEGORIES IN {} MS", p.found.len(), manager.wm.programs.len(), mind::time::uptime_ms() - p.since);
                    programs = None;
                }
            }
        }
        manager.modifiers(mind::input::modifiers());
        let now = mind::time::uptime_ms();
        if now - last_sync >= SYNC_MS { last_sync = now; if manager.sync() { relayout = true; mind::println!("[WM] {}", manager.wm.status()); } }
        let (changed, mut pixels) = manager.follow();
        if !manager.wm.desk.take_changed().is_empty() { relayout = true; }
        if changed || relayout {
            let focused_cursor = manager.wm.desk.focus().and_then(|id| manager.live(id)).and_then(|l| l.surface.cursor());
            let owner = {
                let mut grid = term.grid();
                let lives = &manager.lives;
                let mut cell = |id: u32, x: usize, y: usize| lives.iter().find(|l| l.id == id).and_then(|l| l.surface.cell(x, y));
                let (owner, cursor) = manager.wm.draw(&mut grid, &DARK, &mut cell, focused_cursor);
                drop(grid);
                term.set_cursor(cursor);
                owner
            };
            // The cells under the pointer drawn last time are drawn again (pixel windows: all of them below).
            if let Some((sx, sy)) = shown_pointer.take() {
                for cy in (sy.saturating_sub(y0) / 16)..=((sy + 11).saturating_sub(y0) / 16) { for cx in (sx.saturating_sub(x0) / 8)..=((sx + 7).saturating_sub(x0) / 8) { term.touch(cx, cy); } }
                relayout = true;
            }
            term.present();
            let cols = term.cols();
            if relayout { pixels = manager.wm.desk.windows.iter().filter(|w| w.content == Content::Pixels).map(|w| w.id).collect(); }
            for id in pixels {
                let (Some(index), Some(live)) = (manager.wm.desk.index(id), manager.live(id)) else { continue };
                blit(&screen, (x0, y0), &owner, cols, index, manager.wm.desk.content(id), &live.surface);
            }
            if manager.wm.pointer.is_some() { draw_pointer(&screen, px, py); shown_pointer = Some((px, py)); }
        }
        mind::time::sleep(if busy { 5 } else { 30 });
    }
}
