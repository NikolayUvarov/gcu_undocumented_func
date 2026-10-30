#![no_std]
#![no_main]
// The Effector agent's gateway service (750-EFF-0007, MC-11.3, MC-11.11): serves `idl/effector.wit` to the session
// (`effector`, `BADGE_SESSION`) and to the owner (the shell, `BADGE_OWNER`) and acts on the machine only through the
// grants `init` gives it (750-KRN-0119, `mind::effector`): the system monitor, a lifecycle client, the log, a reading
// client of the volumes, a writable scope (750-KRN-0120), the compositor's client for the screen (750-KRN-0153), the
// updater's client badged for the gateway (751-KRN-0185) and its private directory for the audit file. Every request
// passes the owner's policy and is audited before its reply (`effector_gw::gateway`, MC-10.6); a grant `init` did not
// give makes the requests that need it fail, never widen. It runs only in the development or maintenance mode
// (750-EFF-0013, 750-KRN-0143). No network, no parser, no token.
extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use effector_gw::gateway::{self, Badge, Fail, Gateway, Load, Machine, Mark, Origin, Sample, ServiceState, UpdateState};
use effector_gw::policy::{self, Ask};
use effector_gw::png;
use mind::abi::{BootInfo, CAP_KIND_ENDPOINT, SLOT_DISPLAY, SLOT_FILE, SLOT_LIFECYCLE, SLOT_LOG, SLOT_SYSINFO, SLOT_UPDATE, SLOT_VFS};
use mind::effector::{BADGE_OWNER, BADGE_SESSION, GATEWAY_DIR, POLICY_FILE, SLOT_READER};
use mind::idl::codec::{List, Text};
use mind::idl::effector as wit;
use mind::idl::{display, init, log, sysinfo, update, vfs};
use mind::ipc::Endpoint;
use mind::mem::Mapping;

const RECEIVED: usize = 9; // an unexpected capability lands here and is dropped; the screen's copy too, while encoded
const PIECE: usize = 16384; // vfs reads and writes at most this much per call
const AUDIT_MAX: u32 = 256 * 1024; // the audit file is renamed to `audit.old` beyond this
const MARK_MS: usize = 1000; // the agent's mark is lit this often; the compositor keeps it 3 s (750-EFF-0015)
const SCREEN_WIDTH: usize = 1280; // a larger screen is shrunk to fit, its proportions kept
const SCREEN_HEIGHT: usize = 800;
static mut SCRATCH: [u8; wit::REQUEST_MAX] = [0; wit::REQUEST_MAX];
static mut OUT: [u8; 32768] = [0; 32768];

fn present(slot: usize) -> bool { mind::dev::cap_info(slot).0 == CAP_KIND_ENDPOINT }

fn vfs_fail(e: vfs::Error) -> Fail {
    match e {
        vfs::Error::NotFound | vfs::Error::NotDirectory => Fail::NotFound,
        vfs::Error::Exists | vfs::Error::NotEmpty => Fail::Exists,
        vfs::Error::Denied | vfs::Error::ReadOnly => Fail::Denied,
        vfs::Error::NoSpace => Fail::Bounds,
        vfs::Error::Invalid | vfs::Error::Name | vfs::Error::IsDirectory => Fail::Malformed,
        vfs::Error::Io | vfs::Error::Handles => Fail::Io,
        // `vfs.wit` 2.6's `conflict` (another writer published the file first) and any later code: the write failed (311-EFF-0024)
        #[allow(unreachable_patterns)]
        _ => Fail::Io,
    }
}

fn call<T>(r: mind::sys::Result<core::result::Result<T, vfs::Error>>) -> Result<T, Fail> {
    match r { Ok(Ok(v)) => Ok(v), Ok(Err(e)) => Err(vfs_fail(e)), Err(_) => Err(Fail::Io) }
}

/// An updater's answer as the gateway's error; `denied` is its refusal while the owner's `remote-update` is off.
fn updater<T>(r: mind::sys::Result<core::result::Result<T, update::Error>>) -> Result<T, Fail> {
    use update::Error as E;
    match r {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(e)) => Err(match e {
            E::Denied => Fail::Denied, E::Digest => Fail::Digest, E::Malformed => Fail::Malformed, E::Signature => Fail::Signature, E::Older => Fail::Older,
            E::Expired => Fail::Expired, E::Trial => Fail::Busy, E::NotStaged => Fail::NotFound, E::NoSlot => Fail::Unsupported, _ => Fail::Io,
        }),
        Err(_) => Err(Fail::Io),
    }
}

/// One `vfs_server` client and the volume roots it opened (a root handle is kept for the client's life).
struct Client { endpoint: Endpoint, granted: bool, roots: [Option<u32>; 4] }

impl Client {
    fn new(slot: usize) -> Self { Self { endpoint: Endpoint(slot), granted: present(slot), roots: [None; 4] } }

    fn root(&mut self, volume: &str) -> Result<u32, Fail> {
        if !self.granted { return Err(Fail::Unsupported); }
        let index = match volume { "" => 0, v if v.eq_ignore_ascii_case("ram") => 1, v if v.eq_ignore_ascii_case("log") => 2, _ => return Err(Fail::NotFound) };
        if let Some(h) = self.roots[index] { return Ok(h); }
        let h = call(vfs::root(self.endpoint, volume))?;
        self.roots[index] = Some(h);
        Ok(h)
    }

    fn open(&mut self, path: &str, mode: u8) -> Result<u32, Fail> {
        let (volume, rest) = mind::fs::split(path);
        let root = self.root(volume)?;
        call(vfs::open(self.endpoint, root, rest, mode))
    }

    fn close(&self, handle: u32) { let _ = vfs::close(self.endpoint, handle); }

    fn size(&mut self, path: &str) -> Result<u64, Fail> {
        let h = self.open(path, 0)?;
        let r = call(vfs::stat(self.endpoint, h)).and_then(|e| if e.directory { Err(Fail::Malformed) } else { Ok(e.size as u64) });
        self.close(h);
        r
    }

    fn read(&mut self, path: &str, offset: u64, out: &mut [u8]) -> Result<usize, Fail> {
        let offset = u32::try_from(offset).map_err(|_| Fail::Bounds)?;
        let h = self.open(path, 0)?;
        let mut got = 0;
        let r = loop {
            if got == out.len() { break Ok(got); }
            let want = (out.len() - got).min(PIECE);
            match call(vfs::read(self.endpoint, h, offset + got as u32, want as u32, &mut out[got..got + want])) {
                Ok(0) => break Ok(got),
                Ok(n) => got += n,
                Err(e) => break Err(e),
            }
        };
        self.close(h);
        r
    }

    fn read_whole(&mut self, path: &str, max: usize) -> Result<Vec<u8>, Fail> {
        let size = self.size(path)? as usize;
        let mut out = alloc::vec![0u8; size.min(max)];
        let n = self.read(path, 0, &mut out)?;
        out.truncate(n);
        Ok(out)
    }

    fn write_at(&mut self, path: &str, offset: u64, data: &[u8], mode: u8) -> Result<(), Fail> {
        let offset = u32::try_from(offset).map_err(|_| Fail::Bounds)?;
        let h = self.open(path, mode)?;
        let mut done = 0;
        let r = loop {
            if done == data.len() { break call(vfs::flush(self.endpoint, h)); }
            let piece = &data[done..data.len().min(done + PIECE)];
            match call(vfs::write(self.endpoint, h, offset + done as u32, piece)) {
                Ok(0) => break Err(Fail::Io),
                Ok(n) => done += n as usize,
                Err(e) => break Err(e),
            }
        };
        self.close(h);
        r
    }

    fn remove(&mut self, path: &str) -> Result<(), Fail> {
        let (volume, rest) = mind::fs::split(path);
        let root = self.root(volume)?;
        call(vfs::remove(self.endpoint, root, rest))
    }

    fn rename(&mut self, from: &str, to: &str) -> Result<(), Fail> {
        let ((v1, r1), (v2, r2)) = (mind::fs::split(from), mind::fs::split(to));
        if !v1.eq_ignore_ascii_case(v2) { return Err(Fail::Malformed); }
        let root = self.root(v1)?;
        call(vfs::rename(self.endpoint, root, r1, root, r2))
    }
}

/// The machine as `init`'s grants show it to the gateway. `writer` is the scope `init` made of the policy's `deliver`
/// root at this start (750-KRN-0120): its root is that directory, so paths are named below it.
struct Host { reader: Client, writer: Client, private: Client, deliver: Option<String> }

fn temporary(path: &str) -> String { format!("{}.part", path) }

impl Host {
    /// The boot services as `init` lists them: name, state and PID.
    fn boot_services(&mut self) -> Vec<(String, ServiceState, u64)> {
        if !present(SLOT_LIFECYCLE) { return Vec::new(); }
        let Ok(Ok(list)) = init::list(Endpoint(SLOT_LIFECYCLE)) else { return Vec::new() };
        list.as_slice().iter().map(|s| {
            let state = if s.quarantined { ServiceState::Quarantined } else if s.running { ServiceState::Running } else { ServiceState::Stopped };
            (String::from(s.name.as_str()), state, if s.running { s.pid } else { 0 })
        }).collect()
    }

    /// `path` below the delivery scope's root, or `None` outside it.
    fn below<'p>(&self, path: &'p str) -> Option<&'p str> {
        let root = self.deliver.as_deref()?;
        path.strip_prefix(root).filter(|rest| !rest.is_empty())
    }
}

impl Machine for Host {
    fn uptime_ms(&mut self) -> u64 { mind::time::uptime_ms() as u64 }

    fn memory(&mut self) -> (u64, u64) {
        if !present(SLOT_SYSINFO) { return (0, 0); }
        match sysinfo::memory(Endpoint(SLOT_SYSINFO)) { Ok(Ok(m)) => (m.used, m.arena), _ => (0, 0) }
    }

    fn services(&mut self) -> Vec<(String, ServiceState)> {
        self.boot_services().into_iter().map(|(name, state, _)| (name, state)).collect()
    }

    fn control(&mut self, name: &str, action: policy::Action) -> Result<ServiceState, Fail> {
        if !present(SLOT_LIFECYCLE) { return Err(Fail::Unsupported); }
        let lifecycle = Endpoint(SLOT_LIFECYCLE);
        let failed = |e: init::Error| match e { init::Error::NotFound | init::Error::NoDevice => Fail::NotFound, init::Error::Denied => Fail::Denied, _ => Fail::Io };
        let done = match action {
            policy::Action::Start => init::run(lifecycle, name).map(drop).map_err(|_| Fail::Busy),
            policy::Action::Stop => match init::stop(lifecycle, name) { Ok(Ok(())) => Ok(()), Ok(Err(e)) => Err(failed(e)), Err(_) => Err(Fail::Io) },
            policy::Action::Restart => match init::restart(lifecycle, name) { Ok(Ok(_)) => Ok(()), Ok(Err(e)) => Err(failed(e)), Err(_) => Err(Fail::Io) },
        };
        done?;
        Ok(self.services().into_iter().find(|(n, _)| n == name).map_or(ServiceState::Absent, |(_, s)| s))
    }

    fn log(&mut self, source: &str, max: u64) -> Result<(Vec<u8>, bool), Fail> {
        match source {
            "ring" => {
                if !present(SLOT_LOG) { return Err(Fail::Unsupported); }
                let ring = Endpoint(SLOT_LOG);
                let Ok(Ok(state)) = log::state(ring) else { return Err(Fail::Io) };
                let (mut out, mut from) = (Vec::new(), state.first);
                while from < state.next {
                    let Ok(Ok(page)) = log::read(ring, from) else { return Err(Fail::Io) };
                    if page.len() == 0 { break; }
                    for e in page.as_slice() {
                        let line = format!("[{:>5}.{:03}] {}({}) {}\n", e.time_ms / 1000, e.time_ms % 1000, e.name.as_str(), e.pid, e.text.as_str());
                        if (out.len() + line.len()) as u64 > max { return Ok((out, true)); }
                        out.extend_from_slice(line.as_bytes());
                        from = e.seq + 1;
                    }
                }
                Ok((out, false))
            }
            "boot" => {
                // The newest saved boot log on `log:` (`bootNNNN.log`).
                let root = self.reader.root("log")?;
                let mut newest = String::new();
                let mut start = 0;
                loop {
                    let page = call(vfs::list(self.reader.endpoint, root, start))?;
                    if page.len() == 0 { break; }
                    for e in page.as_slice() {
                        let n = e.name.as_str();
                        if !e.directory && n.len() == 12 && n[..4].eq_ignore_ascii_case("boot") && n[8..].eq_ignore_ascii_case(".log") && n > newest.as_str() { newest = String::from(n); }
                    }
                    start += page.len() as u32;
                }
                if newest.is_empty() { return Err(Fail::NotFound); }
                let path = format!("log:{}", newest);
                let size = self.reader.size(&path)?;
                let mut out = alloc::vec![0u8; size.min(max) as usize];
                let n = self.reader.read(&path, 0, &mut out)?;
                out.truncate(n);
                Ok((out, size > max))
            }
            _ => Err(Fail::NotFound),
        }
    }

    fn size(&mut self, path: &str) -> Result<u64, Fail> { self.reader.size(path) }
    fn read(&mut self, path: &str, offset: u64, out: &mut [u8]) -> Result<usize, Fail> { self.reader.read(path, offset, out) }
    fn exists(&mut self, path: &str) -> bool {
        let below = self.below(path).map(String::from);
        below.is_some_and(|b| self.writer.size(&b).is_ok()) || self.reader.size(path).is_ok()
    }

    fn write_temporary(&mut self, path: &str, offset: u64, data: &[u8]) -> Result<(), Fail> {
        let below = String::from(self.below(path).ok_or(Fail::Denied)?);
        let mode = mind::fs::MODE_WRITE | mind::fs::MODE_CREATE | if offset == 0 { mind::fs::MODE_TRUNCATE } else { 0 };
        self.writer.write_at(&temporary(&below), offset, data, mode)
    }

    fn publish(&mut self, path: &str, replace: bool) -> Result<(), Fail> {
        let below = String::from(self.below(path).ok_or(Fail::Denied)?);
        if self.writer.size(&below).is_ok() {
            if !replace { return Err(Fail::Exists); }
            self.writer.remove(&below)?;
        }
        self.writer.rename(&temporary(&below), &below)
    }

    fn discard_temporary(&mut self, path: &str) {
        if let Some(below) = self.below(path).map(String::from) { let _ = self.writer.remove(&temporary(&below)); }
    }

    fn read_published(&mut self, path: &str, offset: u64, out: &mut [u8]) -> Result<usize, Fail> {
        let below = String::from(self.below(path).ok_or(Fail::Denied)?);
        self.writer.read(&below, offset, out)
    }

    fn audit(&mut self, line: &str) -> bool {
        mind::println!("[EFFECTOR_GW] {}", line);
        let file = format!("{}/audit", GATEWAY_DIR);
        if self.private.size(&file).map_or(false, |s| s > AUDIT_MAX as u64) {
            let old = format!("{}/audit.old", GATEWAY_DIR);
            let _ = self.private.remove(&old);
            if self.private.rename(&file, &old).is_err() { return false; }
        }
        let at = self.private.size(&file).unwrap_or(0);
        let mut text = String::from(line);
        text.push('\n');
        self.private.write_at(&file, at, text.as_bytes(), mind::fs::MODE_WRITE | mind::fs::MODE_CREATE).is_ok()
    }

    fn policy_file(&mut self) -> Result<Vec<u8>, Fail> { self.reader.read_whole(POLICY_FILE, policy::FILE_MAX + 1) }

    fn screen(&mut self, _max: u64) -> Result<Vec<u8>, Fail> {
        if !present(SLOT_DISPLAY) { return Err(Fail::Unsupported); }
        let screen = Endpoint(SLOT_DISPLAY);
        let failed = |what: core::fmt::Arguments| { mind::println!("[EFFECTOR_GW] THE SCREEN: {}", what); Fail::Io };
        let mode = display::mode(screen).map_err(|e| failed(format_args!("NO MODE: {:?}", e)))?;
        let _ = mind::ipc::drop_cap(RECEIVED);
        match display::capture(screen, RECEIVED) {
            Ok(Ok(())) => {}
            Ok(Err(display::Error::NoScreen)) => return Err(Fail::NotFound),
            other => return Err(failed(format_args!("NO COPY: {:?}", other))),
        }
        let encoded = (|| {
            let copy = Mapping::new(RECEIVED).map_err(|e| failed(format_args!("THE COPY NOT MAPPED: {:?}", e)))?;
            let (width, height, stride) = (mode.width as usize, mode.height as usize, mode.stride as usize);
            if width == 0 || height == 0 || stride < width || copy.len() < stride * height * 4 {
                return Err(failed(format_args!("{}x{} STRIDE {} IN A COPY OF {} BYTES", width, height, stride, copy.len())));
            }
            let pixels = unsafe { core::slice::from_raw_parts(copy.as_ptr::<u32>() as *const u32, stride * height) };
            let (to_width, to_height) = png::fit(width, height, SCREEN_WIDTH, SCREEN_HEIGHT);
            Ok(png::encode(to_width, to_height, &mut |y, row| png::shrink_row(pixels, stride, width, height, to_height, y, row)))
        })();
        let _ = mind::ipc::drop_cap(RECEIVED);
        encoded
    }

    fn reboot(&mut self) -> Result<(), Fail> {
        if !present(SLOT_LIFECYCLE) { return Err(Fail::Unsupported); }
        match init::reboot(Endpoint(SLOT_LIFECYCLE)) { Ok(Ok(())) => Ok(()), Ok(Err(init::Error::Denied)) => Err(Fail::Denied), _ => Err(Fail::Io) }
    }

    fn remote_update(&mut self) -> bool {
        present(SLOT_LIFECYCLE) && matches!(init::mode(Endpoint(SLOT_LIFECYCLE)), Ok(Ok(m)) if m.remote_update)
    }

    fn install_begin(&mut self, size: u64, sha256: &str, command: &str) -> Result<u32, Fail> {
        if !present(SLOT_UPDATE) { return Err(Fail::Unsupported); }
        updater(update::install_begin(Endpoint(SLOT_UPDATE), size, sha256, command))
    }

    fn install_write(&mut self, handle: u32, offset: u64, data: &[u8]) -> Result<(), Fail> {
        if !present(SLOT_UPDATE) { return Err(Fail::Unsupported); }
        updater(update::install_write(Endpoint(SLOT_UPDATE), handle, offset, data))
    }

    fn install_commit(&mut self, handle: u32) -> Result<u64, Fail> {
        if !present(SLOT_UPDATE) { return Err(Fail::Unsupported); }
        updater(update::install_commit(Endpoint(SLOT_UPDATE), handle))
    }

    fn apply(&mut self, tries: u8) -> Result<(), Fail> {
        if !present(SLOT_UPDATE) { return Err(Fail::Unsupported); }
        updater(update::apply(Endpoint(SLOT_UPDATE), tries))
    }

    fn update_state(&mut self) -> Option<UpdateState> {
        if !present(SLOT_UPDATE) { return None; }
        let s = update::status(Endpoint(SLOT_UPDATE)).ok()?;
        Some(UpdateState { slot: s.slot, trial: s.trial, running: s.running, recorded: s.recorded, confirmed: s.confirmed, staged: s.staged, failure: String::from(s.failure.as_str()) })
    }

    fn load(&mut self) -> u32 {
        if !present(SLOT_SYSINFO) { return 0; }
        match sysinfo::load(Endpoint(SLOT_SYSINFO)) { Ok(Ok(l)) => l.one, _ => 0 }
    }

    fn sample(&mut self) -> Sample {
        let mut s = Sample { uptime_ms: mind::time::uptime_ms() as u64, ..Sample::default() };
        s.services = self.boot_services().into_iter().map(|(name, state, pid)| Load { name, state: Some(state), pid, ..Load::default() }).collect();
        if present(SLOT_SYSINFO) {
            let info = Endpoint(SLOT_SYSINFO);
            let mut start = 0;
            while let Ok(Ok(cpus)) = sysinfo::cpus(info, start) {
                for c in cpus.as_slice().iter().filter(|c| c.online) { s.cpus += 1; s.busy_ns += c.busy_ns; s.idle_ns += c.idle_ns; }
                if cpus.len() < 64 { break; }
                start += cpus.len() as u32;
            }
            if let Ok(Ok(pool)) = sysinfo::pool(info) { s.memory_total = pool.frames; s.memory_free = pool.frames_free; }
            // The services' processes, found by the PIDs init gave.
            let mut start = 0;
            while let Ok(Ok(tasks)) = sysinfo::tasks(info, start) {
                for t in tasks.as_slice() {
                    if let Some(l) = s.services.iter_mut().find(|l| l.pid != 0 && l.pid == t.pid) {
                        l.run_ns = t.run_ns; l.started_ns = t.started_ns; l.memory = t.image + t.stack + t.heap + t.screen;
                    }
                }
                if tasks.len() < 40 { break; }
                start += tasks.len() as u32;
            }
        }
        for volume in ["", "ram", "log"] {
            let Ok(root) = self.reader.root(volume) else { continue };
            if let Ok(Ok(v)) = vfs::volume(self.reader.endpoint, root) { s.volumes.push((String::from(volume), v.bytes, v.free)); }
        }
        s
    }
}

fn error(f: Fail) -> wit::Error {
    match f {
        Fail::Denied => wit::Error::Denied, Fail::NotFound => wit::Error::NotFound, Fail::Busy => wit::Error::Busy, Fail::Bounds => wit::Error::Bounds,
        Fail::Digest => wit::Error::Digest, Fail::Exists => wit::Error::Exists, Fail::Expired => wit::Error::Expired, Fail::Audit => wit::Error::Audit,
        Fail::Io => wit::Error::Io, Fail::Signature => wit::Error::Signature, Fail::Older => wit::Error::Older, Fail::Malformed => wit::Error::Malformed,
        Fail::Cancelled => wit::Error::Cancelled, Fail::Unsupported => wit::Error::Unsupported, Fail::NoPolicy => wit::Error::NoPolicy,
    }
}

fn state(s: ServiceState) -> wit::ServiceState {
    match s { ServiceState::Running => wit::ServiceState::Running, ServiceState::Stopped => wit::ServiceState::Stopped, ServiceState::Quarantined => wit::ServiceState::Quarantined, ServiceState::Absent => wit::ServiceState::Absent }
}

fn hex(bytes: &[u8; 32]) -> Text<64> {
    let mut s = String::with_capacity(64);
    for b in bytes { s.push(b"0123456789abcdef"[(b >> 4) as usize] as char); s.push(b"0123456789abcdef"[(b & 15) as usize] as char); }
    Text::new(&s).unwrap_or_default()
}

fn transfer(t: &gateway::Transfer) -> wit::Transfer { wit::Transfer { handle: t.handle, size: t.size, sha256: hex(&t.sha256), truncated: t.truncated } }

fn origin(o: &wit::Origin) -> Origin<'_> {
    Origin { command: o.command.as_str(), initiator: o.initiator.as_str(), reason: o.reason.as_str(), truncated: o.truncated, deadline_ms: o.deadline_ms }
}

// From the shell it only says why it does not serve.
mind::request!(mind::process::REQUEST_CONSOLE);

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    // init starts it with `--service` in a mode (750-KRN-0119, 0143); started any other way it serves nothing.
    let service = mind::process::args_str().split_whitespace().any(|a| a == "--service");
    if !service || !present(mind::abi::SLOT_SERVICE) { mind::println!("[EFFECTOR_GW] NOT A SERVICE: init starts it with --service in a mode (750-KRN-0119)"); return; }
    let mut host = Host { reader: Client::new(SLOT_READER), writer: Client::new(SLOT_FILE), private: Client::new(SLOT_VFS), deliver: None };
    // The delivery root `init` made the scope of at this start; a later policy's root takes effect at the next start.
    if let Ok(bytes) = host.policy_file() { host.deliver = policy::Policy::parse(&bytes).ok().and_then(|p| p.deliver().map(String::from)); }
    let mut gw = Gateway::new();
    let held = |slot: usize, name: &'static str| if present(slot) { name } else { "-" };
    mind::println!("[EFFECTOR_GW] READY; GRANTS: {} {} {} {} {} {} {} {}", held(SLOT_SYSINFO, "SYSINFO"), held(SLOT_LIFECYCLE, "LIFECYCLE"), held(SLOT_LOG, "LOG"),
        held(SLOT_READER, "READER"), held(SLOT_FILE, "WRITER"), held(SLOT_VFS, "PRIVATE"), held(SLOT_DISPLAY, "DISPLAY"), held(SLOT_UPDATE, "UPDATER"));
    // The owner's policy as the machine starts; without it every action answers no-policy.
    if let Err(f) = gw.reload_policy(&mut host, Badge::Owner) { mind::println!("[EFFECTOR_GW] NO POLICY: {}", gateway::fail_name(f)); }
    let scratch = unsafe { &mut *core::ptr::addr_of_mut!(SCRATCH) };
    let out = unsafe { &mut *core::ptr::addr_of_mut!(OUT) };
    let mut mark_at = 0;
    loop {
        // The agent's mark each second, between requests: grey, yellow or red (750-EFF-0015); it goes when the gateway does.
        if mind::time::uptime_ms() >= mark_at && present(SLOT_DISPLAY) {
            let state = match gw.mark(mind::time::uptime_ms() as u64) {
                Mark::Unconnected => display::RemoteState::Unconnected, Mark::Reading => display::RemoteState::Reading, Mark::Changing => display::RemoteState::Changing,
            };
            let _ = display::remote(Endpoint(SLOT_DISPLAY), state);
            mark_at = mind::time::uptime_ms() + MARK_MS;
        }
        let Ok(request) = Endpoint::SERVICE.recv_timeout(RECEIVED, MARK_MS as u32) else { continue };
        if !request.is_call { continue; }
        let badge = match request.badge { BADGE_SESSION => Badge::Session, BADGE_OWNER => Badge::Owner, _ => Badge::Other };
        let (req, reply) = match wit::decode(&request, RECEIVED, scratch) {
            Ok(r) => r,
            Err(e) => { mind::println!("[EFFECTOR_GW] A REQUEST NOT READ (BADGE {}): {:?}", request.badge, e); continue; }
        };
        let m: &mut dyn Machine = &mut host;
        let _ = match req {
            wit::Request::Status => {
                let s = gw.status(m);
                let mut services = List::default();
                for (name, st) in s.services.iter() { if !services.push(wit::Service { name: Text::new(name).unwrap_or_default(), state: state(*st) }) { break; } }
                let status = wit::Status { release: Text::default(), uptime_ms: s.uptime_ms, memory_used: s.memory_used, memory_total: s.memory_total, services, policy: s.policy, busy: s.busy, journal_free: 0 };
                if badge == Badge::Other { wit::reply_status(reply, &wit::Status::default()) } else { wit::reply_status(reply, &status) }
            }
            wit::Request::ServiceControl { origin: o, name, action } => {
                let action = match action { wit::ServiceAction::Start => policy::Action::Start, wit::ServiceAction::Stop => policy::Action::Stop, wit::ServiceAction::Restart => policy::Action::Restart };
                wit::reply_service_control(reply, gw.service_control(m, badge, &origin(&o), name.as_str(), action).map(state).map_err(error))
            }
            wit::Request::OpenLog { origin: o, source } => {
                let r = gw.open_log(m, badge, &origin(&o), source.as_str()).map(|t| transfer(&t)).map_err(error);
                wit::reply_open_log(reply, r.as_ref().map_err(|e| *e))
            }
            wit::Request::OpenFile { origin: o, path } => {
                let r = gw.open_file(m, badge, &origin(&o), path.as_str()).map(|t| transfer(&t)).map_err(error);
                wit::reply_open_file(reply, r.as_ref().map_err(|e| *e))
            }
            wit::Request::OpenScreen { origin: o } => {
                let r = gw.open_screen(m, badge, &origin(&o)).map(|t| transfer(&t)).map_err(error);
                wit::reply_open_screen(reply, r.as_ref().map_err(|e| *e))
            }
            wit::Request::Load => wit::reply_load(reply, gw.load(m, badge)),
            wit::Request::Connection { connected, active } => { gw.connection(m, badge, connected, active); wit::reply_connection(reply) }
            wit::Request::Sample => {
                let s = gw.sample(m, badge);
                let mut services = List::default();
                for l in s.services.iter() {
                    let load = wit::Load { name: Text::new(&l.name).unwrap_or_default(), state: state(l.state.unwrap_or(ServiceState::Absent)), pid: l.pid, run_ns: l.run_ns, started_ns: l.started_ns, memory: l.memory };
                    if !services.push(load) { break; }
                }
                let mut volumes = List::default();
                for (name, bytes, free) in s.volumes.iter() { if !volumes.push(wit::Space { name: Text::new(name).unwrap_or_default(), bytes: *bytes, free: *free }) { break; } }
                let sample = wit::Sample { cpus: s.cpus, busy_ns: s.busy_ns, idle_ns: s.idle_ns, uptime_ms: s.uptime_ms, memory_total: s.memory_total, memory_free: s.memory_free, services, volumes };
                wit::reply_sample(reply, &sample)
            }
            wit::Request::Read { handle, offset } => match gw.read(m, badge, handle, offset, out) {
                Ok(n) => wit::reply_read(reply, Ok(&out[..n])),
                Err(f) => wit::reply_read(reply, Err(error(f))),
            },
            wit::Request::Close { handle } => wit::reply_close(reply, gw.close(m, badge, handle).map_err(error)),
            wit::Request::DeliverBegin { origin: o, path, size, sha256, replace } =>
                wit::reply_deliver_begin(reply, gw.deliver_begin(m, badge, &origin(&o), path.as_str(), size, sha256.as_str(), replace).map_err(error)),
            wit::Request::PackageBegin { origin: o, size, .. } => wit::reply_package_begin(reply, Err(error(gw.not_yet(m, badge, &origin(&o), "package-begin", Ask::Package("", size))))),
            wit::Request::Write { handle, offset, data } => wit::reply_write(reply, gw.write(m, badge, handle, offset, data).map_err(error)),
            wit::Request::DeliverCommit { handle } => {
                let delivered = gw.deliver_commit(m, badge, handle).map(|d| wit::Delivered { path: Text::new(&d.path).unwrap_or_default(), size: d.size, sha256: hex(&d.sha256) });
                wit::reply_deliver_commit(reply, delivered.as_ref().map_err(|f| error(*f)))
            }
            wit::Request::PackageCommit { .. } => wit::reply_package_commit(reply, Err(wit::Error::NotFound)),
            wit::Request::UpdateCheck { origin: o, apply } => {
                let step = if apply { policy::Update::Apply } else { policy::Update::Check };
                wit::reply_update_check(reply, Err(error(gw.not_yet(m, badge, &origin(&o), "update-check", Ask::Update(step)))))
            }
            wit::Request::Reboot { origin: o, delay_ms } => {
                // Answered first, so the session keeps and sends its result; then init is asked, after the delay.
                let admitted = gw.reboot(m, badge, &origin(&o), delay_ms);
                let sent = wit::reply_reboot(reply, admitted.map(drop).map_err(error));
                if let Ok(delay) = admitted {
                    mind::time::sleep(delay as usize);
                    if let Err(f) = gw.restart(m, o.command.as_str()) { mind::println!("[EFFECTOR_GW] THE RESTART WAS REFUSED: {}", gateway::fail_name(f)); }
                }
                sent
            }
            wit::Request::TestRun { origin: o, .. } => wit::reply_test_run(reply, Err(error(gw.not_yet(m, badge, &origin(&o), "test-run", Ask::Test(&[]))))),
            wit::Request::Cancel { command } => wit::reply_cancel(reply, gw.cancel(m, badge, command.as_str()).map_err(error)),
            wit::Request::UpdateBegin { origin: o, size, sha256 } =>
                wit::reply_update_begin(reply, gw.update_begin(m, badge, &origin(&o), size, sha256.as_str()).map_err(error)),
            wit::Request::UpdateCommit { handle } => wit::reply_update_commit(reply, gw.update_commit(m, badge, handle).map_err(error)),
            wit::Request::UpdateApply { origin: o, tries } => wit::reply_update_apply(reply, gw.update_apply(m, badge, &origin(&o), tries).map_err(error)),
            wit::Request::UpdateState => {
                let s = gw.update_state(m, badge);
                let state = wit::UpdateState { slot: s.slot, trial: s.trial, running: s.running, recorded: s.recorded, confirmed: s.confirmed, staged: s.staged, failure: Text::new(&s.failure).unwrap_or_default() };
                wit::reply_update_state(reply, &state)
            }
            wit::Request::ReloadPolicy => {
                let summary = gw.reload_policy(m, badge).map(|(version, sha, rules, read_ms)| wit::PolicySummary { version, sha256: hex(&sha), rules, read_ms });
                wit::reply_reload_policy(reply, summary.as_ref().map_err(|f| error(*f)))
            }
            wit::Request::Audit { count } => {
                let lines = gw.audit_lines(badge, count as usize).map(|lines| {
                    let mut list: List<Text<256>, 16> = List::default();
                    for l in lines.iter() { if !list.push(Text::new(l).unwrap_or_default()) { break; } }
                    list
                });
                wit::reply_audit(reply, lines.as_ref().map(|l| l.as_slice()).map_err(|f| error(*f)))
            }
        };
    }
}
