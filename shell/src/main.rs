#![no_std]
#![no_main]
// Command shell in ring 3: text console on its own screen and COM1, commands over process control, loader and init.
// It owns the focus: programs it brings to the foreground get the keyboard, and focus returns to it on exit or Ctrl+Z.
mod console;

use console::{Console, Position, COM1};
use core::fmt::Write;
use mind::abi::*;
use mind::control::{self, Notice};
use mind::dev::{input_event, Ports};
use mind::input::{Code, Key};
use mind::tui::widgets::{Edit, History, InputLine};
use mind::ipc::{Endpoint, Message};
use mind::keys::{Event, Vt};
use mind::mem::Pages;
use mind::sys::Error;

// Words the shell completes with Tab besides program names.
const COMMANDS: [&str; 14] = ["boot", "clear", "clock", "cpus", "faults", "fg", "heap", "help", "kill", "list", "logs", "ps", "run", "stop"];
const NAMES: usize = 64;

struct Shell {
    term: Console, line: InputLine, history: History<32>, prompt_at: Position, own: u64, focused: Option<u64>, line_start: bool,
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

fn tasks() -> ([TaskInfo; 20], usize) {
    let mut list = [unsafe { core::mem::zeroed::<TaskInfo>() }; 20];
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

    fn focus(&mut self, pid: u64, keep_output: bool) -> Result<(), Error> {
        control::focus(pid, keep_output)?;
        self.focused = Some(pid); self.line_start = true;
        Ok(())
    }

    fn list_programs(&mut self) {
        let listing = (|| -> Result<(Pages, usize), Error> {
            let page = Pages::new(4096).ok_or(Error::NoMemory)?;
            let cap = page.share()?;
            let reply = Endpoint::LOADER.call(&Message::new(0, LOADER_LIST).with_cap(cap, 0), 0);
            let _ = mind::ipc::drop_cap(cap);
            let len = mind::sys::check(reply?.data[0])?;
            Ok((page, len.min(4096)))
        })();
        match listing {
            Ok((page, len)) => { let _ = writeln!(self.term, "PROGRAMS ON DISK:"); for &byte in &page.as_slice()[..len] { self.term.print_char(byte); } }
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
            let words = mind::process::pack_name(name).ok_or(Error::Invalid)?;
            return Endpoint::INIT.call(&Message::new(words[0], words[1]), 0).and_then(|reply| mind::sys::check(reply.data[0])).map(|pid| pid as u64);
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
            let _ = write!(self.term, "- list: programs\n- run <name> [args] [&]: new instance\n- <name> [args]: run a program in the foreground (say hello, listen 3)\n- boot: run app\n- cpus: online processors\n- clock: monotonic clock and its resolution\n- faults: recent process faults\n- ps: tasks\n- fg <id>: foreground\n- kill <id>: terminate\n- logs <id>: buffered output\n- heap\n- clear\n- stop\nCTRL+Z: SHELL, KEEP RUNNING. ESC: EXIT FOREGROUND APP.\nKEYS: ←/→ HOME/END DEL EDIT THE LINE, ↑/↓ HISTORY, TAB COMPLETES, ESC CLEARS, SHIFT+PGUP/PGDN SCROLL, CTRL+L CLEARS THE SCREEN, CTRL+SHIFT OR ALT+SHIFT: EN/RU.\n");
        } else if is(b"list") {
            self.list_programs();
        } else if is(b"cpus") {
            let mut index = 0;
            while let Some((apic, online, ticks)) = control::cpu(index) { let _ = writeln!(self.term, "CPU={} APIC={} ONLINE={} TICKS={}", index, apic, online, ticks); index += 1; }
        } else if is(b"faults") {
            let mut faults = [FaultInfo::default(); 16];
            let count = control::faults(&mut faults).unwrap_or(0);
            for f in &faults[..count] { let _ = writeln!(self.term, "FAULT PID={} CPU={} VECTOR={} ERROR={:#x} RIP={:#x} ADDR={:#x}", f.pid, f.cpu, f.vector, f.error, f.rip, f.address); }
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
        } else if is(b"heap") {
            let (used, free, freed) = control::kernel_heap();
            let _ = writeln!(self.term, "Dynamic allocation works! Uptime: {} ms", mind::time::uptime_ms());
            let _ = writeln!(self.term, "HEAP: USED={} FREE={} TEST FREED={}", used, free, freed);
        } else if cmd.len() <= NAME_MAX && cmd.is_ascii() && !BOOT_SERVICES.iter().any(|s| s.as_bytes().eq_ignore_ascii_case(cmd)) {
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
        if let Some(page) = Pages::new(4096) {
            if let Ok(cap) = page.share() {
                let reply = Endpoint::LOADER.call(&Message::new(0, LOADER_LIST).with_cap(cap, 0), 0);
                let _ = mind::ipc::drop_cap(cap);
                if let Ok(len) = reply.and_then(|r| mind::sys::check(r.data[0])) {
                    let mut names = [[0u8; NAME_MAX]; NAMES]; let mut lens = [0usize; NAMES]; let mut count = 0;
                    for line in page.as_slice()[..len.min(4096)].split(|&b| b == b'\n') {
                        let word = line.trim_ascii().split(|b| b.is_ascii_whitespace()).next().unwrap_or(&[]);
                        if !word.is_empty() && word.len() <= NAME_MAX && count < NAMES { names[count][..word.len()].copy_from_slice(word); lens[count] = word.len(); count += 1; }
                    }
                    for i in 0..count { add(self, &names[i][..lens[i]]); }
                }
            }
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
                if self.focused.is_none() { self.prompt(); }
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
            Event::Key(word) if self.focused.is_some() => { let _ = input_event(word, false); }
            Event::Key(word) => self.key(Key(word)),
            Event::Attention if self.focused.is_some() => { let _ = input_event(0, true); }
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
        let cursor = shell.focused.is_none().then(|| shell.cursor());
        shell.term.render(cursor);
        mind::time::sleep(10);
    }
}
