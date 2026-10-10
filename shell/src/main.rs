#![no_std]
#![no_main]
// Command shell in ring 3: text console on its own screen and the serial line, commands over process control, loader and init.
// It owns the focus: programs it brings to the foreground get the keyboard, and focus returns to it on exit or Ctrl+Z.
// Four consoles (issue 155), switched with Ctrl+Alt+F1…F4 whatever program has the focus: each its own text, line,
// history and programs; the serial line belongs to the first. A fifth session has a window of the shell's own in a
// window manager, opened with Ctrl+Alt+F5 or from the manager's menu (211-APP-0040, 211-APP-0045, 211-APP-0044).
extern crate alloc;

mod bmp;
mod clients;
mod console;
mod files;
mod keymap;
mod msh;
mod net;
mod observe;
mod power;
mod programs;
mod ring;
mod screenshot;
mod tpm;
mod voicectl;

use console::{Console, Position};
use core::fmt::Write;
use mind::abi::*;
use mind::control::{self, Notice};
use mind::dev::{input_key, Uart};
use mind::idl::{init as idl_init, loader, shell as commands};
use mind::input::{Code, Key};
use mind::tui::widgets::{Edit, History, InputLine};
use mind::ipc::Endpoint;
use mind::keys::{Event, Vt};
use mind::mem::Pages;
use mind::script::Interpreter;
use mind::sys::Error;

// The shell's commands (`help`); `help <name>` shows the lines that name it.
const HELP: &str = "- help [command or program]: these lines; with a name, what that command or program does (a program also answers <name> --help)\n- list [-l] [mask]: programs on the disk and services; -l: what each program does; a mask keeps the names that match (list a*, list -l *mon*)\n- run <name> [args] [&]: new instance\n- <name> [args]: run a program in the foreground (say hello, listen 3)\n- boot: run app\n- cpus: online processors, busy and idle time\n- free: kernel memory by use\n- physmap: physical memory map\n- pmap <id>: address space of a task\n- stat <id>: task details\n- stat <tasks|cpus|memory|physmap|vmap PID|caps PID|endpoints|irqs|devices>: kernel statistics\n- caps <id>: capabilities of a task; caps: the caps tool (derivation tree, what a revoke removes)\n- endpoints, irqs, devices: kernel objects\n- time: the time of day, the uptime, the monotonic clock and its resolution (clock: the clock program, full screen)\n- date: calendar date and time from the RTC; date set YYYY-MM-DD HH:MM[:SS]: set the clock (it keeps no time zone)\n- ls [path], cat <file>: files (ram: is the RAM disk; log: the boot disk's log partition, with each boot's system log; models: the model disk, read-only)\n- write <file> <text>, mkdir, rm, mv <from> <to>, sync: change files on ram:, on log: and in data/\n- faults: recent process faults\n- ps: tasks\n- quotas: task and endpoint quotas (used/limit)\n- budget <pid> <ms> <period ms>: CPU budget (0: no limit)\n- fg <id>: foreground\n- kill <id>: terminate\n- logs <id>: buffered output\n- logger <text>: a line in the system log (dmesg shows it)\n- net [arp <ip>]: network card (MAC, link, counters); ARP query while the stack is stopped\n- ip [offload on|off]: address, gateway and DNS server, every card; transmit checksum offload\n- netgrants, netrevoke <program>: flow grants of the network policy broker\n- netpolicy [add <line> | remove <line>]: the network policy's lines; a change after you agree to it, kept on the disk\n- ping <host>, nslookup <name> [server[:port]], fetch <host>[:port] [path]: network\n- https [-c] <host>[:port] [path] [name]: HTTPS GET, server certificate verified (-c: offer the device certificate)\n- tls cert: the device certificate (PEM)\n- tpm [seal <text>]: the TPM (manufacturer, interface); seal: a check that the shell may not seal\n- heap\n- clear\n- keymap [us|ru] [--switch both|ctrl-shift|alt-shift|caps|none]: keyboard layout and layout switch\n- voice on [--wav file] [seconds], voice off, voice listen: voice control (F12: speak, Esc: cancel; asks before stopping a service or rebooting)\n- screenshot [file]: the screen as a BMP (ram:screen-NNN.bmp)\n- reboot [-f] [--off]: write cached files to the disks, stop the services (not with -f) and restart the machine (--off: turn it off)\n- msh <file> [args], msh -c \"code\", msh --check <file>: scripts (docs/msh.md); let, if, for, while, fn and try work at the prompt too\n- stop\nCTRL+ALT+F5 OR SHELL IN WM'S MENU (A RIGHT CLICK ON THE DESKTOP, ALT+P): THE SHELL'S OWN WINDOW, TITLED SHELL; IT TAKES EVERY COMMAND BUT FG (REBOOT AND STOP ASK FIRST); PROGRAMS STARTED THERE OPEN IN WINDOWS (PS: CONSOLE=5). CONSOLE IN WM IS A TERMINAL FOR PROGRAMS WITH WHAT WM HOLDS, NOT A SECOND SHELL: THE SHELL ALONE HOLDS THE OPERATOR'S AUTHORITY.\nCTRL+Z: SHELL, KEEP RUNNING. ESC: EXIT FOREGROUND APP. CTRL+ALT+F1…F4: CONSOLES 1-4, EACH WITH ITS OWN LINE, HISTORY AND PROGRAMS (THE SERIAL LINE IS CONSOLE 1).\nKEYS: ←/→ HOME/END DEL EDIT THE LINE, ↑/↓ HISTORY, TAB COMPLETES, ESC CLEARS, SHIFT+PGUP/PGDN SCROLL, CTRL+L CLEARS THE SCREEN, CTRL+SHIFT OR ALT+SHIFT: EN/RU.\n";

// Words the shell completes with Tab besides program names.
const BOOT_LOG_LINES: usize = 24;
const COMMANDS: [&str; 49] = ["boot", "budget", "caps", "cat", "clear", "cpus", "date", "devices", "endpoints", "faults", "fetch", "fg", "free", "heap", "help", "https", "ip", "irqs", "keymap", "kill", "list", "logger", "logs", "ls", "mkdir", "msh", "mv", "net", "netgrants", "netpolicy", "netrevoke", "nslookup", "physmap", "ping", "pmap", "ps", "quotas", "reboot", "rm", "run", "screenshot", "stat", "stop", "sync", "time", "tls", "tpm", "voice", "write"];
const NAMES: usize = 128; // as many as the loader lists (loader.wit 1.4)
// Where the scoped VFS client for a program that asks for a file arrives: a fixed slot the shell does not use (11 is
// SLOT_LIFECYCLE in applications). The shell lends it to the program and drops its own copy.
const SCOPE_RECEIVE: usize = 11;
// Where a capability sent with a request of the shell's commands (idl/shell.wit) would arrive, to be dropped: a fixed
// slot the shell holds nothing in.
const COMMANDS_RECEIVE: usize = SLOT_SHELL;
// What a client gets back of a command's output: its end.
const ANSWER_MAX: usize = 7000;

const CONSOLES: usize = 4;
const LABELS: [&str; CONSOLES] = [" CONSOLE 1 ", " CONSOLE 2 ", " CONSOLE 3 ", " CONSOLE 4 "];
// The session in the shell's window (211-APP-0040), after the consoles.
const WINDOW: usize = CONSOLES;
const SESSIONS: usize = CONSOLES + 1;
const OWNERS: usize = 32;

// A console the shell keeps aside while another is active (issue 155): its text, input line, history and programs.
struct Session { term: Console, line: InputLine, history: History<32>, prompt_at: Position, focused: Option<u64>, line_start: bool, console: Option<u64>, msh: Interpreter }

// A session in memory of its own: one is 9 KB (mostly its history), and the shell's stack is 64 KB.
struct Parked { _pages: Pages, session: *mut Session }

impl Parked {
    fn new(session: Session) -> Option<Self> {
        let mut pages = Pages::new(core::mem::size_of::<Session>())?;
        let at = pages.as_mut_slice().as_mut_ptr() as *mut Session; // page-aligned
        unsafe { at.write(session); }
        Some(Self { _pages: pages, session: at })
    }
    fn get(&mut self) -> &mut Session { unsafe { &mut *self.session } }
}

impl Drop for Parked { fn drop(&mut self) { unsafe { core::ptr::drop_in_place(self.session); } } }

// The fields up to `console` are the active console's; `activate` swaps another one in.
struct Shell {
    term: Console, line: InputLine, history: History<32>, prompt_at: Position, own: u64, focused: Option<u64>, line_start: bool,
    console: Option<u64>, // console program running in the shell
    names: [[u8; NAME_MAX]; NAMES], name_lens: [usize; NAMES], name_count: usize, // program names for completion
    voice: voicectl::Voice,
    parked: [Option<Parked>; SESSIONS], // the other consoles and the window's session (None: the active one, or not opened yet)
    active: usize, shown: usize, // the console the shell works in now, and the one on the screen
    window: Option<mind::windowed::Window>, room: (usize, usize), // the shell's window, and the cells it has memory for (the screen's)
    windowed: Option<u64>, // the program the window's session last started in a window of its own
    commands: Option<Endpoint>, // the shell's commands for a window manager (idl/shell.wit, 211-APP-0044), made at the first lend
    owners: [(u64, u8); OWNERS], next_owner: usize, // which console started which program (for ps), the latest 32
    fronts: [Option<(u64, u8)>; 4], // programs a program in front started in its place, and their console (issue 160)
    serial: Option<Uart>, // the serial line, for notes that are not a console's
    msh: Interpreter, // the active console's: the variables and functions of statements typed at its prompt (issue 094)
    script: Option<alloc::vec::Vec<alloc::string::String>>, // while a script runs: what programs it starts may get
    log_next: Option<u64>, // the next system log record shown above the prompt, until the first key (211-PRT-0004)
    beat: (u64, u64), // until then: the line of the `UP n S` heartbeat above the prompt, and the second it shows
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
        Error::Other(ERR_LIMIT) => "TASK LIMIT REACHED (QUOTA)",
        Error::Other(ERR_IO) => "CANNOT READ THE PROGRAM: ITS DISK DOES NOT ANSWER (UNPLUGGED?)",
        Error::Other(ERR_FOCUS) => "A WINDOW MANAGER STARTS FROM A CONSOLE (CTRL+ALT+F1…F4), NOT FROM THE SHELL'S WINDOW",
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

// Every task: the list doubles until the kernel fills it no more (issue 171: no fixed count of tasks).
fn tasks() -> (alloc::vec::Vec<TaskInfo>, usize) {
    let mut list = alloc::vec![unsafe { core::mem::zeroed::<TaskInfo>() }; 64];
    loop {
        let count = control::tasks(&mut list).unwrap_or(0);
        if count < list.len() { return (list, count); }
        list.resize(list.len() * 2, unsafe { core::mem::zeroed::<TaskInfo>() });
    }
}

impl Shell {
    // Makes console `index` the active one: its fields swap places with those of the console that was. False when it
    // is not open.
    fn activate(&mut self, index: usize) -> bool {
        if index == self.active { return true; }
        let Some(parked) = self.parked[index].as_mut() else { return false };
        let session = parked.get();
        core::mem::swap(&mut self.term, &mut session.term);
        core::mem::swap(&mut self.line, &mut session.line);
        core::mem::swap(&mut self.history, &mut session.history);
        core::mem::swap(&mut self.prompt_at, &mut session.prompt_at);
        core::mem::swap(&mut self.focused, &mut session.focused);
        core::mem::swap(&mut self.line_start, &mut session.line_start);
        core::mem::swap(&mut self.console, &mut session.console);
        core::mem::swap(&mut self.msh, &mut session.msh);
        // The memory now holds the console that was active.
        self.parked[self.active] = self.parked[index].take();
        self.active = index;
        true
    }
    fn is_open(&self, index: usize) -> bool { index == self.active || self.parked[index].is_some() }

    // Ctrl+Alt+F1…F4: console `index` on the screen — its text and prompt, or its foreground program, which gets the
    // keyboard. A console is opened the first time it is shown.
    fn show(&mut self, index: usize) {
        if index == self.shown || index >= CONSOLES { return; }
        if !self.is_open(index) {
            let session = self.term.sibling().and_then(|term| Parked::new(Session { term, line: InputLine::new(), history: History::new(), prompt_at: Position { line: 0, col: 0 }, focused: None, line_start: true, console: None, msh: Interpreter::default() }));
            let Some(session) = session else { let _ = writeln!(self.term, "\nERROR: NO MEMORY FOR CONSOLE {}", index + 1); return self.prompt(); };
            self.parked[index] = Some(session);
            for i in 0..CONSOLES { if self.activate(i) { self.term.label = LABELS[i]; } }
            self.activate(index);
            let _ = writeln!(self.term, "CONSOLE {}. CTRL+ALT+F1…F4: CONSOLES. HELP: COMMANDS.", index + 1);
            self.prompt();
        }
        self.activate(self.shown);
        let screen = self.term.take_screen();
        self.activate(index);
        self.term.give_screen(screen);
        self.shown = index;
        let program = self.focused.filter(|&pid| mind::process::alive(pid));
        let _ = control::focus(program.unwrap_or(self.own), true);
        self.note(format_args!("CONSOLE {} SHOWN{}", index + 1, if program.is_some() { " (ITS PROGRAM HAS THE KEYBOARD)" } else { "" }));
    }

    // The shell's window (211-APP-0040), opened only when the user asks, with Ctrl+Alt+F5 (211-APP-0045) or from the
    // window manager's menu (211-APP-0044): a session of its own, which the manager shows as it shows a program's window
    // and passes keys to. Its id in the broker, also when it was open already.
    fn open_window(&mut self) -> Result<u32, commands::Error> {
        self.tend_window(); // a window the manager closed goes first
        if let Some(window) = &self.window { let id = window.id(); self.note(format_args!("THE SHELL'S WINDOW IS OPEN")); return Ok(id); }
        let window = mind::windowed::Window::open(Endpoint(SLOT_WINDOWS), SCOPE_RECEIVE, mind::window::Kind::Text, self.room, (80, 25), "shell").ok_or(commands::Error::Unavailable)?;
        let term = mind::tui::Terminal::in_window(window.surface(), self.room).and_then(|term| Console::in_window(term, self.room.0)).ok_or(commands::Error::NoMemory)?;
        let session = Parked::new(Session { term, line: InputLine::new(), history: History::new(), prompt_at: Position { line: 0, col: 0 }, focused: None, line_start: true, console: None, msh: Interpreter::default() }).ok_or(commands::Error::NoMemory)?;
        let id = window.id();
        self.parked[WINDOW] = Some(session);
        self.window = Some(window);
        let back = self.active;
        self.activate(WINDOW);
        let _ = writeln!(self.term, "THE SHELL'S WINDOW: EVERY COMMAND BUT FG, ON THE SHELL'S AUTHORITY; PROGRAMS OPEN IN WINDOWS. HELP: COMMANDS.");
        self.prompt();
        self.activate(back);
        self.note(format_args!("THE SHELL'S WINDOW OPENED"));
        Ok(id)
    }

    // A client of the shell's commands for a program that asks for it (REQUEST_SHELL): the endpoint is made the first
    // time; the caller lends the client and drops it.
    fn commands_client(&mut self) -> Result<usize, Error> {
        let endpoint = match self.commands { Some(endpoint) => endpoint, None => { let endpoint = Endpoint::create()?; self.commands = Some(endpoint); endpoint } };
        mind::ipc::mint(endpoint.0, CAP_WRITE | CAP_GRANT, 0, 0) // grant: `run`'s buffer travels with the call
    }

    // The requests to the shell's commands that came (idl/shell.wit 1.0: the window), answered at once.
    fn serve_commands(&mut self, wait_ms: u32) {
        let Some(endpoint) = self.commands else { return };
        let mut wait = wait_ms;
        while let Ok(message) = endpoint.recv_timeout(COMMANDS_RECEIVE, wait.max(1)) {
            wait = 1;
            let (request, call) = match commands::decode(&message, COMMANDS_RECEIVE) { Ok(decoded) => decoded, Err(reason) => { let _ = mind::idl::wire::reject(reason); continue; } };
            match request {
                commands::Request::Window => {
                    let result = self.open_window();
                    self.note(format_args!("THE WINDOW MANAGER ASKED FOR THE SHELL'S WINDOW: {}", if result.is_ok() { "SHOWN" } else { "NOT OPENED" }));
                    let _ = commands::reply_window(call, result);
                }
                commands::Request::Run { line } => {
                    let result = self.client_line(line.as_str(), message.sender);
                    let verdict = match &result { Ok(_) => "RAN", Err(commands::Error::Refused) => "REFUSED", Err(commands::Error::Declined) => "DECLINED", Err(_) => "DID NOT RUN" };
                    self.note(format_args!("FOR PID {} THE SHELL {} \"{}\"", message.sender, verdict, line.as_str()));
                    let answer = result.as_ref().map(|bytes| { let mut from = bytes.len().saturating_sub(ANSWER_MAX); while from < bytes.len() && bytes[from] & 0xC0 == 0x80 { from += 1; } &bytes[from..] });
                    let _ = commands::reply_run(call, answer.map_err(|e| *e));
                }
            }
        }
    }

    // A command line from a client of the shell's commands (`console` in a window manager, 211-APP-0044), run on the
    // shell's authority as if typed: what it printed. One that changes the machine runs in the shell's window, shown as
    // typed there with the client's PID, once the user agrees (reboot and stop ask themselves).
    fn client_line(&mut self, line: &str, client: u64) -> Result<alloc::vec::Vec<u8>, commands::Error> {
        let line = line.trim();
        let word = line.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
        let taken = clients::taken(line);
        if taken == clients::Taken::Refused { return Err(commands::Error::Refused); }
        if taken == clients::Taken::Now {
            let outer = self.term.capture.replace(alloc::vec::Vec::new());
            self.command(line.as_bytes());
            return Ok(core::mem::replace(&mut self.term.capture, outer).unwrap_or_default());
        }
        self.open_window()?;
        let back = self.active;
        self.activate(WINDOW);
        if self.console.is_some() { self.activate(back); return Err(commands::Error::Busy); }
        let _ = writeln!(self.term, "{}    (ASKED BY PID {} THROUGH THE SHELL'S COMMANDS)", line, client);
        let agreed = matches!(word.as_str(), "reboot" | "stop") || msh::ask(self, "RUN IT?");
        let result = if agreed {
            self.term.tee = true;
            let outer = self.term.capture.replace(alloc::vec::Vec::new());
            self.command(line.as_bytes());
            self.term.tee = false;
            Ok(core::mem::replace(&mut self.term.capture, outer).unwrap_or_default())
        } else { Err(commands::Error::Declined) };
        self.prompt();
        self.activate(back);
        result
    }

    // The shell's window between keys: the keys and wheel the manager queued, its size, its close; drawn whatever
    // console is on the screen. Closed, it goes with its session.
    fn tend_window(&mut self) {
        let Some(closed) = self.window.as_ref().map(mind::windowed::Window::closed) else { return };
        let back = if self.active == WINDOW { self.shown } else { self.active };
        if closed {
            self.activate(back);
            self.parked[WINDOW] = None;
            self.window = None;
            return self.note(format_args!("THE SHELL'S WINDOW CLOSED"));
        }
        self.activate(WINDOW);
        while let Some(word) = self.window.as_ref().and_then(mind::windowed::Window::event) {
            if mind::abi::event_key(word) == KEY_POINTER {
                let wheel = mind::abi::pointer_absolute_fields(word).map_or(0, |(_, _, _, wheel)| wheel);
                if wheel != 0 { self.term.scroll(wheel < 0); } // away from the user: back
            } else if let Some(key) = Key::from_event(word) {
                if self.focused.is_none() { self.key(key); }
            }
        }
        let cursor = (self.focused.is_none() && self.console.is_none()).then(|| self.cursor());
        self.term.render(cursor);
        self.activate(back);
    }

    /// A key for the active session: in the shell's window from the keys its manager queued, else from the keyboard.
    pub(crate) fn next_key(&mut self) -> Option<Key> {
        if self.active != WINDOW { return mind::input::read_key(); }
        let window = self.window.as_ref()?;
        while let Some(word) = window.event() { if let Some(key) = Key::from_event(word) { return Some(key); } }
        None
    }

    // A line on the serial line that belongs to no console (which console is shown).
    fn note(&mut self, text: core::fmt::Arguments) {
        let Some(uart) = &self.serial else { return };
        let mut line = mind::util::FixedBuf::<96>::new();
        let _ = write!(line, "\r\n[SHELL] {}\r\n", text);
        for &byte in line.as_bytes() { uart.write(byte); }
    }

    // The active console's programs between keys: the foreground one's output; a console program's output and end;
    // a foreground program that ended while its console was not shown (no notice comes for it).
    fn tend(&mut self) {
        if let Some(pid) = self.focused {
            self.mirror(pid);
            if self.active != self.shown && !mind::process::alive(pid) {
                self.focused = None;
                let _ = writeln!(self.term, "\nPID={} EXITED.", pid);
                self.prompt();
            }
        }
        if let Some(pid) = self.console { self.pump_console(pid); }
        self.mirror_fronts();
    }

    // Programs started in front by the foreground one (issue 160): their output too, each until it ends.
    fn mirror_fronts(&mut self) {
        for index in 0..self.fronts.len() {
            let Some((pid, _)) = self.fronts[index].filter(|f| f.1 as usize == self.active) else { continue };
            let ended = !mind::process::alive(pid);
            self.mirror(pid);
            if ended { self.fronts[index] = None; }
        }
    }

    // NOTICE_FRONT: the program in front of the shown console started `pid` in its place.
    fn add_front(&mut self, pid: u64) {
        if let Some(free) = self.fronts.iter_mut().find(|f| f.is_none()) { *free = Some((pid, self.shown as u8)); }
    }

    // The shell is in front of `console` again: the programs started in front there are not mirrored any more.
    fn drop_fronts(&mut self, console: usize) {
        for index in 0..self.fronts.len() {
            if let Some((pid, _)) = self.fronts[index].filter(|f| f.1 as usize == console) { self.mirror(pid); self.fronts[index] = None; }
        }
    }

    // The console whose foreground program `pid` is (the one shown when none).
    fn console_of(&self, pid: u64) -> usize {
        if self.focused == Some(pid) { return self.active; }
        (0..SESSIONS).find(|&i| self.parked[i].as_ref().is_some_and(|p| unsafe { (*p.session).focused } == Some(pid))).unwrap_or(self.shown)
    }

    fn owner(&self, pid: u64) -> Option<usize> { self.owners.iter().find(|o| o.0 == pid).map(|o| o.1 as usize) }

    fn report(&mut self, error: &str) { let _ = writeln!(self.term, "ERROR: {}", error); }
    // The prompt and whatever was typed so far (output may have interrupted the line).
    // The system log on the screen only, not the serial line (211-PRT-0004): the last BOOT_LOG_LINES records of the boot,
    // then each new record above the prompt until the first key, for a machine whose keyboard does not work yet. The
    // scrollback keeps the banner; the whole log is on the log partition (211-KRN-0019).
    fn show_log(&mut self, before_prompt: bool) {
        let Some(mut next) = self.log_next else { return };
        let mut text = alloc::string::String::new();
        let mut lines = alloc::collections::VecDeque::new();
        while let Ok(count) = mind::log::read(next, |entry| { next = entry.seq + 1; lines.push_back(alloc::string::String::from(entry.text.as_str())); if lines.len() > BOOT_LOG_LINES { lines.pop_front(); } }) {
            if count == 0 { break; }
        }
        for line in &lines { text.push_str(line); text.push('\n'); }
        self.log_next = Some(next);
        if before_prompt { self.term.put_str(&text); return; }
        // A heartbeat: the seconds since boot, so a still screen tells a stopped system from one waiting for keys.
        let second = mind::time::uptime_ms() as u64 / 1000;
        if text.is_empty() && second == self.beat.1 { return; }
        self.term.truncate(Position { line: self.beat.0, col: 0 });
        self.term.put_str(&text);
        self.beat = (self.term.position().line, second);
        let mut line = mind::util::FixedBuf::<32>::new();
        let _ = writeln!(line, "UP {} S", second);
        self.term.put_str(core::str::from_utf8(line.as_bytes()).unwrap_or(""));
        self.term.put_str("MIND> ");
        self.prompt_at = self.term.position();
    }

    fn prompt(&mut self) {
        if self.script.is_some() { return; } // a script's commands do not prompt
        let _ = write!(self.term, "MIND> ");
        self.prompt_at = self.term.position();
        if !self.line.is_empty() { self.redraw_input(true); }
    }

    // Output of the focused program goes to the serial line only, each line prefixed with its PID; in the consoles
    // without it, to their text, which shows it when the program has ended.
    fn mirror(&mut self, pid: u64) {
        let mut buffer = [0u8; 1024];
        let serial = self.term.serial.is_some();
        while let Ok(len @ 1..) = control::console(pid, &mut buffer) {
            for &byte in &buffer[..len] {
                if self.line_start && byte != b'\r' && byte != b'\n' {
                    let mut prefix = mind::util::FixedBuf::<32>::new(); let _ = write!(prefix, "[PID {}] ", pid);
                    for &b in prefix.as_bytes() { if serial { self.term.serial(b) } else { self.term.print_char(b) } }
                }
                if serial { self.term.serial(byte) } else { self.term.print_char(byte) }
                self.line_start = byte == b'\n';
            }
        }
    }

    // Kernel statistics (STAT), one record per line.
    fn stat(&mut self, args: &[u8]) {
        let text = core::str::from_utf8(args).unwrap_or("");
        // `stat <class> [pid] [from N]`: `from` pages through a class with more records than the buffer holds (issue 171).
        let words: alloc::vec::Vec<&str> = text.split_whitespace().collect();
        let name = words.first().copied().unwrap_or("");
        let from = words.iter().position(|w| *w == "from");
        let first = from.and_then(|i| words.get(i + 1)).and_then(|w| w.parse::<usize>().ok()).unwrap_or(0);
        let pid = words.get(1).filter(|_| from != Some(1)).and_then(|w| w.parse::<usize>().ok()).unwrap_or(0);
        let class = match name { "tasks" => STAT_TASKS, "cpus" => STAT_CPUS, "memory" => STAT_MEMORY, "physmap" => STAT_PHYSMAP, "vmap" => STAT_VMAP, "caps" => STAT_CAPS, "endpoints" => STAT_ENDPOINTS, "irqs" => STAT_IRQS, "devices" => STAT_DEVICES, _ => { self.report("STAT <CLASS> [PID]: SEE HELP"); return; } };
        let Some(mut page) = Pages::new(4 * 4096) else { self.report("OUT OF MEMORY"); return };
        // `stat memory` also asks for the largest free block (argument 1).
        let pid = if class == STAT_MEMORY { 1 } else { pid };
        let header = match control::stat_from(class, pid, first, page.as_mut_slice()) { Ok(h) => h, Err(error) => { self.report(if error == Error::NotFound { "NO SUCH PID" } else { "STAT FAILED" }); return; } };
        let buffer = page.as_slice(); let t = &mut self.term;
        let _ = writeln!(t, "STAT {} VERSION={} COUNT={} TOTAL={} FROM={}", Upper(name), header.version, header.count, header.total, first);
        match class {
            STAT_TASKS => for r in control::records::<StatTask>(buffer, header) { let _ = writeln!(t, "{} PARENT={} {} WAIT={}:{} CPU={} RUN_MS={} SENDS={} RECEIVES={} HEAP={} SHARED={} CAPS={} KERNEL={}{}", r.pid, r.parent, label(&r.name), r.wait, r.wait_on, r.cpu, r.run_ns / 1_000_000, r.sends, r.receives, r.heap_bytes, r.shared_bytes, r.caps, r.kernel_bytes, if r.focus != 0 { " FOCUS" } else { "" }); },
            STAT_CPUS => for (i, r) in control::records::<StatCpu>(buffer, header).enumerate() { let _ = writeln!(t, "CPU {} APIC={} ONLINE={} BUSY_MS={} IDLE_MS={} INTERRUPTS={} SWITCHES={} PID={}", i, r.apic_id, r.online, r.busy_ns / 1_000_000, r.idle_ns / 1_000_000, r.interrupts, r.switches, r.current_pid); },
            STAT_MEMORY => for r in control::records::<StatMemory>(buffer, header) { let _ = writeln!(t, "ARENA={} USED={} FREE={} LARGEST={} IMAGES={} STACKS={} TASK_PAGES={} PAGE_TABLES={} SCREENS={} HEAPS={} SHARED={} OBJECTS={} DMA={} TASKS={}/{} ENDPOINTS={}/{} FRAMES={} FRAMES_FREE={}", r.arena, r.used, r.free, r.largest_free, r.images, r.stacks, r.task_pages, r.page_tables, r.screens, r.heaps, r.shared, r.objects, r.dma, r.tasks, r.tasks_limit, r.endpoints, r.endpoints_limit, r.frames, r.frames_free); },
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

    // A program to the foreground of the active console; it gets the keyboard if that console is shown. A program in
    // the foreground of another console leaves it.
    fn focus(&mut self, pid: u64, keep_output: bool) -> Result<(), Error> {
        if self.active == self.shown { control::focus(pid, keep_output)?; }
        else if !tasks().0.iter().any(|t| t.pid == pid && t.screen != 0) { return Err(if tasks().0.iter().any(|t| t.pid == pid) { Error::Invalid } else { Error::NotFound }); }
        for parked in self.parked.iter_mut().flatten() { let session = parked.get(); if session.focused == Some(pid) { session.focused = None; } }
        self.focused = Some(pid); self.line_start = true;
        Ok(())
    }

    // Boot services are (re)started by init. Applications are started by the loader in a launch session: the shell, as
    // the user's agent, gives a program what it asks for in its ELF and the shell itself holds (MC-3.11): a sysmon
    // client for `REQUEST_SYSINFO`, a VFS client confined to the named file's directory for `REQUEST_FILE` (its own
    // VFS client, which writes on `ram:` and in `data/`, for `REQUEST_FILES`), its log
    // client (reads the system log) for `REQUEST_LOG`, its client of init (lifecycle control) for `REQUEST_LIFECYCLE`,
    // its sysmon client with the authority badge (who holds what) for `REQUEST_AUTHORITY`, in SLOT_SYSINFO in place of
    // the plain one, its compositor client (what is on the screen) for `REQUEST_DISPLAY`. Nothing is granted by
    // program name.
    fn start(&mut self, name: &[u8], args: &[u8], service: bool) -> Result<u64, Error> { self.start_with(name, args, service, &[], false) }
    // `start` in the foreground: a program with a screen is in front from its start (issue 160), so one that ends at once
    // still leaves its output and the exit notice.
    fn start_front(&mut self, name: &[u8], args: &[u8], service: bool) -> Result<u64, Error> { self.start_with(name, args, service, &[], true) }

    // `start`, lending `extra` (slot in the program, capability here) too.
    fn start_with(&mut self, name: &[u8], args: &[u8], service: bool, extra: &[(usize, usize)], front: bool) -> Result<u64, Error> {
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
            loader::Error::Unreadable => Error::Other(ERR_IO),
        };
        let needs = loader::inspect(Endpoint::LOADER, name)?.map_err(failed)?;
        // Requests `needs` has no field for (loader.wit 1.2): the network (issue 102), the authority client (issue 081).
        let requests = loader::inspect_requests(Endpoint::LOADER, name)?.map_err(failed)?;
        let authority = requests & mind::process::REQUEST_AUTHORITY != 0;
        // A window manager gets the window broker's manager client, a program in a window a plain client (issue 157).
        let window_manager = requests & mind::process::REQUEST_WINDOW_MANAGER != 0;
        // From the shell's window a program that is not a console one opens a window of its own, as one `wm` starts
        // does; a window manager starts from a console (211-APP-0040).
        if window_manager && self.active == WINDOW { return Err(Error::Other(ERR_FOCUS)); }
        let in_window = self.active == WINDOW && !needs.console && !mind::process::console_run(requests, args);
        let window = (requests & mind::process::REQUEST_WINDOW != 0 || in_window) && !window_manager;
        let display = requests & mind::process::REQUEST_DISPLAY != 0;
        // A program a script starts gets only what the script declared (issue 094); it runs without the rest.
        let granted = |word: &str| self.script.as_ref().is_none_or(|words| words.iter().any(|w| w == word));
        // The pin controller service's control client, where the board has one (issue 207).
        let gpio = requests & mind::process::REQUEST_GPIO != 0 && mind::dev::cap_info(SLOT_GPIO).0 != 0 && granted("gpio");
        // The shell's commands (idl/shell.wit, 211-APP-0044): for a window manager, which opens the shell's window from its
        // menu and passes them on to console, and for what the shell's window starts; that is, where the shell's window
        // can ask before a command that changes the machine.
        let commands = requests & mind::process::REQUEST_SHELL != 0 && (window_manager || self.active == WINDOW) && granted("shell");
        // The block store client (300-KRN-0001), where the store runs.
        let blockstore = requests & mind::process::REQUEST_BLOCKSTORE != 0 && mind::dev::cap_info(SLOT_BLOCKSTORE).0 != 0 && granted("blockstore");
        // A program that asks only to read gets the client with the get badge alone, in the same slot (300-KRN-0024).
        let blockstore_read = !blockstore && requests & mind::process::REQUEST_BLOCKSTORE_READ != 0 && mind::dev::cap_info(SLOT_BLOCKSTORE_READ).0 != 0 && granted("blockstore");
        // The parser service's client (109-APP-0016): it holds nothing, so lending it gives a program no authority.
        let parse = requests & mind::process::REQUEST_PARSE != 0 && mind::dev::cap_info(SLOT_PARSE).0 != 0 && granted("parse");
        let (needs, authority, window_manager, display) = (loader::Needs { sysinfo: needs.sysinfo && granted("sysinfo"), file: needs.file && (granted("file") || granted("files")),
            lifecycle: needs.lifecycle && granted("lifecycle"), log: needs.log && granted("log"), files: needs.files && granted("files"), ..needs },
            authority && granted("authority"), window_manager && granted("window-manager"), display && granted("display"));
        let network_wanted = requests & mind::process::REQUEST_NETWORK != 0 && granted("network");
        // The TLS service's client (351-APP-0017), lent only with a flow grant: without a flow it reaches nothing.
        let tls = network_wanted && requests & mind::process::REQUEST_TLS != 0 && mind::dev::cap_info(SLOT_TLS).0 != 0 && granted("tls");
        // `--help` only prints the program's text (mind::about!): nothing to ask the user for (211-APP-0013).
        let help = args.trim() == "--help";
        let firmware_granted = granted("firmware") && !help;
        // The camera for a program that asks for it: starting it is the user's request, so nothing more is asked (the
        // maintainer, 2026-10-09); the camera mark shows while a stream is open, and a script must have declared it.
        let camera = requests & mind::process::REQUEST_CAMERA != 0 && mind::dev::cap_info(SLOT_CAMERA).0 != 0 && granted("camera");
        // The firmware's variables (the boot order) only when the user agrees, asked every time (351-KRN-0027).
        let firmware = requests & mind::process::REQUEST_FIRMWARE != 0 && mind::dev::cap_info(SLOT_FIRMWARE).0 != 0 && firmware_granted
            && msh::ask(self, &alloc::format!("{} ASKS TO READ AND CHANGE THE FIRMWARE'S BOOT SETTINGS. ALLOW?", name.to_ascii_uppercase()));
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
        // The firmware privilege goes by its own method (loader 1.6); endpoints by `grant`.
        let lend = |slot: usize, cap: usize| if slot == SLOT_FIRMWARE { loader::grant_firmware(Endpoint::LOADER, session, cap) } else { loader::grant(Endpoint::LOADER, session, slot as u8, cap) }.map(|r| r.map_err(failed));
        let wanted = [(needs.sysinfo && !authority, SLOT_SYSINFO, SLOT_SYSINFO), (authority, SLOT_SYSINFO, SLOT_AUTHORITY), (scoped, SLOT_FILE, SCOPE_RECEIVE), (needs.files, SLOT_FILE, SLOT_VFS), (needs.log, SLOT_LOG, SLOT_LOG),
                      (needs.lifecycle, SLOT_LIFECYCLE, SLOT_INIT), (window, SLOT_WINDOW, SLOT_WINDOWS), (window_manager, SLOT_WINDOW, SLOT_WINDOW_MANAGER),
                      (display, SLOT_DISPLAY, SLOT_DISPLAY), (gpio, SLOT_GPIO, SLOT_GPIO), (camera, SLOT_CAMERA, SLOT_CAMERA), (blockstore, SLOT_BLOCKSTORE, SLOT_BLOCKSTORE),
                      (blockstore_read, SLOT_BLOCKSTORE, SLOT_BLOCKSTORE_READ), (firmware, SLOT_FIRMWARE, SLOT_FIRMWARE), (parse, SLOT_PARSE, SLOT_PARSE)];
        let lent = wanted.iter().filter(|w| w.0).map(|&(_, slot, cap)| (slot, cap)).chain(extra.iter().copied())
            .try_for_each(|(slot, cap)| match lend(slot, cap) { Ok(Ok(())) => Ok(()), Err(error) | Ok(Err(error)) => Err(error) });
        if scoped { let _ = mind::ipc::drop_cap(SCOPE_RECEIVE); } // the loader holds its copy now
        if let Err(error) = lent { let _ = loader::abort(Endpoint::LOADER, session); return Err(error); }
        // A program that asks for the network gets what the policy broker grants it, or runs without (issue 102).
        let mut network = None;
        if network_wanted {
            match net::grant(&mut self.term, name, SCOPE_RECEIVE, |cap| match lend(SLOT_NETWORK, cap) { Ok(Ok(())) => Ok(()), Err(error) | Ok(Err(error)) => Err(error) }) {
                Ok(badge) => network = badge,
                Err(error) => { let _ = loader::abort(Endpoint::LOADER, session); return Err(error); }
            }
        }
        if tls && network.is_some() {
            if let Ok(Err(error)) | Err(error) = lend(SLOT_TLS, SLOT_TLS) { let _ = loader::abort(Endpoint::LOADER, session); return Err(error); }
        }
        // Last, and not needed to start: a launch session holds 5 grants (requests-KRN.md); without it the program
        // runs, as from a console.
        if commands {
            if let Ok(client) = self.commands_client() { let _ = lend(SLOT_SHELL, client); let _ = mind::ipc::drop_cap(client); } // the loader holds its copy
        }
        // In front only from the console shown; refused (`rights`) when the shell is not in front: started as before.
        let committed = if front && self.active == self.shown {
            match loader::commit_in_front(Endpoint::LOADER, session)? { Err(loader::Error::Rights) => loader::commit(Endpoint::LOADER, session)?, other => other }
        } else { loader::commit(Endpoint::LOADER, session)? };
        let pid = committed.map_err(failed)?;
        if let Some(badge) = network { net::bind(badge, pid); }
        self.windowed = in_window.then_some(pid);
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
        // Started from the shell's window in a window of its own: the window's session goes on (211-APP-0040).
        let windowed = self.windowed.take() == Some(pid);
        let _ = writeln!(self.term, " {}", if windowed { "IN A WINDOW" } else if background { "BACKGROUND" } else { "FOREGROUND" });
        self.owners[self.next_owner] = (pid, self.active as u8);
        self.next_owner = (self.next_owner + 1) % OWNERS;
        if background || windowed { return; }
        match task {
            Some(t) if t.screen != 0 && self.active == WINDOW => {} // behind the manager: `fg` from a console
            Some(t) if t.screen != 0 => { let _ = self.focus(pid, true); }
            Some(t) if t.service != 0 => {}
            _ => self.console = Some(pid),
        }
    }

    // `<program> --help`: a console program prints its own text into the shell; one with a screen would print it out
    // of sight — it exits before it is in front, and its output goes with it — so the shell shows the same text from
    // the program's file instead of starting it.
    fn help_instead(&mut self, name: &[u8], args: &[u8]) -> bool {
        if args != b"--help" { return false; }
        let Ok(name) = core::str::from_utf8(name) else { return false };
        let console = loader::inspect_requests(Endpoint::LOADER, name).ok().and_then(Result::ok).is_some_and(|r| r & mind::process::REQUEST_CONSOLE != 0);
        !console && programs::show_about(&mut self.term, name)
    }

    fn run_program(&mut self, name: &[u8], args: &[u8], background: bool) {
        let service = BOOT_SERVICES.iter().chain(SERVICE_INSTANCES.iter()).any(|s| s.as_bytes().eq_ignore_ascii_case(name));
        let started = if background { self.start(name, args, service) } else { self.start_front(name, args, service) };
        match started {
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
        // msh (issue 094): its statements at the prompt, `msh` and script files.
        if msh::is_statement(line) { return msh::statement(self, line); }
        if is(b"msh") { return msh::command(self, args); }
        if msh::is_script(cmd) { return msh::command(self, line); }
        if is(b"run") || is(b"boot") {
            let (words, background) = if is(b"boot") {
                if !args.is_empty() { return self.report("BOOT TAKES NO ARGUMENTS"); }
                (&b"app"[..], false)
            } else if let Some(rest) = args.strip_suffix(b"&") { (rest.trim_ascii(), true) } else { (args, false) };
            // run <name> [arguments] [&]
            let split = words.iter().position(|b| b.is_ascii_whitespace()).unwrap_or(words.len());
            let (name, program_args) = (&words[..split], words[split..].trim_ascii());
            if name.is_empty() { let _ = writeln!(self.term, "USAGE: RUN <NAME> [ARGUMENTS] [&]"); return programs::list(&mut self.term, false, ""); }
            if name.len() > NAME_MAX { return self.report("PROGRAM NAME TOO LONG"); }
            if program_args.len() > ARGS_MAX { return self.report("ARGUMENTS TOO LONG"); }
            if self.help_instead(name, program_args) { return; }
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
                if self.active == WINDOW { return self.report("FG WORKS IN A CONSOLE (CTRL+ALT+F1…F4): THE SHELL'S WINDOW HAS NO SCREEN TO GIVE"); }
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
        } else if !args.is_empty() && [&b"cpus"[..], b"faults", b"ps", b"quotas", b"clear", b"stop", b"heap", b"time", b"date", b"free", b"physmap", b"irqs", b"devices", b"endpoints"].iter().any(|c| is(c))
                  && !(is(b"date") && args.split(|&b| b == b' ').next() == Some(&b"set"[..])) {
            self.report("THIS COMMAND TAKES NO ARGUMENTS");
        } else if is(b"help") {
            if args.is_empty() { let _ = write!(self.term, "{}", HELP); } else { programs::help(&mut self.term, args, HELP); }
        } else if is(b"list") {
            // list [-l] [mask...]: `list a*` shows the programs whose names start with a.
            let text = core::str::from_utf8(args).unwrap_or("");
            let long = text.split_whitespace().any(|w| w.eq_ignore_ascii_case("-l"));
            if text.split_whitespace().any(|w| w.starts_with('-') && !w.eq_ignore_ascii_case("-l")) { return self.report("USAGE: LIST [-L] [MASK] (LIST A*: THE PROGRAMS STARTING WITH A)"); }
            let mut masks = mind::util::FixedBuf::<128>::new();
            for word in text.split_whitespace().filter(|w| !w.eq_ignore_ascii_case("-l")) { let _ = write!(masks, "{} ", word); }
            programs::list(&mut self.term, long, core::str::from_utf8(masks.as_bytes()).unwrap_or("").trim());
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
                let _ = write!(self.term, "{} {} {} {} {} {} {} {}", t.pid, label(&t.name), label(&t.state), if t.focus != 0 { "FG" } else { "BG" }, t.cpu, t.runs, t.ticks, t.calls);
                let _ = match self.owner(t.pid) { Some(console) => writeln!(self.term, " CONSOLE={}", console + 1), None => writeln!(self.term) };
            }
            let _ = writeln!(self.term, "{} TASK(S); SHELL PID={}; NO FIXED LIMIT: MEMORY AND QUOTAS DECIDE", count, self.own);
        } else if is(b"clear") {
            self.term.clear();
        } else if is(b"reboot") {
            // Keys in the shell's window come through the window manager: it asks first.
            if self.active == WINDOW && !msh::ask(self, "RESTART THE MACHINE?") { return; }
            power::reboot(&mut self.term, args);
        } else if is(b"voice") {
            self.voice_command(args);
        } else if is(b"keymap") {
            keymap::command(&mut self.term, args);
        } else if is(b"screenshot") {
            screenshot::command(&mut self.term, args, SCOPE_RECEIVE);
        } else if is(b"stop") {
            if self.active == WINDOW && !msh::ask(self, "HALT THE SYSTEM?") { return; }
            files::flush_all(); // what vfs_server still caches goes to the disks first
            let _ = writeln!(self.term, "SYSTEM HALTED. CPU GOING TO SLEEP...");
            control::halt();
        } else if is(b"date") {
            // `date set`: the shell's rtc client carries the setting badge (rtc.wit 1.2, 211-APP-0042).
            if let Some(setting) = args.strip_prefix(b"set") {
                let text = core::str::from_utf8(setting).unwrap_or("");
                let Some((days, seconds)) = mind::rtc::parse_setting(text) else { self.report("USAGE: DATE SET YYYY-MM-DD HH:MM[:SS] (2000-01-01 TO 2099-12-31)"); return; };
                match mind::idl::rtc::set(Endpoint::RTC, days, seconds) {
                    Ok(Ok(())) => { let _ = writeln!(self.term, "CLOCK SET. IT KEEPS NO TIME ZONE: THIS IS THE TIME IT SHOWS."); }
                    Ok(Err(mind::idl::rtc::Error::Rights)) => { self.report("ONLY THE SHELL'S RTC CLIENT MAY SET THE CLOCK"); return; }
                    Ok(Err(mind::idl::rtc::Error::Invalid)) => { self.report("THE CLOCK REFUSED THAT TIME"); return; }
                    Ok(Err(mind::idl::rtc::Error::Unavailable)) | Err(_) => { self.report("RTC NOT AVAILABLE"); return; }
                }
            }
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
        } else if is(b"netpolicy") {
            net::policy(self, args);
        } else if is(b"ip") {
            net::ip(&mut self.term, args);
        } else if is(b"ping") {
            net::ping(&mut self.term, args);
        } else if is(b"nslookup") {
            net::nslookup(&mut self.term, args);
        } else if is(b"fetch") {
            net::fetch(&mut self.term, args);
        } else if is(b"https") {
            net::https(&mut self.term, args);
        } else if is(b"tls") {
            net::tls_command(&mut self.term, args);
        } else if is(b"tpm") {
            tpm::command(&mut self.term, args);
        } else if is(b"time") {
            // One line (`clock` is the clock program, full screen): the time of day from the RTC, then the kernel's clock.
            let (ns, resolution, hz) = mind::time::clock_info();
            let _ = match mind::rtc::seconds_since_midnight() {
                Some(s) => write!(self.term, "TIME: {:02}:{:02}:{:02} ", s / 3600, s / 60 % 60, s % 60),
                None => write!(self.term, "TIME: --:--:-- "),
            };
            let _ = writeln!(self.term, "UPTIME MS={} MONOTONIC NS={} RESOLUTION NS={} TSC HZ={}", mind::time::uptime_ms(), ns, resolution, hz);
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
        } else if cmd.len() <= NAME_MAX && cmd.is_ascii() && !BOOT_SERVICES.iter().chain(SERVICE_INSTANCES.iter()).any(|s| s.as_bytes().eq_ignore_ascii_case(cmd)) {
            // Any other word runs the program of that name in the foreground: `say hello`, `listen 3`.
            if self.help_instead(cmd, args) { return; }
            match self.start_front(cmd, args, false) {
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
        for name in BOOT_SERVICES.iter().chain(SERVICE_INSTANCES.iter()) { add(self, name.as_bytes()); }
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
        // F12: push-to-talk; Esc or Enter answers a question voice control asked (issue 079).
        if self.voice_key(key) { return; }
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
    // The serial line is the first console's: keys for its foreground program go to the program with the keyboard
    // only while that console is shown.
    fn uart(&mut self, event: Event) {
        let shown = self.shown == self.active;
        match event {
            Event::Key(word) if self.focused.is_some() => { if shown { let _ = input_key(word, word, false); } }
            Event::Key(word) => self.key(Key(word)),
            Event::Attention if self.focused.is_some() && shown => { let _ = input_key(0, 0, true); }
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

// The shell's state on the heap: the commands and scripts it runs need the 64 KiB stack (issue 094).
#[inline(never)]
fn new_shell(term: Console, own: u64, room: (usize, usize)) -> alloc::boxed::Box<Shell> {
    alloc::boxed::Box::new(Shell { term, line: InputLine::new(), history: History::new(), prompt_at: Position { line: 0, col: 0 }, own, focused: None, line_start: true,
                            console: None,
                            names: [[0; NAME_MAX]; NAMES], name_lens: [0; NAMES], name_count: 0, voice: voicectl::Voice::default(),
                            parked: [const { None }; SESSIONS], active: 0, shown: 0, window: None, room, windowed: None, commands: None, owners: [(0, 0); OWNERS], next_owner: 0, fronts: [None; 4], serial: Uart::open(SLOT_SERIAL),
                            msh: Interpreter::default(), script: None, log_next: None, beat: (0, u64::MAX) })
}

mind::entry!(main);
fn main(info: &'static BootInfo) {
    let term = Console::new(mind::gfx::Screen::new(info), Uart::open(SLOT_SERIAL));
    let own = control::focus(0, false).unwrap_or(0);
    // The shell's window has memory for the screen's cells, as a program's window has (`Terminal::open`).
    let room = ((info.width / 8).clamp(1, mind::window::MAX_COLUMNS), (info.height / 16).clamp(1, mind::window::MAX_ROWS));
    let mut shell = new_shell(term, own, room);
    // Ctrl+Alt+F1…F4 come to the shell whatever program has the focus, and never to that program (INPUT_LISTEN);
    // Ctrl+Alt+F5 opens the shell's window (211-APP-0040, 211-APP-0045).
    for index in 0..SESSIONS { let _ = mind::input::listen(KEY_F1 + index as u16, MOD_CTRL | MOD_ALT, true); }
    let (used, free, _) = control::kernel_heap();
    let _ = writeln!(shell.term, "MIND CORE v1.6 [Build: 2026-10-03]. SMP / RING 3 SERVICES / RING 3 SHELL.");
    let _ = writeln!(shell.term, "MEMORY MANAGER: {} MB HEAP.", (used + free) / 1024 / 1024);
    let _ = writeln!(shell.term, "LIST: PROGRAMS. RUN <NAME> [&]. PS. FG <ID>. HELP. TAB COMPLETES, ↑/↓ HISTORY, CTRL+SHIFT: EN/RU.");
    // The boot's log after the banner, which stays the first line of the scrollback.
    shell.log_next = Some(0); shell.show_log(true);
    shell.beat.0 = shell.term.position().line;
    shell.prompt();
    let mut vt = Vt::new();
    let mut events = Events::new();
    loop {
        for index in 0..SESSIONS { if shell.activate(index) { shell.tend(); } }
        shell.tend_window();
        // What happened to the program with the keyboard: it is the foreground program of the console shown.
        while let Some(notice) = control::notice() {
            let pid = match notice { Notice::Exited(pid) | Notice::Background(pid) => pid, Notice::Front(pid) => { shell.add_front(pid); continue; } };
            shell.activate(shell.console_of(pid));
            shell.drop_fronts(shell.active);
            let what = match notice { Notice::Exited(pid) => { shell.mirror(pid); "EXITED" } Notice::Background(_) | Notice::Front(_) => "BACKGROUND" };
            shell.focused = None;
            let _ = writeln!(shell.term, "\nPID={} {}. SHELL RESUMED.", pid, what);
            shell.prompt();
        }
        // UART: terminal input decoded into key events (VT100/xterm sequences, UTF-8, a lone Esc after a timeout), for
        // the first console.
        shell.activate(0);
        shell.show_log(false);
        let now = mind::time::uptime_ms() as u64;
        while let Some(byte) = shell.term.serial.as_ref().and_then(Uart::read) {
            vt.feed(byte, now, &mut |event| events.push(event));
        }
        vt.poll(now, &mut |event| events.push(event));
        for &event in events.as_slice() { shell.log_next = None; shell.uart(event); }
        events.clear();
        // PS/2 keys arrive in the shell's queue while it has the focus, for the console shown.
        // While a program has the focus only the keys the shell listens for come here (F12: push-to-talk, issue 154;
        // Ctrl+Alt+F1…F4: the consoles, issue 155).
        shell.activate(shell.shown);
        while let Some(key) = mind::input::read_key() {
            shell.log_next = None;
            match key.code() {
                Code::F(n @ 1..=4) if key.ctrl() && key.alt() => shell.show(n as usize - 1),
                Code::F(5) if key.ctrl() && key.alt() => { let _ = shell.open_window(); }
                _ if shell.focused.is_none() => shell.key(key),
                Code::F(12) => { shell.voice_key(key); }
                _ => {}
            }
        }
        let cursor = (shell.focused.is_none() && shell.console.is_none()).then(|| shell.cursor());
        shell.term.render(cursor);
        // Wait for the next tick, or for the voice program's call or the window manager's.
        match shell.voice.endpoint {
            Some(endpoint) => {
                if let Ok(request) = endpoint.recv_timeout(voicectl::VOICE_RECEIVE, 10) { shell.voice_message(&request); }
                shell.voice_check();
                shell.serve_commands(1);
            }
            None if shell.commands.is_some() => shell.serve_commands(10),
            None => { let _ = mind::time::sleep(10); }
        }
    }
}

// Upper-case display of an ASCII word.
struct Upper<'a>(&'a str);
impl core::fmt::Display for Upper<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result { for c in self.0.chars() { write!(f, "{}", c.to_ascii_uppercase())?; } Ok(()) }
}
