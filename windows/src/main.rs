#![no_std]
#![no_main]
// Window broker (issue 157): keeps the windows of programs so they outlive the window manager (idl/window.wit). A
// window is a surface in the broker's memory (mind::window), lent to its program and to the manager from two separate
// roots, an optional wake endpoint and the place the manager last gave it. One manager at a time (badge
// mind::window::BADGE_MANAGER); when it detaches or ends, its leases are revoked and the windows stay with their
// programs, hidden. Neither side can take the memory away from the other or from the broker. Holds: its own program
// client (slot 2) to lend to managers for the programs they start; nothing else: no screen, no input, no files.
extern crate alloc;

use alloc::vec::Vec;
use mind::abi::BootInfo;
use mind::idl::codec::{List, Text};
use mind::idl::window::{self as api, Error, Info, Kind, Placement, Request};
use mind::idl::wire;
use mind::ipc::{self, Endpoint, Message};
use mind::mem::Pages;
use mind::window::{self as layout, Surface, BADGE_MANAGER, STATE_CLOSE, STATE_HIDDEN, STATE_SHOWN, TITLE};

const RECEIVED: usize = 9;
const CLIENT: usize = 2; // our own program client, lent to managers
const WINDOWS: usize = 64;

// `cap`: the shared surface; `program` and `manager`: its children whose copies are the leases. `ended`: its program
// ended while a manager was attached; the manager's lease stays until the manager has listed the windows without it
// (it may be drawing it: memory revoked under it would fault, issue 088).
struct Window { id: u32, owner: u64, _pages: Pages, cap: usize, program: usize, manager: usize, surface: Surface, waker: Option<usize>, place: Placement, ended: bool }

struct Broker { windows: Vec<Window>, next: u32, manager: Option<u64>, generation: u64 }

impl Broker {
    fn is_manager(&self, badge: u16, pid: u64) -> bool { badge == BADGE_MANAGER && self.manager == Some(pid) }
    fn index(&self, id: u32) -> Option<usize> { self.windows.iter().position(|w| w.id == id && !w.ended) }
    fn live(&self) -> impl Iterator<Item = &Window> { self.windows.iter().filter(|w| !w.ended) }

    fn create(&mut self, kind: Kind, width: u16, height: u16, pid: u64) -> Result<u32, Error> {
        if self.windows.len() >= WINDOWS || self.windows.iter().filter(|w| w.owner == pid).count() >= 8 { return Err(Error::Limit); }
        let (width, height) = (width as usize, height as usize);
        let (kind, fits) = match kind {
            Kind::Text => (layout::Kind::Text, width <= layout::MAX_COLUMNS && height <= layout::MAX_ROWS),
            Kind::Pixels => (layout::Kind::Pixels, width <= layout::MAX_PIXELS.0 && height <= layout::MAX_PIXELS.1),
        };
        if width == 0 || height == 0 || !fits { return Err(Error::Invalid); }
        let pages = Pages::new(layout::bytes(kind, width, height)).ok_or(Error::Limit)?;
        let surface = unsafe { Surface::new(pages.address() as *mut u8, pages.len()) };
        surface.init(kind, width, height, "");
        surface.set_state(if self.manager.is_some() { STATE_SHOWN } else { STATE_HIDDEN });
        let cap = pages.share().map_err(|_| Error::Limit)?;
        let roots = (ipc::mint(cap, u8::MAX, 0, 0), ipc::mint(cap, u8::MAX, 0, 0));
        let (Ok(program), Ok(manager)) = roots else {
            for slot in [roots.0, roots.1].into_iter().flatten() { let _ = ipc::drop_cap(slot); }
            let _ = ipc::drop_cap(cap); return Err(Error::Limit);
        };
        self.next = self.next.wrapping_add(1).max(1);
        let id = self.next;
        mind::println!("[WINDOWS] WINDOW {} OF PID {}: {:?} {}X{}", id, pid, kind, width, height);
        self.windows.push(Window { id, owner: pid, _pages: pages, cap, program, manager, surface, waker: None, place: Placement::default(), ended: false });
        self.generation += 1;
        Ok(id)
    }

    // Ends window `index`: every lease is revoked before the memory goes.
    fn end(&mut self, index: usize) {
        let window = self.windows.swap_remove(index);
        let _ = ipc::revoke(window.cap);
        for slot in [window.program, window.manager, window.cap] { let _ = ipc::drop_cap(slot); }
        if let Some(waker) = window.waker { let _ = ipc::revoke(waker); let _ = ipc::drop_cap(waker); }
        self.generation += 1;
    }

    fn wake(&self, window: &Window) {
        if let Some(waker) = window.waker { let _ = Endpoint(waker).send_timeout(&Message::new(0, 0), 1); }
    }

    // Windows whose end the manager has seen (or that no manager shows any more) go.
    fn reap(&mut self) { while let Some(index) = self.windows.iter().position(|w| w.ended) { self.end(index); } }

    // The manager detaches (or ended): its leases go, the windows stay hidden.
    fn detach(&mut self, why: &str) {
        let Some(pid) = self.manager.take() else { return };
        self.reap();
        for w in &self.windows {
            let _ = ipc::revoke(w.manager);
            if let Some(waker) = w.waker { let _ = ipc::revoke(waker); }
            w.surface.set_state(STATE_HIDDEN);
            self.wake(w);
        }
        mind::println!("[WINDOWS] MANAGER PID {} {}: {} WINDOWS KEPT", pid, why, self.windows.len());
    }

    // Windows of programs that ended; a manager that ended is detached.
    fn expire(&mut self) {
        if self.manager.is_some_and(|pid| !mind::process::alive(pid)) { self.detach("ENDED"); }
        while let Some(index) = self.windows.iter().position(|w| !w.ended && !mind::process::alive(w.owner)) {
            mind::println!("[WINDOWS] WINDOW {} ENDED WITH PID {}", self.windows[index].id, self.windows[index].owner);
            if self.manager.is_none() { self.end(index); continue; }
            // The manager keeps its lease until it lists the windows again; the program's goes now.
            let w = &mut self.windows[index];
            w.ended = true;
            let _ = ipc::revoke(w.program);
            if let Some(waker) = w.waker.take() { let _ = ipc::revoke(waker); let _ = ipc::drop_cap(waker); }
            self.generation += 1;
        }
    }

    fn info(w: &Window) -> Info {
        let (kind, width, height) = w.surface.check().unwrap_or((mind::window::Kind::Text, 0, 0));
        let mut title = [0u8; TITLE]; let len = w.surface.title(&mut title);
        Info { id: w.id, owner: w.owner, kind: if kind == mind::window::Kind::Pixels { Kind::Pixels } else { Kind::Text }, width: width as u16, height: height as u16,
               title: Text::new(core::str::from_utf8(&title[..len]).unwrap_or("")).unwrap_or_default(), place: w.place }
    }

    fn serve(&mut self, request: Request, call: wire::Call, badge: u16, pid: u64) -> mind::sys::Result<()> {
        let manager = self.is_manager(badge, pid);
        match request {
            Request::Create { kind, width, height } => { let result = self.create(kind, width, height, pid); api::reply_create(call, result) }
            Request::Wake { window, waker } => {
                let result = match self.index(window) {
                    Some(i) if self.windows[i].owner == pid => match ipc::mint(waker, u8::MAX, 0, 0) {
                        Ok(copy) => { if let Some(old) = self.windows[i].waker.replace(copy) { let _ = ipc::revoke(old); let _ = ipc::drop_cap(old); } Ok(()) }
                        Err(_) => Err(Error::Invalid),
                    },
                    Some(_) => Err(Error::Denied),
                    None => Err(Error::NotFound),
                };
                api::reply_wake(call, result)
            }
            Request::Remove { window } => {
                let result = match self.index(window) { Some(i) if self.windows[i].owner == pid => { self.end(i); Ok(()) } Some(_) => Err(Error::Denied), None => Err(Error::NotFound) };
                api::reply_remove(call, result)
            }
            Request::Attach => {
                let result = if badge != BADGE_MANAGER { Err(Error::Denied) }
                    else if self.manager.is_some_and(|m| m != pid && mind::process::alive(m)) { Err(Error::Busy) }
                    else {
                        self.manager = Some(pid);
                        for w in self.live() { w.surface.set_state(STATE_SHOWN); self.wake(w); }
                        let count = self.live().count();
                        mind::println!("[WINDOWS] MANAGER PID {} ATTACHED: {} WINDOWS", pid, count);
                        Ok(count as u32)
                    };
                api::reply_attach(call, result)
            }
            Request::Detach => { let result = if manager { self.detach("DETACHED"); Ok(()) } else { Err(Error::Denied) }; api::reply_detach(call, result) }
            Request::CloseAll => {
                let result = if !manager { Err(Error::Denied) } else {
                    for w in self.live() { w.surface.set_state(STATE_CLOSE); self.wake(w); }
                    let count = self.live().count();
                    mind::println!("[WINDOWS] CLOSE ALL: {} WINDOWS ASKED TO END", count);
                    Ok(count as u32)
                };
                api::reply_close_all(call, result)
            }
            Request::List { start } => {
                if !manager { return api::reply_list(call, Err(Error::Denied)); }
                let mut list = List::<Info, 16>::default();
                for w in self.live().skip(start as usize).take(16) { list.push(Self::info(w)); }
                let result = api::reply_list(call, Ok(list.as_slice()));
                self.reap(); // the manager now knows which windows ended
                result
            }
            Request::Generation => api::reply_generation(call, if manager { self.generation } else { 0 }),
            Request::Surface { window } => {
                let result = match self.index(window) {
                    Some(i) if self.windows[i].owner == pid => Ok(self.windows[i].program),
                    Some(i) if manager => Ok(self.windows[i].manager),
                    Some(_) => Err(Error::Denied),
                    None => Err(Error::NotFound),
                };
                api::reply_surface(call, result)
            }
            Request::Waker { window } => {
                let result = if !manager { Err(Error::Denied) } else { self.index(window).ok_or(Error::NotFound).and_then(|i| self.windows[i].waker.ok_or(Error::NotFound)) };
                api::reply_waker(call, result)
            }
            Request::Place { window, place } => {
                let result = if !manager { Err(Error::Denied) } else { match self.index(window) { Some(i) => { self.windows[i].place = place; Ok(()) } None => Err(Error::NotFound) } };
                api::reply_place(call, result)
            }
            Request::Client => api::reply_client(call, if manager { Ok(CLIENT) } else { Err(Error::Denied) }),
        }
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut broker = Broker { windows: Vec::new(), next: 0, manager: None, generation: 0 };
    mind::println!("[WINDOWS] READY");
    let mut checked = 0u64;
    loop {
        let now = mind::time::uptime_ms() as u64;
        if now - checked >= 500 { checked = now; broker.expire(); }
        let Ok(request) = Endpoint::SERVICE.recv_timeout(RECEIVED, 500) else { continue };
        if !request.is_call { wire::discard(&request, RECEIVED); continue; }
        let (badge, pid) = (request.badge, request.sender);
        let _ = match api::decode(&request, RECEIVED) {
            Err(reason) => wire::reject(reason),
            Ok((request, call)) => broker.serve(request, call, badge, pid),
        };
    }
}
