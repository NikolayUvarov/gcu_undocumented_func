#![no_std]
#![no_main]
// link (800-LNK-0003): MIND Core's end of MIND Link, the slave of linkproto (docs/link/protocol.md) over usb_serial's
// port (idl/serial.wit, SLOT_DEV0). Its authority is what init grants (MC-2.3): files only in data/link/ (a vfs client
// confined there, SLOT_VFS), the system log through a reading log client (SLOT_LOG), an application's loader client
// (SLOT_LOADER) and no console. init starts it when the boot volume holds link.conf at its root and passes that file's
// words as arguments: `baud=<rate>`, and `run`, without which it starts no program (800-LNK-0009). Files may lie in
// directories below data/link/ (800-LNK-0010). With `dev`, the developer mode (800-LNK-0012, the maintainer's decision
// of 2026-10-21): it also starts the built-in programs by name, lends them the network, the Wi-Fi station and the
// system monitor where init granted it those, and restarts the machine on REBOOT; and posts the master's key events where
// init gave it the input capability (800-LNK-0032). Everything from the line is checked by linkproto before use (MC-11.4);
// hellos, files, programs, input and refusals are logged (MC-10.2).
mod names;

use linkproto::app::{FileService, Files, RunOutput, COMMAND_MAX, EXIT_UNKNOWN, RUN_ENDED, RUN_FAULTED, RUN_RUNNING, RUN_STOPPED};
use linkproto::endpoint::{channel, code, kind};
use linkproto::{Line, Service, Slave};
use mind::abi::{BootInfo, CAP_KIND_ENDPOINT, CAP_READ, CAP_WRITE, SLOT_CONSOLE, SLOT_DEV0, SLOT_DISPLAY, SLOT_FILE, SLOT_LIFECYCLE, SLOT_NETPOLICY, SLOT_NETWORK, SLOT_SHELL, SLOT_SYSINFO, SLOT_VFS, SLOT_WIFI};
use mind::fs::{self, File};
use mind::idl::{display, init as lifecycle, loader, netpolicy, serial, shell};
use mind::ipc::{self, Endpoint};
use names::{hidden, DIR};

const FIRST_BAUD: u32 = 115_200;
// The screen's mark for the link (800-LNK-0020, the compositor's 800-KRN-0156): said every MARK_MS; a master heard in
// the last PEER_MS is connected.
const MARK_MS: u64 = 1000;
const PEER_MS: u64 = 5000;
/// The longest path `loader` takes (idl/loader.wit `begin`).
const LOADER_PATH_MAX: usize = 64;

/// The port as linkproto's line: bytes through usb_serial, waiting with its `wait`.
struct Port { endpoint: Endpoint, pending: [u8; 4096], at: usize, len: usize }

impl Line for Port {
    fn write(&mut self, bytes: &[u8]) -> bool {
        bytes.chunks(4096).all(|chunk| matches!(serial::write(self.endpoint, chunk), Ok(Ok(()))))
    }

    fn read(&mut self, out: &mut [u8], timeout_ms: u32) -> usize {
        if self.at == self.len {
            // While a program runs, its output is taken between short waits, so it is not kept waiting.
            let end = mind::time::uptime_ms() as u64 + timeout_ms.min(1000) as u64;
            loop {
                let running = programs().pump();
                let left = end.saturating_sub(mind::time::uptime_ms() as u64) as u32;
                let waiting = serial::wait(self.endpoint, if running { left.min(20) } else { left }).unwrap_or(0);
                if waiting > 0 { break; }
                if mind::time::uptime_ms() as u64 >= end { return 0; }
            }
            self.len = match serial::read(self.endpoint, &mut self.pending) { Ok(Ok(n)) => n, _ => 0 };
            self.at = 0;
        }
        let n = (self.len - self.at).min(out.len());
        out[..n].copy_from_slice(&self.pending[self.at..self.at + n]);
        self.at += n;
        n
    }

    fn now_ms(&mut self) -> u64 { mind::time::uptime_ms() as u64 }
}

fn io(e: fs::Error) -> u8 {
    match e {
        fs::Error::NotFound => code::NOT_FOUND,
        fs::Error::Exists => code::EXISTS,
        fs::Error::NotEmpty => code::NOT_EMPTY,
        fs::Error::Denied | fs::Error::ReadOnly | fs::Error::Name | fs::Error::NotDirectory | fs::Error::IsDirectory | fs::Error::Invalid => code::DENIED,
        _ => code::IO,
    }
}

/// Removes directory `top` and everything in it, without a heap: the first entry each time, a file removed, a directory
/// entered, and each emptied directory removed on the way back up.
fn remove_tree(top: &str) -> Result<(), u8> {
    let mut at = [0u8; 320];
    let mut len = top.len();
    at[..len].copy_from_slice(top.as_bytes());
    loop {
        let current = core::str::from_utf8(&at[..len]).map_err(|_| code::IO)?;
        let mut first: Option<([u8; 255], usize, bool)> = None;
        fs::list(current, |e| {
            if first.is_some() || hidden(e.name_str(), false, false) { return; }
            let mut name = [0u8; 255];
            let n = e.name.len().min(255);
            name[..n].copy_from_slice(&e.name[..n]);
            first = Some((name, n, e.is_dir));
        }).map_err(io)?;
        let Some((name, n, is_dir)) = first else {
            fs::remove(current).map_err(io)?;
            if len == top.len() { return Ok(()); }
            len = at[..len].iter().rposition(|&b| b == b'/').unwrap_or(0);
            continue;
        };
        if len + 1 + n > at.len() { return Err(code::IO); }
        at[len] = b'/';
        at[len + 1..len + 1 + n].copy_from_slice(&name[..n]);
        if is_dir { len += 1 + n; } else { fs::remove(core::str::from_utf8(&at[..len + 1 + n]).map_err(|_| code::IO)?).map_err(io)?; }
    }
}

/// The programs link started for the master (800-LNK-0009): their output arrives at `inbox` (each holds a send copy in
/// SLOT_CONSOLE, `mind::output`'s 15-byte messages) and the last KEEP bytes of each are kept for OUTPUT. Their exit
/// notices arrive there too, through the loader's watch (`notices`, a receiving copy; 800-KRN-0138), and the loader
/// ends them on STOP. With a loader that cannot, the end is seen by `alive` and its reason stays unknown.
/// A COMMAND's line waits in `pending` for the shell until its answer is out (800-LNK-0015).
struct Programs { inbox: Option<Endpoint>, output: usize, notices: usize, allowed: bool, dev: bool, reboot_at: u64, next: u32, runs: [Run; RUNS], checked: u64,
                  pending: Option<(u32, [u8; COMMAND_MAX], usize)> }

/// Whether init granted link an endpoint in `slot` (the developer mode's clients, 800-LNK-0012).
fn holds(slot: usize) -> bool { mind::dev::cap_info(slot).0 == CAP_KIND_ENDPOINT }

/// The bytes of a command or arguments on one log line: libmind's line holds 200 with the head.
const LOGGED_PIECE: usize = 140;
const SECRET_NOT_LOGGED: &str = " [THE SECRET IS NOT LOGGED]";

/// A shell line as logged: whole, but for the secret `tpm seal` takes (shell/src/tpm.rs), which stays out of the log.
fn logged(line: &str) -> (&str, &str) {
    let first = line.trim_start();
    let w1 = first.find(char::is_whitespace).unwrap_or(first.len());
    let rest = first[w1..].trim_start();
    let w2 = rest.find(char::is_whitespace).unwrap_or(rest.len());
    if first[..w1].eq_ignore_ascii_case("tpm") && rest[..w2].eq_ignore_ascii_case("seal") {
        return (&line[..line.len() - rest.len() + w2], SECRET_NOT_LOGGED);
    }
    (line, "")
}

/// Logs `text` whole after `[LINK] <head> <id>: <label>`, in pieces a log line holds; the pieces after the first are
/// marked `…`, and `tail` ends the last.
fn log_whole(head: &str, id: u32, label: &str, text: &str, tail: &str) {
    let mut rest = text;
    let mut first = true;
    while first || !rest.is_empty() {
        let mut end = rest.len().min(LOGGED_PIECE);
        while !rest.is_char_boundary(end) { end -= 1; }
        let last = end == rest.len();
        if first { mind::println!("[LINK] {} {}: {}{}{}", head, id, label, &rest[..end], if last { tail } else { "" }); }
        else { mind::println!("[LINK] {} {} …: {}{}", head, id, &rest[..end], if last { tail } else { "" }); }
        rest = &rest[end..];
        first = false;
    }
}
struct Run { id: u32, pid: u64, state: u8, exit: u32, watched: bool, gone: u64, name: [u8; LOADER_PATH_MAX], name_len: usize, total: u64, kept: [u8; KEEP] }
const RUNS: usize = 8;
const KEEP: usize = 16 * 1024;
/// How long a watched program may be gone before its notice is given up on.
const NOTICE_WAIT_MS: u64 = 1000;
/// How long the shell may take over one line: a `ping` waits for its answers.
const COMMAND_WAIT_MS: u32 = 60_000;
/// What the shell's `run` answers at most (`idl/shell.wit`).
const COMMAND_OUTPUT: usize = 7000;
static mut COMMAND_OUT: [u8; COMMAND_OUTPUT] = [0; COMMAND_OUTPUT];
const NO_RUN: Run = Run { id: 0, pid: 0, state: RUN_ENDED, exit: EXIT_UNKNOWN, watched: false, gone: 0, name: [0; LOADER_PATH_MAX], name_len: 0, total: 0, kept: [0; KEEP] };

fn loader_code(error: loader::Error) -> u8 {
    match error {
        loader::Error::NotFound => code::NOT_FOUND,
        loader::Error::Invalid => code::BAD_REQUEST,
        loader::Error::Rights => code::DENIED,
        loader::Error::Unreadable => code::IO,
        loader::Error::NoMemory | loader::Error::Limit | loader::Error::Busy | loader::Error::Sessions => code::BUSY,
    }
}

impl Run {
    fn name(&self) -> &str { core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("?") }
    fn print(&mut self, bytes: &[u8]) {
        for &b in bytes { self.kept[(self.total % KEEP as u64) as usize] = b; self.total += 1; }
    }

    fn end(&mut self, state: u8, exit: u32) {
        let first = self.state == RUN_RUNNING;
        (self.state, self.exit) = (state, exit);
        if !first { return; }
        let how = match state { RUN_STOPPED => "STOPPED", RUN_FAULTED => "FAULTED", _ => "ENDED" };
        mind::println!("[LINK] RUN {} {}: {} (PID {}, EXIT {}, {} BYTES OF OUTPUT)", self.id, how, self.name(), self.pid, self.exit as i32, self.total);
    }
}

impl Programs {
    /// Output taken from the inbox, and the ends of programs seen; whether one still runs.
    fn pump(&mut self) -> bool {
        let Some(inbox) = self.inbox else { return false };
        let mut chunk = [0u8; mind::output::CHUNK];
        while let Ok(m) = inbox.recv_timeout(0, 1) {
            // A program's exit notice: it printed all it did before, since each piece waits for link to take it.
            if let Some(e) = m.exit {
                let Some(run) = self.runs.iter_mut().find(|r| r.id != 0 && r.pid == e.pid) else { continue };
                match mind::process::Exit::from_reason(e.reason) {
                    mind::process::Exit::Code(code) => run.end(if run.state == RUN_STOPPED { RUN_STOPPED } else { RUN_ENDED }, code),
                    mind::process::Exit::Killed => run.end(RUN_STOPPED, EXIT_UNKNOWN),
                    mind::process::Exit::Fault(vector) => run.end(RUN_FAULTED, vector as u32),
                }
                continue;
            }
            let n = mind::output::unpack(m.data, &mut chunk);
            if let Some(run) = self.runs.iter_mut().find(|r| r.id != 0 && r.pid == m.sender) { run.print(&chunk[..n]); }
        }
        let now = mind::time::uptime_ms() as u64;
        if now >= self.checked + 200 {
            self.checked = now;
            for run in self.runs.iter_mut().filter(|r| r.id != 0 && r.state == RUN_RUNNING && !mind::process::alive(r.pid)) {
                // A watched program's notice may still be on its way.
                if run.watched && run.gone == 0 { run.gone = now; continue; }
                if run.watched && now < run.gone + NOTICE_WAIT_MS { continue; }
                run.end(RUN_ENDED, EXIT_UNKNOWN);
            }
        }
        self.runs.iter().any(|r| r.id != 0 && r.state == RUN_RUNNING)
    }

    fn find(&mut self, id: u32) -> Result<&mut Run, u8> { self.runs.iter_mut().find(|r| r.id != 0 && r.id == id).ok_or(code::NOT_FOUND) }

    fn start(&mut self, full: &str, args: &str) -> Result<(u32, u64), u8> {
        if !self.allowed { mind::println!("[LINK] RUN {} REFUSED: link.conf HAS NO \"run\"", full); return Err(code::DENIED); }
        if self.inbox.is_none() { return Err(code::UNSUPPORTED); }
        if full.len() > LOADER_PATH_MAX { return Err(code::DENIED); }
        self.pump();
        let index = self.entry().ok_or(code::BUSY)?;
        let lost = |_| code::BUSY;
        let session = loader::begin(Endpoint::LOADER, full, args).map_err(lost)?.map_err(loader_code)?;
        if !matches!(loader::grant(Endpoint::LOADER, session, SLOT_CONSOLE as u8, self.output), Ok(Ok(()))) {
            let _ = loader::abort(Endpoint::LOADER, session);
            return Err(code::IO);
        }
        // Its exit notice to the same endpoint (loader.wit 1.11); an older loader leaves the end to `alive`.
        let watched = matches!(loader::watch(Endpoint::LOADER, session, self.notices), Ok(Ok(())));
        let (lent, flow) = if self.dev { self.lend(session, full) } else { ([""; 3], None) };
        let pid = loader::commit(Endpoint::LOADER, session).map_err(lost)?.map_err(loader_code)?;
        // The network grant ends with the program (as the shell binds its own).
        if let Some(badge) = flow { let _ = netpolicy::bind(Endpoint(SLOT_NETPOLICY), badge, pid); }
        let id = self.next;
        self.next = self.next.wrapping_add(1).max(1);
        let run = &mut self.runs[index];
        (run.id, run.pid, run.state, run.exit, run.watched, run.gone, run.total, run.name_len) = (id, pid, RUN_RUNNING, EXIT_UNKNOWN, watched, 0, 0, full.len());
        run.name[..full.len()].copy_from_slice(full.as_bytes());
        mind::println!("[LINK] RUN {}: STARTED {} AS PID {}{}{}{}{}", id, full, pid, if lent.iter().any(|l| !l.is_empty()) { ", LENT" } else { "" }, lent[0], lent[1], lent[2]);
        if !args.is_empty() { log_whole("RUN", id, "ARGUMENTS ", args, ""); }
        Ok((id, pid))
    }

    /// The developer mode's clients a program asks for (`mind::request!`) and init granted link, lent in its slots as
    /// the shell lends them: the station's (REQUEST_WIFI), the system monitor's (REQUEST_SYSINFO) and a network grant
    /// from the policy broker (REQUEST_NETWORK), which `start` binds to the program. What was lent, and the grant's badge.
    fn lend(&self, session: u32, full: &str) -> ([&'static str; 3], Option<u16>) {
        let requests = match loader::inspect_requests(Endpoint::LOADER, full) { Ok(Ok(r)) => r, _ => 0 };
        let give = |slot: usize, cap: usize| matches!(loader::grant(Endpoint::LOADER, session, slot as u8, cap), Ok(Ok(())));
        let mut lent = [""; 3];
        if requests & mind::process::REQUEST_WIFI != 0 && holds(SLOT_WIFI) && give(SLOT_WIFI, SLOT_WIFI) { lent[0] = " WIFI"; }
        if requests & mind::process::REQUEST_SYSINFO != 0 && holds(SLOT_SYSINFO) && give(SLOT_SYSINFO, SLOT_SYSINFO) { lent[1] = " SYSINFO"; }
        let mut flow = None;
        if requests & mind::process::REQUEST_NETWORK != 0 && holds(SLOT_NETPOLICY) {
            let file = full.rsplit('/').next().unwrap_or(full);
            let program = file.strip_suffix(".elf").unwrap_or(file);
            let broker = Endpoint(SLOT_NETPOLICY);
            match netpolicy::prepare(broker, program) {
                // Taken into SLOT_NETWORK, which link holds nothing in, lent from there and dropped: the loader keeps its copy.
                Ok(Ok(badge)) if matches!(netpolicy::take(broker, badge, SLOT_NETWORK), Ok(Ok(()))) => {
                    if give(SLOT_NETWORK, SLOT_NETWORK) { (lent[2], flow) = (" NETWORK", Some(badge)); }
                    let _ = ipc::drop_cap(SLOT_NETWORK);
                }
                Ok(Err(e)) => mind::println!("[LINK] NO NETWORK FOR {}: {:?}", program, e),
                _ => mind::println!("[LINK] NO NETWORK FOR {}: THE POLICY BROKER DID NOT ANSWER", program),
            }
        }
        (lent, flow)
    }

    /// A free run entry, else the one that ended longest ago.
    fn entry(&self) -> Option<usize> {
        self.runs.iter().position(|r| r.id == 0)
            .or_else(|| self.runs.iter().enumerate().filter(|(_, r)| r.state != RUN_RUNNING).min_by_key(|(_, r)| r.id.wrapping_sub(self.next)).map(|(i, _)| i))
    }

    /// COMMAND (800-LNK-0015): a run for `line`, handed to the shell's `run` by `main` once this answer is out, so that a
    /// long command does not hold it. The shell takes the link's lines by its own rule (800-APP-0102), through the client
    /// init grants with `dev` (800-KRN-0148). The whole line is logged (the maintainer's word, 2026-10-22), but for a
    /// secret `tpm seal` takes.
    fn command(&mut self, line: &str) -> Result<u32, u8> {
        let word = line.split_whitespace().next().unwrap_or("");
        let word = &word[..word.len().min(LOADER_PATH_MAX - 4)];
        let (shown, tail) = logged(line);
        if !self.dev { log_whole("COMMAND", 0, "REFUSED, link.conf HAS NO \"dev\": ", shown, tail); return Err(code::DENIED); }
        if !holds(SLOT_SHELL) { return Err(code::UNSUPPORTED); }
        if self.pending.is_some() { return Err(code::BUSY); }
        self.pump();
        let index = self.entry().ok_or(code::BUSY)?;
        let id = self.next;
        self.next = self.next.wrapping_add(1).max(1);
        let run = &mut self.runs[index];
        (run.id, run.pid, run.state, run.exit, run.watched, run.gone, run.total, run.name_len) = (id, 0, RUN_RUNNING, EXIT_UNKNOWN, false, 0, 0, 4 + word.len());
        run.name[..4].copy_from_slice(b"sh: ");
        run.name[4..4 + word.len()].copy_from_slice(word.as_bytes());
        let mut copy = [0u8; COMMAND_MAX];
        copy[..line.len()].copy_from_slice(line.as_bytes());
        self.pending = Some((id, copy, line.len()));
        log_whole("COMMAND", id, "", shown, tail);
        Ok(id)
    }

    /// The line waiting for the shell, run now: its answer is the run's output with exit 0, a refusal exit 1.
    fn run_pending(&mut self) {
        let Some((id, line, len)) = self.pending.take() else { return };
        let line = core::str::from_utf8(&line[..len]).unwrap_or("");
        let out: &'static mut [u8; COMMAND_OUTPUT] = unsafe { &mut *core::ptr::addr_of_mut!(COMMAND_OUT) };
        let answer = mind::idl::wire::with_timeout(COMMAND_WAIT_MS, || shell::run(Endpoint(SLOT_SHELL), line, &mut out[..]));
        let Ok(run) = self.find(id) else { return };
        match answer {
            Ok(Ok(n)) => { run.print(&out[..n]); run.end(RUN_ENDED, 0); }
            Ok(Err(e)) => {
                let why: &[u8] = match e {
                    shell::Error::Refused => b"the shell refused this line from the link\n",
                    shell::Error::Declined => b"the shell's user declined it\n",
                    shell::Error::Busy => b"the shell's window is busy with a program\n",
                    shell::Error::Unavailable => b"the shell cannot run it now\n",
                    shell::Error::NoMemory => b"the shell has no memory for it\n",
                    _ => b"the shell did not take this line\n",
                };
                run.print(why);
                run.end(RUN_ENDED, 1);
            }
            Err(_) => { run.print(b"the shell did not answer\n"); run.end(RUN_ENDED, 1); }
        }
    }

    /// REPLACE (800-LNK-0016): init starts `unit` again from `data/link/<path>`, or from its boot image for "", through the
    /// lifecycle client it grants with `dev` (800-KRN-0144); init logs the image NOT VERIFIED.
    fn replace(&mut self, unit: &str, path: &str) -> Result<(), u8> {
        if !self.dev { mind::println!("[LINK] REPLACE {} REFUSED: link.conf HAS NO \"dev\"", unit); return Err(code::DENIED); }
        if !holds(SLOT_LIFECYCLE) { return Err(code::UNSUPPORTED); }
        mind::println!("[LINK] REPLACE {} FROM {} ASKED BY THE MASTER", unit, if path.is_empty() { "ITS BOOT IMAGE" } else { path });
        match lifecycle::replace(Endpoint(SLOT_LIFECYCLE), unit, path) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(lifecycle::Error::Denied)) => Err(code::DENIED),
            Ok(Err(lifecycle::Error::NotFound)) => Err(code::NOT_FOUND),
            Ok(Err(_)) => Err(code::IO),
            Err(_) => Err(code::BUSY),
        }
    }

    /// A REBOOT from the master: the machine restarts once the answer is out (`main`), through init's lifecycle client
    /// badged for it, which init grants only with `dev`.
    fn reboot(&mut self) -> Result<(), u8> {
        if !self.dev { mind::println!("[LINK] REBOOT REFUSED: link.conf HAS NO \"dev\""); return Err(code::DENIED); }
        if !holds(SLOT_LIFECYCLE) { return Err(code::UNSUPPORTED); }
        mind::println!("[LINK] REBOOT ASKED BY THE MASTER");
        self.reboot_at = mind::time::uptime_ms() as u64 + 500;
        Ok(())
    }

    /// INPUT (800-LNK-0032): the master's events posted as the keyboard drivers post them, a word of 0 the attention key,
    /// through the input capability init gives link with `dev` (KRN). Without either, refused DENIED with none posted.
    fn input(&mut self, words: &[u64]) -> Result<u32, u8> {
        if !self.dev { mind::println!("[LINK] INPUT REFUSED: link.conf HAS NO \"dev\""); return Err(code::DENIED); }
        let mut posted = 0u32;
        for &w in words {
            let r = if w == 0 { mind::dev::input_key(0, 0, true) } else { mind::dev::input_key(w as usize, w as usize, false) };
            match r {
                Ok(()) => posted += 1,
                Err(mind::sys::Error::Rights) if posted == 0 => { mind::println!("[LINK] INPUT REFUSED: INIT GAVE LINK NO INPUT CAPABILITY"); return Err(code::DENIED); }
                Err(e) => { mind::println!("[LINK] INPUT: {} OF {} EVENTS POSTED, THEN {:?}", posted, words.len(), e); return Ok(posted); }
            }
        }
        mind::println!("[LINK] INPUT: {} EVENTS FROM THE MASTER", posted);
        Ok(posted)
    }

    fn output(&mut self, id: u32, cursor: u64, out: &mut [u8]) -> Result<RunOutput, u8> {
        self.pump();
        let run = self.find(id)?;
        let first = run.total.saturating_sub(KEEP as u64);
        let from = cursor.clamp(first, run.total);
        let n = ((run.total - from) as usize).min(out.len());
        for (i, b) in out[..n].iter_mut().enumerate() { *b = run.kept[((from + i as u64) % KEEP as u64) as usize]; }
        Ok(RunOutput { state: run.state, exit: run.exit, from, len: n })
    }

    fn list(&mut self, cursor: u32, out: &mut [u8]) -> Result<(usize, Option<u32>), u8> {
        self.pump();
        let (mut used, mut index) = (0usize, 0u32);
        for run in self.runs.iter().filter(|r| r.id != 0) {
            index += 1;
            if index - 1 < cursor { continue; }
            let mut line = Text::<160>::new();
            line.number(run.id as u64); line.push("\t"); line.number(run.pid); line.push("\t");
            line.push(match run.state { RUN_RUNNING => "running", RUN_STOPPED => "stopped", RUN_FAULTED => "faulted", _ => "ended" });
            line.push("\t"); line.push(run.name()); line.push("\n");
            if used + line.len > out.len() { return Ok((used, Some(index - 1))); }
            out[used..used + line.len].copy_from_slice(line.bytes());
            used += line.len;
        }
        Ok((used, None))
    }

    /// Ends a run's program through the loader, which ends only those it started for link (800-KRN-0138).
    fn stop(&mut self, id: u32) -> Result<(), u8> {
        self.pump();
        let run = self.find(id)?;
        if run.state != RUN_RUNNING { return Ok(()); }
        match loader::stop(Endpoint::LOADER, run.pid) {
            Ok(Ok(())) => { mind::println!("[LINK] RUN {}: STOP ASKED BY THE MASTER", run.id); run.end(RUN_STOPPED, EXIT_UNKNOWN); Ok(()) }
            // It ended meanwhile.
            Ok(Err(loader::Error::NotFound)) => Ok(()),
            Ok(Err(e)) => Err(loader_code(e)),
            Err(_) => Err(code::UNSUPPORTED),
        }
    }
}

static mut PROGRAMS: Programs = Programs { inbox: None, output: 0, notices: 0, allowed: false, dev: false, reboot_at: 0, next: 1, runs: [NO_RUN; RUNS], checked: 0, pending: None };
fn programs() -> &'static mut Programs { unsafe { &mut *core::ptr::addr_of_mut!(PROGRAMS) } }

/// The files of data/link/ and the system log. A put's data is gathered and written 16 KiB at a time, a get's read 16
/// KiB ahead: each request carries at most 1 KiB, and a write to the volume costs far more than its bytes.
/// `firmware` says which client is in use: data/link/'s, or data/firmware/'s (800-LNK-0027); a put and a get remember
/// theirs, since a handle answers only through the client it was opened with.
struct Store { put: Option<(File, [u8; 255], usize)>, get: Option<File>, buffer: [u8; CHUNK], start: usize, len: usize, known: [Known; KNOWN],
               firmware: bool, put_firmware: bool, get_firmware: bool }

/// The SHA-256 of files received here, so `stat` answers at once for them; the oldest is forgotten first.
#[derive(Clone, Copy)]
struct Known { name: [u8; 255], len: usize, size: u64, digest: [u8; 32] }
const KNOWN: usize = 8;
const UNKNOWN: Known = Known { name: [0; 255], len: 0, size: 0, digest: [0; 32] };

const CHUNK: usize = 16 * 1024;

impl Store {
    /// The client of `firmware`'s place in use: data/firmware/'s only where init granted it, in developer mode.
    fn scope(&mut self, firmware: bool) -> Result<(), u8> {
        if firmware && !holds(SLOT_FILE) { return Err(code::UNSUPPORTED); }
        if firmware != self.firmware {
            if firmware { fs::use_scope(Endpoint(SLOT_FILE), names::FIRMWARE) } else { fs::use_scope(Endpoint(SLOT_VFS), DIR) }
            self.firmware = firmware;
        }
        Ok(())
    }

    /// `name`'s path, with its place's client in use.
    fn place<'a>(&mut self, name: &str, out: &'a mut [u8; names::PATH_MAX]) -> Result<&'a str, u8> {
        self.scope(names::firmware(name))?;
        Ok(names::path(name, out))
    }

    fn forget(&mut self, name: &str) {
        for k in self.known.iter_mut().filter(|k| &k.name[..k.len] == name.as_bytes()) { *k = UNKNOWN; }
    }

    fn remember(&mut self, name: &str, size: u64, digest: &[u8; 32]) {
        self.forget(name);
        self.known.rotate_right(1);
        let k = &mut self.known[0];
        k.name[..name.len()].copy_from_slice(name.as_bytes());
        (k.len, k.size, k.digest) = (name.len(), size, *digest);
    }

    // The gathered bytes of a put written out.
    fn drain(&mut self) -> Result<(), u8> {
        let Some((file, _, _)) = self.put.as_mut() else { return Ok(()) };
        if self.len > 0 {
            match file.write_at(self.start, &self.buffer[..self.len]) { Ok(n) if n == self.len => {}, Ok(_) => return Err(code::IO), Err(e) => return Err(io(e)) }
        }
        self.start += self.len;
        self.len = 0;
        Ok(())
    }
}

impl Files for Store {
    fn create(&mut self, name: &str, size: u64) -> Result<(), u8> {
        if size > u32::MAX as u64 { return Err(code::IO); }
        self.put = None;
        // A put into a directory that is missing would fail only at its end.
        if !names::changeable(name) { return Err(code::DENIED); }
        let mut buf = [0u8; names::PATH_MAX];
        self.place(name, &mut buf)?;
        if let Some((parent, _)) = name.rsplit_once('/') {
            if !fs::metadata(names::path(parent, &mut buf)).is_ok_and(|m| m.is_dir) { return Err(code::NOT_FOUND); }
        }
        let file = File::create(names::part(name)).map_err(io)?;
        self.put_firmware = names::firmware(name);
        (self.start, self.len) = (0, 0);
        let mut kept = [0u8; 255];
        kept[..name.len()].copy_from_slice(name.as_bytes());
        mind::println!("[LINK] RECEIVING {} ({} BYTES)", name, size);
        self.put = Some((file, kept, name.len()));
        Ok(())
    }

    fn write_at(&mut self, offset: u64, data: &[u8]) -> Result<(), u8> {
        if self.put.is_none() || offset as usize != self.start + self.len { return Err(code::BAD_REQUEST); }
        self.scope(self.put_firmware)?;
        let mut rest = data;
        while !rest.is_empty() {
            let n = rest.len().min(CHUNK - self.len);
            self.buffer[self.len..self.len + n].copy_from_slice(&rest[..n]);
            self.len += n;
            rest = &rest[n..];
            if self.len == CHUNK { self.drain()?; }
        }
        Ok(())
    }

    fn commit(&mut self, digest: &[u8; 32], matches: bool) -> Result<(), u8> {
        self.scope(self.put_firmware)?;
        let drained = self.drain();
        let (file, kept, len) = self.put.take().ok_or(code::BAD_REQUEST)?;
        drained?;
        let name = core::str::from_utf8(&kept[..len]).map_err(|_| code::BAD_REQUEST)?;
        if !matches {
            drop(file);
            let _ = fs::remove(names::part(name));
            mind::println!("[LINK] {}: SHA-256 DOES NOT MATCH, NOT KEPT", name);
            return Ok(());
        }
        file.flush().map_err(io)?;
        drop(file);
        let mut buf = [0u8; names::PATH_MAX];
        let target = self.place(name, &mut buf)?;
        // A file of the same name is replaced.
        let _ = fs::remove(target);
        self.forget(name);
        fs::rename(names::part(name), target).map_err(io)?;
        self.remember(name, self.start as u64, digest);
        mind::println!("[LINK] RECEIVED {}: SHA-256 {}", name, Hex(digest));
        Ok(())
    }

    fn open(&mut self, name: &str) -> Result<u64, u8> {
        let mut buf = [0u8; names::PATH_MAX];
        let file = File::open(self.place(name, &mut buf)?).map_err(io)?;
        self.get_firmware = names::firmware(name);
        let size = file.size() as u64;
        mind::println!("[LINK] SENDING {} ({} BYTES)", name, size);
        self.get = Some(file);
        (self.start, self.len) = (0, 0);
        Ok(size)
    }

    fn read_at(&mut self, offset: u64, out: &mut [u8]) -> Result<usize, u8> {
        if self.get.is_none() { return Err(code::BAD_REQUEST); }
        self.scope(self.get_firmware)?;
        let file = self.get.as_ref().ok_or(code::BAD_REQUEST)?;
        let offset = offset as usize;
        if offset < self.start || offset >= self.start + self.len {
            self.len = match file.read_at(offset, &mut self.buffer) { Ok(0) => return Err(code::IO), Ok(n) => n, Err(e) => return Err(io(e)) };
            self.start = offset;
        }
        let at = offset - self.start;
        let n = out.len().min(self.len - at);
        out[..n].copy_from_slice(&self.buffer[at..at + n]);
        Ok(n)
    }

    fn stat(&mut self, name: &str) -> Result<(u64, [u8; 32]), u8> {
        let mut buf = [0u8; names::PATH_MAX];
        let file = File::open(self.place(name, &mut buf)?).map_err(io)?;
        if let Some(k) = self.known.iter().find(|k| k.len > 0 && &k.name[..k.len] == name.as_bytes() && k.size == file.size() as u64) { return Ok((k.size, k.digest)); }
        let mut sha = mind::sha256::Sha256::new();
        let mut chunk = [0u8; 4096];
        let mut at = 0;
        while at < file.size() {
            let n = file.read_at(at, &mut chunk).map_err(io)?;
            if n == 0 { return Err(code::IO); }
            sha.update(&chunk[..n]);
            at += n;
        }
        Ok((at as u64, sha.finish()))
    }

    fn delete(&mut self, name: &str) -> Result<(), u8> {
        if !names::changeable(name) { return Err(code::DENIED); }
        let mut buf = [0u8; names::PATH_MAX];
        fs::remove(self.place(name, &mut buf)?).map_err(io)?;
        self.forget(name);
        mind::println!("[LINK] DELETED {}", name);
        Ok(())
    }

    fn list(&mut self, cursor: u32, out: &mut [u8]) -> Result<(usize, Option<u32>), u8> {
        let (mut used, mut index, mut next) = (0usize, 0u32, None);
        self.scope(false)?;
        fs::list(DIR, |e| {
            if e.is_dir || next.is_some() || hidden(e.name_str(), true, false) { return; }
            index += 1;
            if index - 1 < cursor { return; }
            let mut line = Text::<300>::new();
            line.push(e.name_str()); line.push("\t"); line.number(e.size as u64); line.push("\n");
            if used + line.len > out.len() { next = Some(index - 1); return; }
            out[used..used + line.len].copy_from_slice(line.bytes());
            used += line.len;
        }).map_err(io)?;
        Ok((used, next))
    }

    fn mkdir(&mut self, name: &str) -> Result<(), u8> {
        if !names::changeable(name) { return Err(code::DENIED); }
        let mut buf = [0u8; names::PATH_MAX];
        let target = self.place(name, &mut buf)?;
        if fs::metadata(target).is_ok_and(|m| !m.is_dir) { return Err(code::EXISTS); }
        fs::mkdir(target).map_err(io)?;
        mind::println!("[LINK] MADE {}/", name);
        Ok(())
    }

    fn rename(&mut self, from: &str, to: &str, replace: bool) -> Result<(), u8> {
        if to.len() > from.len() && to.starts_with(from) && to.as_bytes()[from.len()] == b'/' { return Err(code::DENIED); }
        // Each place is its own client: a move between them is refused, and a put goes straight to its place.
        if !names::one_place(from, to) { return Err(code::DENIED); }
        let (mut a, mut b) = ([0u8; names::PATH_MAX], [0u8; names::PATH_MAX]);
        let source = self.place(from, &mut a)?;
        let target = names::path(to, &mut b);
        fs::metadata(source).map_err(io)?;
        match fs::metadata(target) {
            Ok(m) if m.is_dir || !replace => return Err(code::EXISTS),
            Ok(_) => fs::remove(target).map_err(io)?,
            Err(_) => {}
        }
        fs::rename(source, target).map_err(io)?;
        self.known = [UNKNOWN; KNOWN];
        mind::println!("[LINK] MOVED {} TO {}", from, to);
        Ok(())
    }

    fn rmdir(&mut self, name: &str, recursive: bool) -> Result<(), u8> {
        if !names::changeable(name) { return Err(code::DENIED); }
        let mut buf = [0u8; names::PATH_MAX];
        let target = self.place(name, &mut buf)?;
        if !fs::metadata(target).map_err(io)?.is_dir { return Err(code::DENIED); }
        if recursive { remove_tree(target)?; } else { fs::remove(target).map_err(io)?; }
        self.known = [UNKNOWN; KNOWN];
        mind::println!("[LINK] REMOVED {}/{}", name, if recursive { " AND ALL IN IT" } else { "" });
        Ok(())
    }

    fn list_dir(&mut self, name: &str, cursor: u32, out: &mut [u8]) -> Result<(usize, Option<u32>), u8> {
        let mut buf = [0u8; names::PATH_MAX];
        let firmware_held = holds(SLOT_FILE);
        let (dir, top) = if name.is_empty() { self.scope(false)?; (DIR, true) } else { (self.place(name, &mut buf)?, name == names::FIRMWARE_NAME) };
        let (mut used, mut index, mut next) = (0usize, 0u32, None);
        fs::list(dir, |e| {
            if next.is_some() || hidden(e.name_str(), top, firmware_held && name.is_empty()) { return; }
            index += 1;
            if index - 1 < cursor { return; }
            let mut line = Text::<300>::new();
            line.push(e.name_str());
            if e.is_dir { line.push("/\t0\n"); } else { line.push("\t"); line.number(e.size as u64); line.push("\n"); }
            if used + line.len > out.len() { next = Some(index - 1); return; }
            out[used..used + line.len].copy_from_slice(line.bytes());
            used += line.len;
        }).map_err(io)?;
        // data/firmware/ last at the top, where init granted it (800-LNK-0027).
        if name.is_empty() && firmware_held && next.is_none() {
            index += 1;
            let line = b"firmware/\t0\n";
            if index - 1 >= cursor {
                if used + line.len() > out.len() { next = Some(index - 1); } else { out[used..used + line.len()].copy_from_slice(line); used += line.len(); }
            }
        }
        Ok((used, next))
    }

    fn run(&mut self, name: &str, args: &str) -> Result<(u32, u64), u8> {
        let mut buf = [0u8; names::PATH_MAX];
        // In the developer mode a plain name (`wifi`, `netcheck`) is a built-in program, which the loader finds by name.
        if programs().dev && !name.contains('/') && !name.contains('.') { return programs().start(name, args); }
        // The loader starts programs from data/link/ alone.
        if names::firmware(name) { return Err(code::DENIED); }
        programs().start(names::path(name, &mut buf), args)
    }

    fn reboot(&mut self) -> Result<(), u8> { programs().reboot() }

    fn input(&mut self, words: &[u64]) -> Result<u32, u8> { programs().input(words) }

    fn command(&mut self, line: &str) -> Result<u32, u8> { programs().command(line) }

    fn replace(&mut self, unit: &str, path: &str) -> Result<(), u8> {
        // init reads a replacing image from data/link/ alone.
        if names::firmware(path) { return Err(code::DENIED); }
        programs().replace(unit, path)
    }

    fn output(&mut self, run: u32, cursor: u64, out: &mut [u8]) -> Result<RunOutput, u8> { programs().output(run, cursor, out) }

    fn stop(&mut self, run: u32) -> Result<(), u8> { programs().stop(run) }

    fn runs(&mut self, cursor: u32, out: &mut [u8]) -> Result<(usize, Option<u32>), u8> { programs().list(cursor, out) }

    fn log(&mut self, cursor: u64, out: &mut [u8]) -> Result<(usize, u64), u8> {
        let (mut used, mut next, mut full) = (0usize, cursor, false);
        while !full {
            let seen = mind::log::read(next, |e| {
                if full { return; }
                let mut line = Text::<300>::new();
                line.number(e.time_ms / 1000); line.push("."); line.number_width(e.time_ms % 1000, 3); line.push(" ");
                line.push(e.name.as_str()); line.push(": "); line.push(e.text.as_str()); line.push("\n");
                // A line longer than a whole answer is cut, so the log never stalls.
                let n = if used == 0 { line.len.min(out.len()) } else { line.len };
                if used + n > out.len() { full = true; return; }
                out[used..used + n].copy_from_slice(&line.bytes()[..n]);
                used += n;
                next = e.seq + 1;
            }).map_err(|_| code::DENIED)?;
            if seen == 0 { break; }
        }
        Ok((used, next))
    }
}

/// A line of text built without a heap.
struct Text<const N: usize> { buf: [u8; N], len: usize }
impl<const N: usize> Text<N> {
    fn new() -> Self { Self { buf: [0; N], len: 0 } }
    fn push(&mut self, s: &str) { let n = s.len().min(N - self.len); self.buf[self.len..self.len + n].copy_from_slice(&s.as_bytes()[..n]); self.len += n; }
    fn number(&mut self, v: u64) { self.number_width(v, 1) }
    fn number_width(&mut self, mut v: u64, width: usize) {
        let mut digits = [0u8; 20]; let mut n = 0;
        while v > 0 || n < width { digits[n] = b'0' + (v % 10) as u8; v /= 10; n += 1; }
        for &d in digits[..n].iter().rev() { if self.len < N { self.buf[self.len] = d; self.len += 1; } }
    }
    fn bytes(&self) -> &[u8] { &self.buf[..self.len] }
}

struct Hex<'a>(&'a [u8]);
impl core::fmt::Display for Hex<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result { for b in self.0 { write!(f, "{:02x}", b)?; } Ok(()) }
}

static mut STORE: Store = Store { put: None, get: None, buffer: [0; CHUNK], start: 0, len: 0, known: [UNKNOWN; KNOWN], firmware: false, put_firmware: false, get_firmware: false };

/// The service with each hello and refusal logged.
struct Logged(FileService<&'static mut Store>);
impl Service for Logged {
    fn handle(&mut self, ch: u8, k: u8, p: &[u8], reply: &mut [u8]) -> Result<usize, u8> {
        if (ch, k) == (channel::CONTROL, kind::HELLO) {
            let name = p.get(4..).and_then(|n| core::str::from_utf8(n).ok()).unwrap_or("?");
            mind::println!("[LINK] HELLO FROM \"{}\"", name);
        }
        let r = self.0.handle(ch, k, p, reply);
        if let Err(c) = r { mind::println!("[LINK] REFUSED CHANNEL {} KIND {}: CODE {}", ch, k, c); }
        r
    }
}

/// The link's mark on the screen (800-LNK-0020): grey with no master heard for PEER_MS, red while one is and `dev` lets
/// it change the machine, yellow while it may only read. The compositor shows it a few seconds past the last call.
struct Mark { requests: u32, heard: u64, next: u64, on: bool, said: Option<display::RemoteState> }
impl Mark {
    fn tick(&mut self, requests: u32, dev: bool) {
        let now = mind::time::uptime_ms() as u64;
        if requests != self.requests { (self.requests, self.heard) = (requests, now); }
        if !self.on || now < self.next { return; }
        self.next = now + MARK_MS;
        let state = if self.heard == 0 || now - self.heard > PEER_MS { display::RemoteState::Unconnected }
                    else if dev { display::RemoteState::Changing } else { display::RemoteState::Reading };
        match display::remote(Endpoint(SLOT_DISPLAY), state) {
            Ok(Ok(())) => if self.said != Some(state) { mind::println!("[LINK] MARK: {:?}", state); self.said = Some(state); },
            // An older compositor, or no grant: said once, then not asked again.
            other => { mind::println!("[LINK] NO MARK ON THE SCREEN: {:?}", other.map(|r| r.err())); self.on = false; }
        }
    }
}

/// The rate from the arguments (`baud=<rate>`), else 115200.
fn baud() -> u32 {
    mind::process::args_str().split_ascii_whitespace().find_map(|w| w.strip_prefix("baud=")?.parse().ok()).unwrap_or(FIRST_BAUD)
}

/// This machine's address on a line shared by several (800-LNK-0026): `address=<1..247>` in link.conf. None, or one out
/// of range (said once), is a line with one slave, answered in version 1 frames as before.
fn address() -> u8 {
    let Some(word) = mind::process::args_str().split_ascii_whitespace().find_map(|w| w.strip_prefix("address=")) else { return 0 };
    match word.parse::<u8>() {
        Ok(a) if (1..=linkproto::frame::ADDRESS_MAX).contains(&a) => a,
        _ => { mind::println!("[LINK] address={} IS NOT 1 TO {}: SERVING A LINE OF ONE", word, linkproto::frame::ADDRESS_MAX); 0 }
    }
}

/// Programs may be started only with the word `run` in link.conf; their output comes to an endpoint of link's.
fn prepare_programs() {
    let p = programs();
    p.dev = mind::process::args_str().split_ascii_whitespace().any(|w| w == "dev");
    // The developer mode starts programs too.
    p.allowed = p.dev || mind::process::args_str().split_ascii_whitespace().any(|w| w == "run");
    if p.dev {
        let has = |slot: usize, what: &'static str| if holds(slot) { what } else { "" };
        mind::println!("[LINK] DEVELOPER MODE (link.conf: dev): BUILT-IN PROGRAMS BY NAME; FROM INIT:{}{}{}{}{}", has(SLOT_NETPOLICY, " NETWORK"),
                       has(SLOT_WIFI, " WIFI"), has(SLOT_SYSINFO, " SYSINFO"), has(SLOT_LIFECYCLE, " REBOOT"), has(SLOT_FILE, " FIRMWARE (data/firmware/ AS firmware/)"));
    }
    p.next = (mind::time::rdtsc() as u32 & 0xFFFF_0000) | 1;
    if !p.allowed { return; }
    // One endpoint (init's quota, 800-KRN-0137): a send copy for the programs' output, a receiving one for the loader's watch.
    match Endpoint::create().and_then(|inbox| Ok((inbox, ipc::mint(inbox.0, CAP_WRITE, 0, 0)?, ipc::mint(inbox.0, CAP_READ, 0, 0)?))) {
        Ok((inbox, output, notices)) => {
            (p.inbox, p.output, p.notices) = (Some(inbox), output, notices);
            mind::println!("[LINK] PROGRAMS IN data/link/ MAY BE STARTED (link.conf: run)");
        }
        Err(e) => mind::println!("[LINK] NO ENDPOINT FOR PROGRAMS' OUTPUT ({:?}): NONE WILL BE STARTED", e),
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("link — MIND Link's end on this machine: files in data/link/ and its directories, the system log, and with the word run in link.conf the programs in data/link/, for mindlink on another machine, over a serial converter; init starts it when the boot volume holds link.conf (800-LNK-0003).");
    let endpoint = Endpoint(SLOT_DEV0);
    fs::use_scope(Endpoint(SLOT_VFS), DIR);
    let baud = baud();
    let mut said = false;
    loop {
        match serial::configure(endpoint, baud) {
            Ok(Ok(actual)) => { mind::println!("[LINK] SERVING data/link/ AND THE LOG AT {} BIT/S (THE CONVERTER MAKES {})", baud, actual); break; }
            Ok(Err(serial::Error::Invalid)) => { mind::println!("[LINK] {} BIT/S IS NOT A RATE THE CONVERTER MAKES", baud); return; }
            Ok(Err(_)) => { if !said { mind::println!("[LINK] WAITING FOR A SERIAL CONVERTER"); said = true; } mind::time::sleep(1000); }
            // No usb_serial behind the slot: started by hand, not by init.
            Err(_) => { mind::println!("link: no serial port was given to it; init starts it when the boot volume holds link.conf"); return; }
        }
    }
    prepare_programs();
    let store: &'static mut Store = unsafe { &mut *core::ptr::addr_of_mut!(STORE) };
    let mut service = Logged(FileService::new(store));
    service.0.set_name("MIND Core link");
    let mut slave = Slave::new(Port { endpoint, pending: [0; 4096], at: 0, len: 0 });
    slave.address = address();
    if slave.address != 0 { mind::println!("[LINK] ADDRESS {} ON A SHARED LINE: ONLY FRAMES TO IT ARE ANSWERED", slave.address); }
    let mut dropped = 0;
    let mut mark = Mark { requests: 0, heard: 0, next: 0, on: holds(SLOT_DISPLAY), said: None };
    loop {
        if slave.serve(&mut service, 1000).is_err() { mind::time::sleep(100); }
        mark.tick(slave.stats.requests, programs().dev);
        // A REBOOT is carried out once its answer has gone; the line has had time to turn round.
        let p = programs();
        // So is a COMMAND's line, whose output the master then reads.
        p.run_pending();
        if p.reboot_at != 0 && mind::time::uptime_ms() as u64 >= p.reboot_at {
            p.reboot_at = 0;
            match lifecycle::reboot(Endpoint(SLOT_LIFECYCLE)) {
                Ok(Ok(())) => {}
                other => mind::println!("[LINK] REBOOT NOT DONE: {:?}", other.map(|r| r.is_ok())),
            }
        }
        // Damaged frames say the line is noisy or the rates differ.
        if slave.stats.dropped != dropped {
            dropped = slave.stats.dropped;
            mind::println!("[LINK] {} DAMAGED FRAMES DROPPED SO FAR", dropped);
        }
    }
}
