#![no_std]
#![no_main]
// Command shell in ring 3: text console on its own screen and COM1, commands over process control, loader and init.
// It owns the focus: programs it brings to the foreground get the keyboard, and focus returns to it on exit or Ctrl+Z.
use core::fmt::Write;
use mind::abi::*;
use mind::control::{self, Notice};
use mind::dev::{input_event, Ports};
use mind::font::FONT;
use mind::ipc::Endpoint;
use mind::mem::Pages;
use mind::sys::Error;

const COM1: u16 = 0x3F8;
const BACKGROUND: u32 = 0x001E1E2E;
const FOREGROUND: u32 = 0x00A6E3A1;

struct Console { fb: *mut u32, width: usize, height: usize, stride: usize, cx: usize, cy: usize, serial: Ports }

impl Console {
    fn serial(&self, byte: u8) {
        while self.serial.in8(COM1 + 5) & 0x20 == 0 {}
        self.serial.out8(COM1, byte);
    }
    fn pixel(&self, x: usize, y: usize, color: u32) { unsafe { core::ptr::write_volatile(self.fb.add(y * self.stride + x), color); } }
    fn scroll(&mut self) {
        let limit = self.height - 10;
        for y in 10..limit { for x in 0..self.width { unsafe { core::ptr::write_volatile(self.fb.add((y - 10) * self.stride + x), core::ptr::read_volatile(self.fb.add(y * self.stride + x))); } } }
        for y in (limit - 10)..limit { for x in 0..self.width { self.pixel(x, y, BACKGROUND); } }
        self.cy -= 10;
    }
    fn print_char(&mut self, ch: u8) {
        if ch == b'\r' { return; }
        if ch == b'\n' { self.serial(b'\r'); }
        self.serial(ch);
        // UTF-8 goes to COM1 as is; the 8x8 font has only ASCII, so a multi-byte character is drawn as one '?'.
        if (0x80..0xC0).contains(&ch) { return; }
        let ch = if ch >= 0xC0 { b'?' } else { ch };
        if ch == b'\n' {
            self.cx = 0; self.cy += 10;
        } else if ch == 0x08 {
            self.cx = self.cx.saturating_sub(8);
            for row in 0..8 { for col in 0..8 { self.pixel(self.cx + col, self.cy + row, BACKGROUND); } }
        } else {
            let index = match ch { 32..=95 => (ch - 32) as usize, 97..=122 => (ch - 97 + 33) as usize, _ => 0 };
            let bitmap = FONT[index];
            for row in 0..8 {
                let bits = (bitmap >> ((7 - row) * 8)) & 0xFF;
                for col in 0..8 { self.pixel(self.cx + col, self.cy + row, if bits & (1 << (7 - col)) != 0 { FOREGROUND } else { BACKGROUND }); }
            }
            self.cx += 8;
        }
        if self.cx >= self.width { self.cx = 0; self.cy += 10; }
        if self.cy >= self.height - 10 { self.scroll(); }
    }
    fn clear(&mut self) {
        for y in 0..self.height { for x in 0..self.width { self.pixel(x, y, BACKGROUND); } }
        self.cx = 0; self.cy = 0;
    }
}

impl Write for Console {
    fn write_str(&mut self, text: &str) -> core::fmt::Result { for byte in text.bytes() { self.print_char(byte); } Ok(()) }
}

struct Shell { term: Console, line: [u8; 256], len: usize, own: u64, focused: Option<u64>, line_start: bool }

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

fn tasks() -> ([TaskInfo; 20], usize) {
    let mut list = [unsafe { core::mem::zeroed::<TaskInfo>() }; 20];
    let count = control::tasks(&mut list).unwrap_or(0);
    (list, count)
}

impl Shell {
    fn report(&mut self, error: &str) { let _ = writeln!(self.term, "ERROR: {}", error); }
    fn prompt(&mut self) { let _ = write!(self.term, "MIND> "); }

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
        let header = match control::stat(class, pid, page.as_mut_slice()) { Ok(h) => h, Err(error) => { self.report(if error == Error::NotFound { "NO SUCH PID" } else { "STAT FAILED" }); return; } };
        let buffer = page.as_slice(); let t = &mut self.term;
        let _ = writeln!(t, "STAT {} VERSION={} COUNT={} TOTAL={}", Upper(name), header.version, header.count, header.total);
        match class {
            STAT_TASKS => for r in control::records::<StatTask>(buffer, header) { let _ = writeln!(t, "{} PARENT={} {} WAIT={}:{} CPU={} RUN_MS={} SENDS={} RECEIVES={} HEAP={} SHARED={} CAPS={}", r.pid, r.parent, label(&r.name), r.wait, r.wait_on, r.cpu, r.run_ns / 1_000_000, r.sends, r.receives, r.heap_bytes, r.shared_bytes, r.caps); },
            STAT_CPUS => for (i, r) in control::records::<StatCpu>(buffer, header).enumerate() { let _ = writeln!(t, "CPU {} APIC={} ONLINE={} BUSY_MS={} IDLE_MS={} INTERRUPTS={} SWITCHES={} PID={}", i, r.apic_id, r.online, r.busy_ns / 1_000_000, r.idle_ns / 1_000_000, r.interrupts, r.switches, r.current_pid); },
            STAT_MEMORY => for r in control::records::<StatMemory>(buffer, header) { let _ = writeln!(t, "ARENA={} USED={} FREE={} IMAGES={} STACKS={} TASK_PAGES={} SCREENS={} HEAPS={} OBJECTS={} DMA={} TASKS={} ENDPOINTS={}", r.arena, r.used, r.free, r.images, r.stacks, r.task_pages, r.screens, r.heaps, r.objects, r.dma, r.tasks, r.endpoints); },
            STAT_PHYSMAP => for r in control::records::<StatPhys>(buffer, header).filter(|r| r.kind >= PHYS_PLATFORM) { let _ = writeln!(t, "KIND={:#x} INDEX={} START={:#x} PAGES={}", r.kind, r.index, r.start, r.pages); },
            STAT_VMAP => for r in control::records::<StatRegion>(buffer, header) {
                let kind = ["?", "IMAGE", "STACK", "SCREEN", "INFO", "MAILBOX", "EXIT", "HEAP", "SHARED", "DEVICE"].get(r.kind as usize).copied().unwrap_or("?");
                let _ = writeln!(t, "{:#x} {} {} {}{}{}", r.start, r.size, kind, if r.flags & REGION_READ != 0 { 'R' } else { '-' }, if r.flags & REGION_WRITE != 0 { 'W' } else { '-' }, if r.flags & REGION_EXECUTE != 0 { 'X' } else { '-' });
            },
            STAT_CAPS => for r in control::records::<StatCap>(buffer, header) { let _ = writeln!(t, "SLOT={} GEN={} KIND={} RIGHTS={} SIZE={} BADGE={} NODE={} PARENT={}", r.slot, r.generation, r.kind, r.rights, r.size, r.badge, r.node, r.parent); },
            STAT_ENDPOINTS => for r in control::records::<StatEndpoint>(buffer, header) { let _ = writeln!(t, "EP {} RECEIVERS={} SENDERS={} WAITING={} CREATOR={} MESSAGES={} BUSY={} TIMEOUTS={}", r.index, r.receivers, r.waiting_senders, r.waiting_receivers, r.creator, r.messages, r.busy, r.timeouts); },
            STAT_IRQS => for r in control::records::<StatIrq>(buffer, header) { let _ = writeln!(t, "IRQ {} ENDPOINT={} MASKED={} HOLDER={} COUNT={}", r.line, r.endpoint, r.masked, r.holder, r.count); },
            _ => for r in control::records::<StatDevice>(buffer, header) { let _ = writeln!(t, "DEVICE CLASS={:06x} IRQ={} HOLDER={} BARS={:?}", r.class, r.irq, r.holder, r.bar_sizes); },
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

    // Boot services are (re)started by init, applications by the loader from disk (with arguments, if any).
    fn start(name: &[u8], args: &[u8], service: bool) -> Result<u64, Error> {
        if service {
            if !args.is_empty() { return Err(Error::Invalid); }
            let name = core::str::from_utf8(name).map_err(|_| Error::Invalid)?;
            return mind::idl::init::run(Endpoint::INIT, name);
        }
        let name = core::str::from_utf8(name).map_err(|_| Error::Invalid)?;
        if args.is_empty() { return mind::process::spawn(name, None); }
        mind::process::spawn_with_args(name, core::str::from_utf8(args).map_err(|_| Error::Invalid)?)
    }

    fn run_program(&mut self, name: &[u8], args: &[u8], background: bool) {
        let service = BOOT_SERVICES.iter().any(|s| s.as_bytes().eq_ignore_ascii_case(name));
        let pid = match Self::start(name, args, service) {
            Ok(pid) => pid,
            Err(error) => return self.report(error_text(error, service)),
        };
        let (list, count) = tasks();
        let task = list[..count].iter().find(|t| t.pid == pid);
        let task_name = task.map_or("?", |t| label(&t.name));
        let _ = writeln!(self.term, "STARTED PID={} NAME={} {}", pid, task_name, if background { "BACKGROUND" } else { "FOREGROUND" });
        // Only programs with a screen can take the focus; their output since start is kept.
        if !background && task.is_some_and(|t| t.screen != 0) { let _ = self.focus(pid, true); }
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
        } else if !args.is_empty() && [&b"help"[..], b"list", b"cpus", b"faults", b"ps", b"clear", b"stop", b"heap", b"clock"].iter().any(|c| is(c)) {
            self.report("THIS COMMAND TAKES NO ARGUMENTS");
        } else if is(b"help") {
            let _ = write!(self.term, "- list: programs\n- run <name> [args] [&]: new instance\n- <name> [args]: run a program in the foreground (say hello, listen 3)\n- boot: run app\n- cpus: online processors\n- clock: monotonic clock and its resolution\n- faults: recent process faults\n- ps: tasks\n- quotas: task and endpoint quotas (used/limit)\n- budget <pid> <ms> <period ms>: CPU budget (0: no limit)\n- stat <tasks|cpus|memory|physmap|vmap PID|caps PID|endpoints|irqs|devices>: kernel statistics\n- fg <id>: foreground\n- kill <id>: terminate\n- logs <id>: buffered output\n- heap\n- clear\n- stop\nCTRL+Z: SHELL, KEEP RUNNING. ESC: EXIT FOREGROUND APP.\n");
        } else if is(b"list") {
            self.list_programs();
        } else if is(b"cpus") {
            let mut index = 0;
            while let Some((apic, online, ticks)) = control::cpu(index) { let _ = writeln!(self.term, "CPU={} APIC={} ONLINE={} TICKS={}", index, apic, online, ticks); index += 1; }
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
        } else if is(b"stop") {
            let _ = writeln!(self.term, "SYSTEM HALTED. CPU GOING TO SLEEP...");
            control::halt();
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
        } else if cmd.len() <= NAME_MAX && !BOOT_SERVICES.iter().any(|s| s.as_bytes().eq_ignore_ascii_case(cmd)) {
            // Any other word runs the program of that name in the foreground: `say hello`, `listen 3`.
            match Self::start(cmd, args, false) {
                Ok(pid) => {
                    let (list, count) = tasks();
                    let task = list[..count].iter().find(|t| t.pid == pid);
                    let _ = writeln!(self.term, "STARTED PID={} NAME={} FOREGROUND", pid, task.map_or("?", |t| label(&t.name)));
                    if task.is_some_and(|t| t.screen != 0) { let _ = self.focus(pid, true); }
                }
                Err(Error::NotFound) => self.report("UNKNOWN COMMAND"),
                Err(error) => self.report(error_text(error, false)),
            }
        } else {
            self.report("UNKNOWN COMMAND");
        }
    }

    // A key typed while the shell has the focus.
    fn key(&mut self, byte: u8) {
        match byte {
            8 if self.len != 0 => {
                // Remove a whole UTF-8 character: continuation bytes, then the lead byte.
                while self.len > 1 && (0x80..0xC0).contains(&self.line[self.len - 1]) { self.len -= 1; }
                self.len -= 1; self.term.print_char(8);
            }
            b'\n' => {
                self.term.print_char(b'\n');
                let line = self.line; let len = self.len; self.len = 0;
                self.command(&line[..len]);
                if self.focused.is_none() { self.prompt(); }
            }
            32..=126 | 0x80..=0xFF if self.len < self.line.len() => { self.line[self.len] = byte; self.len += 1; self.term.print_char(byte); }
            _ => {}
        }
    }
}

mind::entry!(main);
fn main(info: &'static BootInfo) {
    let term = Console { fb: info.fb_ptr, width: info.width, height: info.height, stride: info.stride, cx: 0, cy: 0, serial: Ports(SLOT_SERIAL) };
    let own = control::focus(0, false).unwrap_or(0);
    let mut shell = Shell { term, line: [0; 256], len: 0, own, focused: None, line_start: true };
    shell.term.clear();
    let (used, free, _) = control::kernel_heap();
    let _ = writeln!(shell.term, "MIND CORE v1.6 [Build: 2026-10-03]. SMP / RING 3 SERVICES / RING 3 SHELL.");
    let _ = writeln!(shell.term, "MEMORY MANAGER: {} MB HEAP.", (used + free) / 1024 / 1024);
    let _ = writeln!(shell.term, "LIST: PROGRAMS. RUN <NAME> [&]. PS. FG <ID>. HELP.");
    shell.prompt();
    let serial = Ports(SLOT_SERIAL);
    loop {
        if let Some(pid) = shell.focused { shell.mirror(pid); }
        while let Some(notice) = control::notice() {
            let (pid, what) = match notice { Notice::Exited(pid) => { shell.mirror(pid); (pid, "EXITED") } Notice::Background(pid) => (pid, "BACKGROUND") };
            shell.focused = None;
            let _ = writeln!(shell.term, "\nPID={} {}. SHELL RESUMED.", pid, what);
            shell.prompt();
        }
        // UART: the shell's own input, or forwarded to the focused program (Ctrl+Z is the attention key).
        while serial.in8(COM1 + 5) & 1 != 0 {
            let byte = serial.in8(COM1);
            let translated = match byte { b'\r' => b'\n', 127 => 8, other => other };
            if shell.focused.is_some() { let _ = input_event(byte, translated, byte == 26); } else { shell.key(translated); }
        }
        // PS/2 keys arrive in the shell's queue while it has the focus.
        while let Some(byte) = mind::input::read_key() { if shell.focused.is_none() { shell.key(byte); } }
        mind::time::sleep(10);
    }
}

// Upper-case display of an ASCII word.
struct Upper<'a>(&'a str);
impl core::fmt::Display for Upper<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result { for c in self.0.chars() { write!(f, "{}", c.to_ascii_uppercase())?; } Ok(()) }
}
