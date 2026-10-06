#![no_std]
#![no_main]
// console (issue u004): a terminal for programs, in a window of wm or on its own screen. A line typed starts a program;
// a console program (uptime, df, grep, …) is lent an endpoint of console's in SLOT_CONSOLE (issue 162) and what it
// prints shows here, as the shell shows it; a program with a screen opens a window of its own (in wm) or a screen in
// the background. Of what a program asks for, it gets what console holds: the user's files and system information.
extern crate alloc;

mod builtins;
mod screen;
pub use mind::{keys, tui};

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use mind::abi::*;
use mind::idl::loader;
use mind::input::{Input, Key};
use mind::ipc::{self, Endpoint};
use mind::tui::Terminal;
use screen::{parse, Command, Kind, Screen};

// The network only for its own `ping`: a flow grant the policy names for console, when the shell starts it.
mind::request!(REQUEST_FILES | REQUEST_SYSINFO | REQUEST_NETWORK);

const SCOPE: usize = 13; // a file client confined to one directory, for a program that asks for one file
const HELP: &str = "Type a program and its arguments: uptime, df, find ram: -name *.txt, grep -i word docs/notes.txt, fm, …\n\
A console program prints here; one with a screen opens a window of its own. run <program> starts a program even where a\n\
command has its name (run ping: the IPC demo).\n\
Commands: ps, ls [dir], cat <file>, date, time, ping <host>, mkdir, rm, mv, write <file> <text>; list: the programs;\n\
clear (Ctrl+L): clear; exit: close. ↑ ↓: earlier lines; PgUp PgDn or the wheel: scroll back.\n\
kill, fg, logs, ip, nslookup, fetch and the shell's other commands need what only the shell holds: type them there.";

struct Job { pid: u64, name: String, console: bool, printed: bool }

fn holds(slot: usize) -> bool { mind::dev::cap_info(slot).0 == CAP_KIND_ENDPOINT }

// Starts `name` with `args`, lending what it asks for of what console holds, and `output` (a send copy of console's
// endpoint) to a console program.
fn run(name: &str, args: &str, output: usize) -> Result<Job, String> {
    let failed = |error: loader::Error| match error { loader::Error::NotFound => format!("{}: no such program", name), other => format!("{}: {:?}", name, other) };
    let lost = |_| format!("{}: the loader does not answer", name);
    let needs = loader::inspect(Endpoint::LOADER, name).map_err(lost)?.map_err(failed)?;
    let requests = loader::inspect_requests(Endpoint::LOADER, name).map_err(lost)?.map_err(failed)?;
    if requests & mind::process::REQUEST_WINDOW_MANAGER != 0 { return Err(format!("{}: a window manager; start it from the shell", name)); }
    let console = mind::process::console_run(requests, args); // `clock --line` too (issue u016)
    let session = loader::begin(Endpoint::LOADER, name, args).map_err(lost)?.map_err(failed)?;
    let grant = |slot: usize, cap: usize| matches!(loader::grant(Endpoint::LOADER, session, slot as u8, cap), Ok(Ok(())));
    if console { grant(SLOT_CONSOLE, output); } else if mind::windowed::active() { grant(SLOT_WINDOW, SLOT_WINDOW); }
    if needs.files { if holds(SLOT_FILE) { grant(SLOT_FILE, SLOT_FILE); } }
    else if needs.file && holds(SLOT_FILE) {
        // One file: a client confined to its directory, as the shell and wm make it.
        let path = args.split_whitespace().next().unwrap_or("");
        let (volume, rest) = if path.is_empty() { ("ram", "") } else { mind::fs::split(path) };
        let parent = rest.rfind('/').map_or("", |i| &rest[..i]);
        if mind::fs::Dir::root(volume).and_then(|root| root.dir(parent, false)).and_then(|dir| dir.scope(true, SCOPE)).is_ok() {
            grant(SLOT_FILE, SCOPE);
            let _ = ipc::drop_cap(SCOPE);
        }
    }
    if needs.sysinfo && requests & mind::process::REQUEST_AUTHORITY == 0 && holds(SLOT_SYSINFO) { grant(SLOT_SYSINFO, SLOT_SYSINFO); }
    // The window wm lent console to see, for the recorder console was started with (issue u014): passed on once.
    if requests & mind::process::REQUEST_DISPLAY != 0 && mind::dev::cap_info(SLOT_DISPLAY).0 == CAP_KIND_MEMORY {
        let _ = loader::grant_memory(Endpoint::LOADER, session, SLOT_DISPLAY as u8, SLOT_DISPLAY);
        let _ = ipc::drop_cap(SLOT_DISPLAY);
    }
    let pid = loader::commit(Endpoint::LOADER, session).map_err(lost)?.map_err(failed)?;
    Ok(Job { pid, name: String::from(name), console, printed: false })
}

fn list(screen: &mut Screen) {
    let Ok(programs) = loader::list(Endpoint::LOADER) else { screen.say("list: the loader does not answer", Kind::Error); return };
    let mut names: Vec<&str> = programs.as_slice().iter().filter(|p| !p.service).map(|p| p.name.as_str()).collect();
    names.sort_unstable();
    let mut line = String::new();
    for name in names {
        if line.len() + 14 > 78 { screen.say(&line, Kind::Note); line.clear(); }
        line.push_str(&format!("{:<14}", name));
    }
    if !line.is_empty() { screen.say(&line, Kind::Note); }
}

mind::entry!(main);
fn main(info: &'static BootInfo) {
    mind::about!("console — a terminal for programs: type a program and its arguments; what a console program prints shows here,\none with a screen opens a window of its own (in wm).\nUsage: console [program [arguments]]\nlist: the programs; clear or Ctrl+L: clear; exit: close; ↑ ↓: earlier lines; PgUp PgDn or the wheel: scroll back.");
    let Some(mut term) = Terminal::open(info, "console") else { return };
    // Where the programs' output arrives, and the copy each console program gets.
    let Ok(inbox) = Endpoint::create() else { mind::println!("[CONSOLE] NO ENDPOINT"); return };
    let Ok(output) = ipc::mint(inbox.0, CAP_WRITE, 0, 0) else { mind::println!("[CONSOLE] NO ENDPOINT"); return };
    if holds(SLOT_FILE) { mind::fs::use_endpoint(Endpoint(SLOT_FILE)); }
    let mut screen = Screen::new();
    let mut jobs: Vec<Job> = Vec::new();
    screen.say("console: type a program and its arguments, or help", Kind::Note);
    // A line to run, and whether it still has to be shown after the prompt (the one console was started with).
    let mut pending = Some((String::from(mind::process::args_str().trim()), true)).filter(|(c, _)| !c.is_empty());
    mind::input::pointer(true);
    mind::println!("[CONSOLE] READY");
    let mut dirty = true;
    let mut checked = 0usize;
    loop {
        if let Some((line, echo)) = pending.take() {
            if echo { screen.say(&format!("{}{}", screen::PROMPT, line), Kind::Command); }
            match parse(&line) {
                Command::Nothing => {}
                Command::Help => screen.say(HELP, Kind::Note),
                Command::Clear => screen.clear(),
                Command::Exit => break,
                Command::List => list(&mut screen),
                Command::Builtin { name, args } => builtins::run(&mut screen, name, args),
                Command::Shell(name) => screen.say(&format!("{}: a command of the shell (it needs what only the shell holds); type it in the shell", name), Kind::Error),
                Command::Run { name: "", .. } => screen.say("run <program> [arguments]", Kind::Error),
                Command::Run { name, args } => match run(name, args, output) {
                    Ok(job) => {
                        mind::println!("[CONSOLE] RUN {} PID {}{}", job.name, job.pid, if job.console { "" } else { " ITS OWN SCREEN" });
                        if !job.console {
                            let place = if mind::windowed::active() { "in a window of its own" } else { "on a screen of its own, in the background (FG in the shell)" };
                            screen.say(&format!("{} (PID {}) {}", job.name, job.pid, place), Kind::Note);
                        } else { jobs.push(job); }
                    }
                    Err(error) => { mind::println!("[CONSOLE] {}", error); screen.say(&error, Kind::Error); }
                },
            }
            dirty = true;
        }
        if dirty {
            let busy = jobs.iter().map(|j| j.name.as_str()).collect::<Vec<_>>().join(", ");
            let cursor = { let mut grid = term.grid(); screen.draw(&mut grid, &busy) };
            term.set_cursor(cursor);
            term.set_title(&if busy.is_empty() { String::from("console") } else { format!("console — {}", busy) });
            term.present();
            dirty = false;
        }
        // What the programs print; then the keys; ended programs.
        let mut wait = 20;
        while let Ok(message) = inbox.recv_timeout(0, wait) {
            if let Some(job) = jobs.iter_mut().find(|j| j.pid == message.sender) { job.printed = true; }
            let mut bytes = [0u8; mind::output::CHUNK];
            let len = mind::output::unpack(message.data, &mut bytes);
            screen.output(&bytes[..len]);
            dirty = true;
            wait = 1;
        }
        let page = term.rows().saturating_sub(2);
        while let Some(input) = mind::input::read_input() {
            match input {
                Input::Key(event) => {
                    let Some(key) = Key::from_event(event.to_word()) else { continue };
                    if let Some(line) = screen.key(key, page) { pending = Some((line, false)); }
                    dirty = true;
                }
                Input::Pointer(p) if p.wheel != 0 => { screen.wheel(p.wheel); dirty = true; }
                Input::Pointer(_) => {}
            }
        }
        if mind::windowed::resize_pending() { dirty = true; }
        let now = mind::time::uptime_ms();
        if now - checked >= 200 {
            checked = now;
            jobs.retain(|job| {
                if mind::process::alive(job.pid) { return true; }
                mind::println!("[CONSOLE] ENDED {} PID {}", job.name, job.pid);
                // A program that ends at once still shows that it ran (issue u011).
                screen.say(&format!("({} ended{})", job.name, if job.printed { "" } else { " without printing anything" }), Kind::Note);
                dirty = true;
                false
            });
        }
    }
    mind::println!("[CONSOLE] DONE");
}
