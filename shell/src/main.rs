#![no_std]
#![no_main]
// Command shell in ring 3: text console on its own screen and COM1, commands over process control, loader and init.
// It owns the focus: programs it brings to the foreground get the keyboard, and focus returns to it on exit or Ctrl+Z.
mod bmp;
mod console;
mod files;
mod keymap;
mod net;
mod observe;
mod power;
mod screenshot;

use console::{Console, Position, COM1};
use core::fmt::Write;
use mind::abi::*;
use mind::control::{self, Notice};
use mind::dev::{input_key, Ports};
use mind::idl::{init as idl_init, loader};
use mind::input::{Code, Key};
use mind::tui::widgets::{Edit, History, InputLine};
use mind::ipc::Endpoint;
use mind::keys::{Event, Vt};
use mind::mem::Pages;
use mind::sys::Error;

// Words the shell completes with Tab besides program names.
const COMMANDS: [&str; 43] = ["boot", "budget", "caps", "cat", "clear", "clock", "cpus", "date", "devices", "endpoints", "faults", "fetch", "fg", "free", "heap", "help", "ip", "irqs", "keymap", "kill", "list", "logger", "logs", "ls", "mkdir", "mv", "net", "netgrants", "netrevoke", "nslookup", "physmap", "ping", "pmap", "ps", "quotas", "reboot", "rm", "run", "screenshot", "stat", "stop", "sync", "write"];
const NAMES: usize = 64;
// Where the scoped VFS client for a program that asks for a file arrives: a fixed slot the shell does not use (11 is
// SLOT_LIFECYCLE in applications). The shell lends it to the program and drops its own copy.
const SCOPE_RECEIVE: usize = 11;

struct Shell {
    term: Console, line: InputLine, history: History<32>, prompt_at: Position, own: u64, focused: Option<u64>, line_start: bool,
    console: Option<u64>, // console program running in the shell
    names: [[u8; NAME_MAX]; NAMES], name_lens: [usize; NAMES], name_count: usize, // program names for completion
}

fn pid_arg(args: &[u8]) -> Option<u64> {
    if args.is_empty() { return None; }
    let mut pid = 0u64;
    for &byte in args {
        if !byte.is_ascii_digit() { return None; }
        pid = pid.checked_mul(10)?.checked_add((byte - b'0') as u64)?;
    }
    (pid != 0).then_some(pid)
}

// Text for a failed start of a program or service.
fn error_text(error: Error, service: bool) -> &'static str {
    match error {
        Error::Other(ERR_BUSY) => "SERVICE ALREADY RUNNING",
        Error::Other(ERR_LIMIT) => "TASK LIMIT REACHED (8)",
        Error::NotFound if service => "SERVICE NOT AVAILABLE ON THIS MACHINE",
        Error::NotFound => "UNKNOWN PROGRAM. TYPE LIST TO SEE PROGRAMS.",
        Error::Invalid if service => "SERVICES TAKE NO ARGUMENTS",
        Error::Invalid => "NOT A PROGRAM FILE",
        Error::NoMemory => "OUT OF MEMORY",
        Error::Peer => if service { "INIT NOT RUNNING" } else { "LOADER NOT RUNNING" },
        _ => "LOAD FAILED",
    }
}

fn label(bytes: &[u8]) -> &str { core::str::from_utf8(bytes).unwrap_or("?").trim_end_matches([' ', '\0']) }

fn tasks() -> ([TaskInfo; 40], usize) {
    let mut list = [unsafe { core::mem::zeroed::<TaskInfo>() }; 40];
    let count = control::tasks(&mut list).unwrap_or(0);
    (list, count)
}

impl Shell {
    fn report(&mut self, error: &str) { let _ = writeln!(self.term, "ERROR: {}", error); }
    // The prompt and whatever was typed so far (output may have interrupted the line).
    fn prompt(&mut self) {
        let _ = write!(self.term, "MIND> ");
        self.prompt_at = self.term.position();
        if !self.line.is_empty() { self.redraw_input(true); }
    }

    // Output of the focused program goes to COM1 only, each line prefixed with its PID.
    fn mirror(&mut self, pid: u64) {
        let mut buffer = [0u8; 1024];
        while let Ok(len @ 1..) = control::console(pid, &mut buffer) {
            for &byte in &buffer[..len] {
                if self.line_start && byte != b'\r' && byte != b'\n' { let mut prefix = mind::util::FixedBuf::<32>::new(); let _ = write!(prefix, "[PID {}] ", pid); for &b in prefix.as_bytes() { self.term.serial(b); } }
                self.term.serial(byte);
                self.line_start = byte == b'\n';
            }
        }
    }

    // Kernel statistics (STAT), one record per line.
    fn stat(&mut self, args: &[u8]) {
        let text = core::str::from_utf8(args).unwrap_or("");
        let mut words = text.split_whitespace();
        let (name, pid) = (words.next().unwrap_or(""), words.next().and_then(|w| w.parse::<usize>().ok()).unwrap_or(0));
        let class = match name { "tasks" => STAT_TASKS, "cpus" => STAT_CPUS, "memory" => STAT_MEMORY, "physmap" => STAT_PHYSMAP, "vmap" => STAT_VMAP, "caps" => STAT_CAPS, "endpoints" => STAT_ENDPOINTS, "irqs" => STAT_IRQS, "devices" => STAT_DEVICES, _ => { self.report("STAT <CLASS> [PID]: SEE HELP"); return; } };
        let Some(mut page) = Pages::new(4 * 4096) else { self.report("OUT OF MEMORY"); return };
        // `stat memory` also asks for the largest free block (argument 1).
        let pid = if class == STAT_MEMORY { 1 } else { pid };
        let header = match control::stat(class, pid, page.as_mut_slice()) { Ok(h) => h, Err(error) => { self.report(if error == Error::NotFound { "NO SUCH PID" } else { "STAT FAILED" }); return; } };
        let buffer = page.as_slice(); let t = &mut self.term;
        let _ = writeln!(t, "STAT {} VERSION={} COUNT={} TOTAL={}", Upper(name), header.version, header.count, header.total);
        match class {
            STAT_TASKS => for r in control::records::<StatTask>(buffer, header) { let _ = writeln!(t, "{} PARENT={} {} WAIT={}:{} CPU={} RUN_MS={} SENDS={} RECEIVES={} HEAP={} SHARED={} CAPS={} KERNEL={}{}", r.pid, r.parent, label(&r.name), r.wait, r.wait_on, r.cpu, r.run_ns / 1_000_000, r.sends, r.receives, r.heap_bytes, r.shared_bytes, r.caps, r.kernel_bytes, if r.focus != 0 { " FOCUS" } else { "" }); },
            STAT_CPUS => for (i, r) in control::records::<StatCpu>(buffer, header).enumerate() { let _ = writeln!(t, "CPU {} APIC={} ONLINE={} BUSY_MS={} IDLE_MS={} INTERRUPTS={} SWITCHES={} PID={}", i, r.apic_id, r.online, r.busy_ns / 1_000_000, r.idle_ns / 1_000_000, r.interrupts, r.switches, r.current_pid); },
            STAT_MEMORY => for r in control::records::<StatMemory>(buffer, header) { let _ = writeln!(t, "ARENA={} USED={} FREE={} LARGEST={} IMAGES={} STACKS={} TASK_PAGES={} PAGE_TABLES={} SCREENS={} HEAPS={} SHARED={} OBJECTS={} DMA={} TASKS={}/{} ENDPOINTS={}/{}", r.arena, r.used, r.free, r.largest_free, r.images, r.stacks, r.task_pages, r.page_tables, r.screens, r.heaps, r.shared, r.objects, r.dma, r.tasks, r.tasks_limit, r.endpoints, r.endpoints_limit); },
            STAT_PHYSMAP => for r in control::records::<StatPhys>(buffer, header).filter(|r| r.kind >= PHYS_PLATFORM) { let _ = writeln!(t, "KIND={:#x} INDEX={} START={:#x} PAGES={}", r.kind, r.index, r.start, r.pages); },
            STAT_VMAP => for r in control::records::<StatRegion>(buffer, header) {
                let kind = ["?", "IMAGE", "STACK", "SCREEN", "INFO", "MAILBOX", "EXIT", "HEAP", "SHARED", "DEVICE", "GUARD"].get(r.kind as usize).copied().unwrap_or("?");
                let _ = writeln!(t, "{:#x} {} {} {}{}{}", r.start, r.size, kind, if r.flags & REGION_READ != 0 { 'R' } else { '-' }, if r.flags & REGION_WRITE != 0 { 'W' } else { '-' }, if r.flags & REGION_EXECUTE != 0 { 'X' } else { '-' });
            },
            STAT_CAPS => for r in control::records::<StatCap>(buffer, header) { let _ = writeln!(t, "SLOT={} GEN={} KIND={} RIGHTS={} SIZE={} BADGE={} EP={} NODE={} PARENT={}", r.slot, r.generation, r.kind, r.rights, r.size, r.badge, r.endpoint, r.node, r.parent); },
            STAT_ENDPOINTS => for r in control::records::<StatEndpoint>(buffer, header) { let _ = writeln!(t, "EP {} RECEIVERS={} SENDERS={} WAITING={} CREATOR={} SERVER={} HOLDERS={} IRQ={} MESSAGES={} BUSY={} TIMEOUTS={}", r.index, r.receivers, r.waiting_senders, r.waiting_receivers, r.creator, r.server, r.holders, r.irq, r.messages, r.busy, r.timeouts); },
            STAT_IRQS => for r in control::records::<StatIrq>(buffer, header) { let _ = writeln!(t, "IRQ {} ENDPOINT={} MASKED={} HOLDER={} HOLDERS={} COUNT={}", r.line, r.endpoint, r.masked, r.holder, r.holders, r.count); },
            _ => for r in control::records::<StatDevice>(buffer, header) { let _ = writeln!(t, "DEVICE {:02x}:{:02x}.{} CLASS={:06x} IRQ={} HOLDER={} BARS={:?} IO_BARS={:#x}", r.location >> 8, r.location >> 3 & 31, r.location & 7, r.class, r.irq, r.holder, r.bar_sizes, r.io_bars); },
        }
    }

    fn focus(&mut self, pid: u64, keep_output: bool) -> Result<(), Error> {
        control::focus(pid, keep_output)?;
        self.focused = Some(pid); self.line_start = true;
        Ok(())
    }

    fn list_programs(&mut self) {
        // idl/loader.wit: a typed list instead of text.
        match mind::idl::loader::list(Endpoint::LOADER) {
            Ok(programs) => {
                let _ = writeln!(self.term, "PROGRAMS ON DISK:");
                for p in programs.as_slice() { let _ = writeln!(self.term, "  {:<12} {} BYTES{}", p.name.as_str(), p.size, if p.service { " (SERVICE)" } else { "" }); }
            }
            Err(_) => self.report("CANNOT LIST THE BOOT DISK"),
        }
        let _ = write!(self.term, "SERVICES (STARTED AT BOOT): ");
        for name in BOOT_SERVICES { let _ = write!(self.term, "{} ", name); }
        let _ = writeln!(self.term, "\nUSE: RUN <NAME> [&]. CTRL+Z: BACKGROUND. ESC: EXIT.");
    }

    // Boot services are (re)started by init. Applications are started by the loader in a launch session: the shell, as
    // the user's agent, gives a program what it asks for in its ELF and the shell itself holds (MC-3.11): a sysmon
    // client for `REQUEST_SYSINFO`, a VFS client confined to the named file's directory for `REQUEST_FILE` (its own
    // VFS client, which writes on `ram:` and in `data/`, for `REQUEST_FILES`), its log
    // client (reads the system log) for `REQUEST_LOG`, its client of init (lifecycle control) for `REQUEST_LIFECYCLE`,
    // its sysmon client with the authority badge (who holds what) for `REQUEST_AUTHORITY`, in SLOT_SYSINFO in place of
    // the plain one. Nothing is granted by program name.
    fn start(&mut self, name: &[u8], args: &[u8], service: bool) -> Result<u64, Error> {
        if service {
            if !args.is_empty() { return Err(Error::Invalid); }
            let name = core::str::from_utf8(name).map_err(|_| Error::Invalid)?;
            return idl_init::run(Endpoint::INIT, name);
        }
        let name = core::str::from_utf8(name).map_err(|_| Error::Invalid)?;
        let args = core::str::from_utf8(args).map_err(|_| Error::Invalid)?;
        let failed = |error: loader::Error| match error {
            loader::Error::NotFound => Error::NotFound, loader::Error::Invalid => Error::Invalid, loader::Error::NoMemory => Error::NoMemory,
            loader::Error::Rights => Error::Rights, loader::Error::Limit => Error::Other(ERR_LIMIT), loader::Error::Busy | loader::Error::Sessions => Error::Other(ERR_BUSY),
        };
        let needs = loader::inspect(Endpoint::LOADER, name)?.map_err(failed)?;
        // Requests `needs` has no field for (loader.wit 1.2): the network (issue 102), the authority client (issue 081).
        let requests = loader::inspect_requests(Endpoint::LOADER, name)?.map_err(failed)?;
        let authority = requests & mind::process::REQUEST_AUTHORITY != 0;
        let session = loader::begin(Endpoint::LOADER, name, args)?.map_err(failed)?;
        // A program that asks for a file gets a client confined to the file's directory (`ram:` without a file),
        // writable where the user may write; one that asks for the user's files gets the shell's own client.
        let scoped = needs.file && !needs.files;
        if scoped {
            let path = args.split_whitespace().next().unwrap_or("");
            let (volume, rest) = if path.is_empty() { ("ram", "") } else { mind::fs::split(path) };
            let parent = rest.rfind('/').map_or("", |i| &rest[..i]);
            let made = mind::fs::Dir::root(volume).and_then(|root| root.dir(parent, false)).and_then(|dir| dir.scope(true, SCOPE_RECEIVE));
            if let Err(error) = made { let _ = loader::abort(Endpoint::LOADER, session); return Err(Error::from(error)); }
        }
        let lend = |slot: usize, cap: usize| loader::grant(Endpoint::LOADER, session, slot as u8, cap).map(|r| r.map_err(failed));
        let wanted = [(needs.sysinfo && !authority, SLOT_SYSINFO, SLOT_SYSINFO), (authority, SLOT_SYSINFO, SLOT_AUTHORITY), (scoped, SLOT_FILE, SCOPE_RECEIVE), (needs.files, SLOT_FILE, SLOT_VFS), (needs.log, SLOT_LOG, SLOT_LOG),
                      (needs.lifecycle, SLOT_LIFECYCLE, SLOT_INIT)];
        let lent = wanted.iter().filter(|w| w.0).try_for_each(|&(_, slot, cap)| match lend(slot, cap) { Ok(Ok(())) => Ok(()), Err(error) | Ok(Err(error)) => Err(error) });
        if scoped { let _ = mind::ipc::drop_cap(SCOPE_RECEIVE); } // the loader holds its copy now
        if let Err(error) = lent { let _ = loader::abort(Endpoint::LOADER, session); return Err(error); }
        // A program that asks for the network gets what the policy broker grants it, or runs without (issue 102).
        let mut network = None;
        if requests & mind::process::REQUEST_NETWORK != 0 {
            match net::grant(&mut self.term, name, SCOPE_RECEIVE, |cap| match lend(SLOT_NETWORK, cap) { Ok(Ok(())) => Ok(()), Err(error) | Ok(Err(error)) => Err(error) }) {
                Ok(badge) => network = badge,
                Err(error) => { let _ = loader::abort(Endpoint::LOADER, session); return Err(error); }
            }
        }
        let pid = loader::commit(Endpoint::LOADER, session)?.map_err(failed)?;
        if let Some(badge) = network { net::bind(badge, pid); }
        Ok(pid)
    }

    // After a start: a program with a screen takes the focus (its output since start is kept); a console program runs
    // in the shell, which shows its output and waits for it.
    // A program that has already exited is named as the loader names tasks; a console program's output is kept.
    fn started(&mut self, pid: u64, name: &[u8], background: bool) {
        let (list, count) = tasks();
        let task = list[..count].iter().find(|t| t.pid == pid);
        let file = name.rsplit(|&b| b == b'/').next().unwrap_or(name);
        let stem = if file.len() > 4 && file[file.len() - 4..].eq_ignore_ascii_case(b".elf") { &file[..file.len() - 4] } else { file };
        let _ = write!(self.term, "STARTED PID={} NAME=", pid);
        match task { Some(t) => { let _ = write!(self.term, "{}", label(&t.name)); } None => for &b in stem { self.term.print_char(b.to_ascii_lowercase()); } }
        let _ = writeln!(self.term, " {}", if background { "BACKGROUND" } else { "FOREGROUND" });
        if background { return; }
        match task {
            Some(t) if t.screen != 0 => { let _ = self.focus(pid, true); }
            Some(t) if t.service != 0 => {}
            _ => self.console = Some(pid),
        }
    }

    fn run_program(&mut self, name: &[u8], args: &[u8], background: bool) {
        let service = BOOT_SERVICES.iter().any(|s| s.as_bytes().eq_ignore_ascii_case(name));
        match self.start(name, args, service) {
            Ok(pid) => self.started(pid, name, background),
            Err(error) => self.report(error_text(error, service)),
        }
    }

    // Output of the console program running in the shell; when it has exited, its last output and the prompt.
    fn pump_console(&mut self, pid: u64) {
        let mut buffer = [0u8; 1024];
        while let Ok(len @ 1..) = control::console(pid, &mut buffer) { for &byte in &buffer[..len] { self.term.print_char(byte); } }
        if mind::process::alive(pid) { return; }
        while let Ok(len @ 1..) = control::console(pid, &mut buffer) { for &byte in &buffer[..len] { self.term.print_char(byte); } }
        self.console = None;
        if self.term.position().col != 0 { self.term.print_char(b'\n'); }
        self.prompt();
    }

    fn command(&mut self, line: &[u8]) {
        let line = line.trim_ascii();
        let split = line.iter().position(|b| b.is_ascii_whitespace()).unwrap_or(line.len());
        let (cmd, args) = (&line[..split], line[split..].trim_ascii());
        let is = |name: &[u8]| cmd.eq_ignore_ascii_case(name);
        if cmd.is_empty() { return; }
        if is(b"run") || is(b"boot") {
            let (words, background) = if is(b"boot") {
                if !args.is_empty() { return self.report("BOOT TAKES NO ARGUMENTS"); }
                (&b"app"[..], false)
            } else if let Some(rest) = args.strip_suffix(b"&") { (rest.trim_ascii(), true) } else { (args, false) };
            // run <name> [arguments] [&]
            let split = words.iter().position(|b| b.is_ascii_whitespace()).unwrap_or(words.len());
            let (name, program_args) = (&words[..split], words[split..].trim_ascii());
            if name.is_empty() { let _ = writeln!(self.term, "USAGE: RUN <NAME> [ARGUMENTS] [&]"); return self.list_programs(); }
            if name.len() > NAME_MAX { return self.report("PROGRAM NAME TOO LONG"); }
            if program_args.len() > ARGS_MAX { return self.report("ARGUMENTS TOO LONG"); }
            self.run_program(name, program_args, background);
        } else if is(b"ls") {
            files::ls(&mut self.term, args);
        } else if is(b"cat") || is(b"mkdir") || is(b"rm") || is(b"mv") || is(b"write") {
            if args.is_empty() { return self.report("EXPECTED A PATH"); }
            if is(b"cat") { files::cat(&mut self.term, args) } else if is(b"write") { files::write(&mut self.term, args) }
            else { files::change(&mut self.term, if is(b"mkdir") { "MKDIR" } else if is(b"rm") { "RM" } else { "MV" }, args) }
        } else if is(b"sync") {
            files::sync(&mut self.term);
        } else if is(b"logger") {
            // A line in the system log; logd records the shell as its source whatever the text says.
            let Ok(text) = core::str::from_utf8(args) else { return self.report("NOT UTF-8") };
            if text.is_empty() { return self.report("EXPECTED A TEXT"); }
            match mind::log::write(mind::log::INFO, text) { Ok(()) => { let _ = writeln!(self.term, "LOGGED"); } Err(_) => self.report("NO SYSTEM LOG") }
        } else if is(b"caps") && args.is_empty() {
            // Without a PID: the caps tool (capabilities and their derivation tree).
            self.run_program(b"caps", b"", false);
        } else if is(b"pmap") || is(b"caps") || (is(b"stat") && pid_arg(args).is_some()) {
            // `stat <id>`: task details; `stat <class> [pid]`: the kernel's records (below).
            let Some(pid) = pid_arg(args) else { return self.report("EXPECTED ONE POSITIVE PID") };
            if is(b"pmap") { observe::pmap(&mut self.term, pid) } else if is(b"caps") { observe::caps(&mut self.term, pid) } else { observe::task_details(&mut self.term, pid) }
        } else if is(b"fg") || is(b"kill") || is(b"logs") {
            let Some(pid) = pid_arg(args) else { return self.report("EXPECTED ONE POSITIVE PID") };
            let missing = |error: Error| if error == Error::NotFound { "NO SUCH PID" } else { "SERVICE HAS NO SCREEN" };
            if is(b"fg") {
                if pid == self.own { return self.report("THIS IS THE SHELL"); }
                match self.focus(pid, false) {
                    Ok(()) => { let _ = writeln!(self.term, "FOREGROUND PID={} (CTRL+Z: SHELL, ESC: EXIT)", pid); }
                    Err(error) => self.report(missing(error)),
                }
            } else if is(b"kill") {
                match control::kill(pid) { Ok(()) => { let _ = writeln!(self.term, "KILLED PID={}", pid); } Err(error) => self.report(missing(error)) }
            } else {
                let mut buffer = [0u8; 4096];
                match control::logs(pid, &mut buffer) {
                    Ok(len) => {
                        let _ = writeln!(self.term, "LOGS PID={} (LAST 4096 BYTES, DRAINED):", pid);
                        for &byte in &buffer[..len] { self.term.print_char(byte); }
                        let _ = writeln!(self.term, "\nEND LOGS");
                    }
                    Err(error) => self.report(missing(error)),
                }
            }
        } else if !args.is_empty() && [&b"help"[..], b"list", b"cpus", b"faults", b"ps", b"quotas", b"clear", b"stop", b"heap", b"clock", b"date", b"free", b"physmap", b"irqs", b"devices", b"endpoints"].iter().any(|c| is(c)) {
            self.report("THIS COMMAND TAKES NO ARGUMENTS");
        } else if is(b"help") {
            let _ = write!(self.term, "- list: programs\n- run <name> [args] [&]: new instance\n- <name> [args]: run a program in the foreground (say hello, listen 3)\n- boot: run app\n- cpus: online processors, busy and idle time\n- free: kernel memory by use\n- physmap: physical memory map\n- pmap <id>: address space of a task\n- stat <id>: task details\n- stat <tasks|cpus|memory|physmap|vmap PID|caps PID|endpoints|irqs|devices>: kernel statistics\n- caps <id>: capabilities of a task; caps: the caps tool (derivation tree, what a revoke removes)\n- endpoints, irqs, devices: kernel objects\n- clock: monotonic clock and its resolution\n- date: calendar date and time from the RTC\n- ls [path], cat <file>: files (ram: is the RAM disk)\n- write <file> <text>, mkdir, rm, mv <from> <to>, sync: change files on ram: and in data/\n- faults: recent process faults\n- ps: tasks\n- quotas: task and endpoint quotas (used/limit)\n- budget <pid> <ms> <period ms>: CPU budget (0: no limit)\n- fg <id>: foreground\n- kill <id>: terminate\n- logs <id>: buffered output\n- logger <text>: a line in the system log (dmesg shows it)\n- net [arp <ip>]: network card (MAC, link, counters); ARP query while the stack is stopped\n- ip: address, gateway and DNS server\n- netgrants, netrevoke <program>: flow grants of the network policy broker\n- ping <host>, nslookup <name> [server[:port]], fetch <host>[:port] [path]: network\n- heap\n- clear\n- keymap [us|ru] [--switch both|ctrl-shift|alt-shift|caps|none]: keyboard layout and layout switch\n- screenshot [file]: the screen as a BMP (ram:screen-NNN.bmp)\n- reboot [-f]: write cached files to the disks, stop the services (not with -f) and restart the machine\n- stop\nCTRL+Z: SHELL, KEEP RUNNING. ESC: EXIT FOREGROUND APP.\nKEYS: ←/→ HOME/END DEL EDIT THE LINE, ↑/↓ HISTORY, TAB COMPLETES, ESC CLEARS, SHIFT+PGUP/PGDN SCROLL, CTRL+L CLEARS THE SCREEN, CTRL+SHIFT OR ALT+SHIFT: EN/RU.\n");
        } else if is(b"list") {
            self.list_programs();
        } else if is(b"cpus") {
            observe::cpus(&mut self.term);
        } else if is(b"free") {
            observe::free(&mut self.term);
        } else if is(b"physmap") {
            observe::physmap(&mut self.term);
        } else if is(b"irqs") {
            observe::irqs(&mut self.term);
        } else if is(b"devices") {
            observe::devices(&mut self.term);
        } else if is(b"endpoints") {
            observe::endpoints(&mut self.term);
        } else if is(b"faults") {
            let mut faults = [FaultInfo::default(); 16];
            let count = control::faults(&mut faults).unwrap_or(0);
            for f in &faults[..count] { let _ = writeln!(self.term, "FAULT PID={} CPU={} VECTOR={} ERROR={:#x} RIP={:#x} ADDR={:#x}", f.pid, f.cpu, f.vector, f.error, f.rip, f.address); }
        } else if is(b"quotas") {
            // Quotas delegated at spawn (MC-1.7): tasks reserved by live children, endpoints created or delegated.
            let _ = writeln!(self.term, "PID NAME TASKS ENDPOINTS");
            let (list, count) = tasks();
            for t in &list[..count] { let _ = writeln!(self.term, "{} {} {}/{} {}/{}", t.pid, label(&t.name), t.used_tasks, t.quota_tasks, t.used_endpoints, t.quota_endpoints); }
        } else if is(b"ps") {
            let _ = writeln!(self.term, "PID NAME STATE FOCUS CPU RUNS CPU_TICKS SYSCALLS");
            let (list, count) = tasks();
            for t in &list[..count] {
                let _ = writeln!(self.term, "{} {} {} {} {} {} {} {}", t.pid, label(&t.name), label(&t.state), if t.focus != 0 { "FG" } else { "BG" }, t.cpu, t.runs, t.ticks, t.calls);
            }
            let _ = writeln!(self.term, "{} TASK(S); SHELL PID={}; LIMIT={} APPS + SERVICES", count, self.own, MAX_APPS);
        } else if is(b"clear") {
            self.term.clear();
        } else if is(b"reboot") {
            power::reboot(&mut self.term, args);
        } else if is(b"keymap") {
            keymap::command(&mut self.term, args);
        } else if is(b"screenshot") {
            screenshot::command(&mut self.term, args, SCOPE_RECEIVE);
        } else if is(b"stop") {
            files::flush_all(); // what vfs_server still caches goes to the disks first
            let _ = writeln!(self.term, "SYSTEM HALTED. CPU GOING TO SLEEP...");
            control::halt();
        } else if is(b"date") {
            // Calendar time from the rtc service (idl/rtc.wit 1.1), without a time zone.
            match (mind::rtc::date(), mind::rtc::seconds_since_midnight()) {
                (Some((y, m, d)), Some(s)) => { let _ = writeln!(self.term, "DATE: {:04}-{:02}-{:02} {:02}:{:02}:{:02} (RTC, NO TIME ZONE)", y, m, d, s / 3600, s / 60 % 60, s % 60); }
                _ => self.report("RTC NOT AVAILABLE"),
            }
        } else if is(b"net") {
            net::command(&mut self.term, args);
        } else if is(b"netgrants") {
            net::grants(&mut self.term);
        } else if is(b"netrevoke") {
            net::revoke(&mut self.term, args);
        } else if is(b"ip") {
            net::ip(&mut self.term);
        } else if is(b"ping") {
            net::ping(&mut self.term, args);
        } else if is(b"nslookup") {
            net::nslookup(&mut self.term, args);
        } else if is(b"fetch") {
            net::fetch(&mut self.term, args);
        } else if is(b"clock") {
            let (ns, resolution, hz) = mind::time::clock_info();
            let _ = writeln!(self.term, "CLOCK: MONOTONIC NS={} RESOLUTION NS={} TSC HZ={} UPTIME MS={}", ns, resolution, hz, mind::time::uptime_ms());
        } else if is(b"budget") {
            // budget <pid> <ms> <period ms>: CPU budget per period (0: no limit) — scheduling contexts (C7).
            let text = core::str::from_utf8(args).unwrap_or("");
            let numbers: [Option<u64>; 3] = { let mut w = text.split_whitespace().map(|w| w.parse::<u64>().ok()); [w.next().flatten(), w.next().flatten(), w.next().flatten()] };
            match numbers {
                [Some(pid), Some(budget), Some(period)] => match control::sched_set(pid, budget * 1000, period * 1000, BAND_KEEP) {
                    Ok(()) => { let _ = writeln!(self.term, "BUDGET PID={} {} MS PER {} MS", pid, budget, period); }
                    Err(Error::NotFound) => self.report("NO SUCH PID"),
                    Err(_) => self.report("INVALID BUDGET (PERIOD >= 10 MS, BUDGET <= PERIOD)"),
                },
                _ => self.report("BUDGET <PID> <MS> <PERIOD MS>"),
            }
        } else if is(b"stat") {
            self.stat(args);
        } else if is(b"heap") {
            let (used, free, freed) = control::kernel_heap();
            let _ = writeln!(self.term, "Dynamic allocation works! Uptime: {} ms", mind::time::uptime_ms());
            let _ = writeln!(self.term, "HEAP: USED={} FREE={} TEST FREED={}", used, free, freed);
        } else if cmd.len() <= NAME_MAX && cmd.is_ascii() && !BOOT_SERVICES.iter().any(|s| s.as_bytes().eq_ignore_ascii_case(cmd)) {
            // Any other word runs the program of that name in the foreground: `say hello`, `listen 3`.
            match self.start(cmd, args, false) {
                Ok(pid) => self.started(pid, cmd, false),
                Err(Error::NotFound) => self.report("UNKNOWN COMMAND"),
                Err(error) => self.report(error_text(error, false)),
            }
        } else {
            self.report("UNKNOWN COMMAND");
        }
    }

    fn cursor(&self) -> Position { self.term.offset(self.prompt_at, self.line.cursor_chars()) }

    // Redraws the input line after the prompt; `uart` also redraws it on the terminal (VT100: CR, prompt, line,
    // erase to the end, cursor back).
    fn redraw_input(&mut self, uart: bool) {
        let mut copy = [0u8; 256];
        let text = self.line.as_str(); let len = text.len(); copy[..len].copy_from_slice(text.as_bytes());
        let text = core::str::from_utf8(&copy[..len]).unwrap_or("");
        self.term.truncate(self.prompt_at);
        self.term.put_str(text);
        if uart {
            self.term.serial_str("\r\x1b[KMIND> ");
            self.term.serial_str(text);
            let back = text.chars().count() - self.line.cursor_chars();
            if back > 0 { let mut seq = mind::util::FixedBuf::<16>::new(); let _ = write!(seq, "\x1b[{}D", back); self.term.serial_str(core::str::from_utf8(seq.as_bytes()).unwrap_or("")); }
        }
    }

    // Program names from the loader (the root of the boot disk) and the boot services, for completion.
    fn refresh_names(&mut self) {
        self.name_count = 0;
        let add = |shell: &mut Self, name: &[u8]| {
            if shell.name_count < NAMES && !name.is_empty() && name.len() <= NAME_MAX && !shell.names[..shell.name_count].iter().zip(&shell.name_lens).any(|(n, &l)| &n[..l] == name) {
                shell.names[shell.name_count][..name.len()].copy_from_slice(name); shell.name_lens[shell.name_count] = name.len(); shell.name_count += 1;
            }
        };
        if let Ok(programs) = loader::list(Endpoint::LOADER) {
            for program in programs.as_slice() { add(self, program.name.as_str().as_bytes()); }
        }
        for name in BOOT_SERVICES { add(self, name.as_bytes()); }
    }

    // Tab: completes the word before the cursor with a command or program name; several matches are listed.
    fn complete(&mut self) {
        let mut copy = [0u8; 256];
        let text = self.line.as_str(); let len = text.len(); copy[..len].copy_from_slice(text.as_bytes());
        if self.line.cursor_chars() != text.chars().count() { return; }
        let start = copy[..len].iter().rposition(|&b| b == b' ').map_or(0, |i| i + 1);
        let word = &copy[start..len];
        let first = copy[..start].trim_ascii().is_empty();
        let first_token = copy[..len].split(|&b| b == b' ').next().unwrap_or(&[]);
        if !first && !first_token.eq_ignore_ascii_case(b"run") { return; }
        self.refresh_names();
        let mut matches = [[0u8; NAME_MAX]; NAMES]; let mut lens = [0usize; NAMES]; let mut count = 0;
        let names = (0..self.name_count).map(|i| &self.names[i][..self.name_lens[i]]);
        let commands = COMMANDS.iter().map(|c| c.as_bytes()).filter(|_| first);
        for name in commands.chain(names) {
            if name.len() >= word.len() && name[..word.len()].eq_ignore_ascii_case(word) && count < NAMES && !matches[..count].iter().zip(&lens).any(|(m, &l)| &m[..l] == name) {
                matches[count][..name.len()].copy_from_slice(name); lens[count] = name.len(); count += 1;
            }
        }
        if count == 0 { return; }
        // The longest common prefix of all matches.
        let mut common = lens[0];
        for i in 1..count { common = common.min(lens[i]); while common > 0 && !matches[i][..common].eq_ignore_ascii_case(&matches[0][..common]) { common -= 1; } }
        if common > word.len() || count == 1 {
            for &b in &matches[0][word.len()..common] { self.line.insert(b as char); }
            if count == 1 { self.line.insert(' '); }
            self.redraw_input(true);
            return;
        }
        self.term.print_char(b'\n');
        for i in 0..count { for &b in &matches[i][..lens[i]] { self.term.print_char(b); } self.term.print_char(b' '); self.term.print_char(b' '); }
        self.term.print_char(b'\n');
        let _ = write!(self.term, "MIND> ");
        self.prompt_at = self.term.position();
        self.redraw_input(true);
    }

    // A key typed while the shell has the focus.
    fn key(&mut self, key: Key) {
        // While a console program runs, Esc or Ctrl+C stops it; other keys are not for the shell.
        if let Some(pid) = self.console {
            if key.is_escape() || key.is_ctrl('c') { if control::kill(pid).is_ok() { let _ = write!(self.term, "^C"); } }
            return;
        }
        if key.shift() && matches!(key.code(), Code::PageUp | Code::PageDown) { self.term.scroll(key.code() == Code::PageUp); return; }
        self.term.unscroll();
        if key.is_ctrl('l') { self.term.clear(); let _ = write!(self.term, "MIND> "); self.prompt_at = self.term.position(); self.redraw_input(true); return; }
        if self.history.key(key, &mut self.line) { self.redraw_input(true); return; }
        if key.code() == Code::Tab { self.complete(); return; }
        let (old_len, old_at_end) = (self.line.as_str().len(), self.line.cursor_chars() == self.line.as_str().chars().count());
        match self.line.key(key) {
            Edit::Submit => {
                let mut copy = [0u8; 256];
                let text = self.line.as_str(); let len = text.len(); copy[..len].copy_from_slice(text.as_bytes());
                self.history.push(core::str::from_utf8(&copy[..len]).unwrap_or(""));
                self.line.clear();
                self.term.print_char(b'\n');
                self.command(&copy[..len]);
                if self.focused.is_none() && self.console.is_none() { self.prompt(); }
            }
            Edit::Cancel => { if !self.line.is_empty() { self.line.clear(); self.history.reset(); self.redraw_input(true); } }
            Edit::Changed => {
                self.history.reset();
                let text = self.line.as_str();
                let at_end = self.line.cursor_chars() == text.chars().count();
                if old_at_end && at_end && text.len() > old_len {
                    // Typing at the end: echo only the new character (the common case on a slow UART).
                    let mut copy = [0u8; 8]; let added = &text.as_bytes()[old_len..]; copy[..added.len()].copy_from_slice(added);
                    for &byte in &copy[..added.len()] { self.term.print_char(byte); }
                } else {
                    self.redraw_input(true);
                }
            }
            Edit::Moved => self.redraw_input(true),
            _ => {}
        }
    }
    // A decoded UART event: the shell's own input, or forwarded to the focused program (Ctrl+Z is the attention key).
    fn uart(&mut self, event: Event) {
        match event {
            Event::Key(word) if self.focused.is_some() => { let _ = input_key(word, word, false); }
            Event::Key(word) => self.key(Key(word)),
            Event::Attention if self.focused.is_some() => { let _ = input_key(0, 0, true); }
            _ => {}
        }
    }
}

// UART events decoded in one pass of the main loop (the 16550 FIFO holds 16 bytes; a sequence yields at most two).
struct Events { list: [Option<Event>; 64], len: usize }
impl Events {
    fn new() -> Self { Self { list: [None; 64], len: 0 } }
    fn push(&mut self, event: Event) { if self.len < self.list.len() { self.list[self.len] = Some(event); self.len += 1; } }
    fn as_slice(&self) -> impl Iterator<Item = &Event> { self.list[..self.len].iter().flatten() }
    fn clear(&mut self) { self.len = 0; }
}

mind::entry!(main);
fn main(info: &'static BootInfo) {
    let term = Console::new(mind::gfx::Screen::new(info), Ports(SLOT_SERIAL));
    let own = control::focus(0, false).unwrap_or(0);
    let mut shell = Shell { term, line: InputLine::new(), history: History::new(), prompt_at: Position { line: 0, col: 0 }, own, focused: None, line_start: true,
                            console: None,
                            names: [[0; NAME_MAX]; NAMES], name_lens: [0; NAMES], name_count: 0 };
    let (used, free, _) = control::kernel_heap();
    let _ = writeln!(shell.term, "MIND CORE v1.6 [Build: 2026-10-03]. SMP / RING 3 SERVICES / RING 3 SHELL.");
    let _ = writeln!(shell.term, "MEMORY MANAGER: {} MB HEAP.", (used + free) / 1024 / 1024);
    let _ = writeln!(shell.term, "LIST: PROGRAMS. RUN <NAME> [&]. PS. FG <ID>. HELP. TAB COMPLETES, ↑/↓ HISTORY, CTRL+SHIFT: EN/RU.");
    shell.prompt();
    let serial = Ports(SLOT_SERIAL);
    let mut vt = Vt::new();
    let mut events = Events::new();
    loop {
        if let Some(pid) = shell.focused { shell.mirror(pid); }
        if let Some(pid) = shell.console { shell.pump_console(pid); }
        while let Some(notice) = control::notice() {
            let (pid, what) = match notice { Notice::Exited(pid) => { shell.mirror(pid); (pid, "EXITED") } Notice::Background(pid) => (pid, "BACKGROUND") };
            shell.focused = None;
            let _ = writeln!(shell.term, "\nPID={} {}. SHELL RESUMED.", pid, what);
            shell.prompt();
        }
        // UART: terminal input decoded into key events (VT100/xterm sequences, UTF-8, a lone Esc after a timeout).
        let now = mind::time::uptime_ms() as u64;
        while serial.in8(COM1 + 5) & 1 != 0 {
            let byte = serial.in8(COM1);
            vt.feed(byte, now, &mut |event| events.push(event));
        }
        vt.poll(now, &mut |event| events.push(event));
        for &event in events.as_slice() { shell.uart(event); }
        events.clear();
        // PS/2 keys arrive in the shell's queue while it has the focus.
        while let Some(key) = mind::input::read_key() { if shell.focused.is_none() { shell.key(key); } }
        let cursor = (shell.focused.is_none() && shell.console.is_none()).then(|| shell.cursor());
        shell.term.render(cursor);
        mind::time::sleep(10);
    }
}

// Upper-case display of an ASCII word.
struct Upper<'a>(&'a str);
impl core::fmt::Display for Upper<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result { for c in self.0.chars() { write!(f, "{}", c.to_ascii_uppercase())?; } Ok(()) }
}
