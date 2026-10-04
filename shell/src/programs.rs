//! `list [-l]`: the programs on the boot disk, sorted, in columns that fit the screen, and the services; with `-l` one
//! line per program with its size and what it does. 55 entries one per line scrolled the first ones (`fm`) off a
//! 50-line screen.
use crate::console::Console;
use core::fmt::Write;
use mind::abi::{BOOT_SERVICES, NAME_MAX, SERVICE_INSTANCES};
use mind::idl::loader;
use mind::ipc::Endpoint;

/// What each program of the system does (`list -l`); a program not named here shows only its size.
const ABOUT: [(&str, &str); 32] = [
    ("app", "graphics demo (Esc exits)"),
    ("app2", "test program: a second demo"),
    ("beep", "tones and a PCM sweep on the speaker"),
    ("caps", "capabilities of the tasks and their derivation tree"),
    ("clock", "a digital clock from the RTC"),
    ("df", "volumes: size and free space"),
    ("dmesg", "the system log"),
    ("dzen-clock", "the Dzen clock"),
    ("edit", "text editor: edit <file>"),
    ("files", "VFS demo: lists the disk and reads a file"),
    ("find", "find files by name, type or size: find [path] [-name mask]"),
    ("fm", "file manager: two panels, view, edit, copy, move, delete"),
    ("format", "format the RAM disk: format ram: [-l label] -y"),
    ("fsck", "check the FAT volumes (changes nothing)"),
    ("grep", "search text in files: grep [-i -n -l -c -r] text path"),
    ("hear", "recognize voice commands: hear [seconds] | hear --wav file"),
    ("hw", "hardware: CPU, framebuffer, PCI devices, interrupts"),
    ("ipc", "endpoints, their holders and who waits for whom"),
    ("keys", "show the key events a program gets"),
    ("listen", "record from the microphone; --vad: find speech; --wav file"),
    ("load", "CPU load graphs"),
    ("memmap", "memory: physical map, kernel arena, address spaces"),
    ("netbench", "network benchmark: netbench <ip>:<port> [MB]"),
    ("netcheck", "network access checks: netcheck tcp:<ip>:<port> ..."),
    ("ping", "IPC demo: the client pong starts"),
    ("pong", "IPC demo: lends a page to ping"),
    ("say", "speak text: say [-p pitch] [-r rate] text"),
    ("svc", "services: state, stop, start, restart"),
    ("top", "processes: CPU, memory, sorting, tree"),
    ("uptime", "uptime and load averages"),
    ("view", "file viewer, text and hex: view <file>"),
    ("voice", "voice control's listener (the shell starts it: voice on)"),
];

fn name(entry: &([u8; NAME_MAX], usize, u64)) -> &str { core::str::from_utf8(&entry.0[..entry.1]).unwrap_or("?") }

fn about(name: &str) -> &'static str { ABOUT.iter().find(|(n, _)| *n == name).map_or("", |(_, text)| text) }

/// `list` (programs in columns) or `list -l` (one per line with what it does).
pub fn list(out: &mut Console, long: bool) {
    let programs = match loader::list(Endpoint::LOADER) { Ok(programs) => programs, Err(_) => { let _ = writeln!(out, "ERROR: CANNOT LIST THE BOOT DISK"); return; } };
    // Applications only, sorted by name (services are listed below).
    let mut names: [([u8; NAME_MAX], usize, u64); 64] = [([0; NAME_MAX], 0, 0); 64];
    let mut count = 0;
    for program in programs.as_slice().iter().filter(|p| !p.service) {
        let name = program.name.as_str().as_bytes();
        let len = name.len().min(NAME_MAX);
        if count < names.len() { names[count].0[..len].copy_from_slice(&name[..len]); names[count].1 = len; names[count].2 = program.size; count += 1; }
    }
    let names = &mut names[..count];
    names.sort_unstable_by(|a, b| a.0[..a.1].cmp(&b.0[..b.1]));

    let _ = writeln!(out, "PROGRAMS ON DISK ({}): RUN <NAME> [ARGS] [&], OR JUST <NAME> [ARGS].{}", count, if long { "" } else { " LIST -L: WHAT EACH ONE DOES." });
    if long {
        for entry in names.iter() { let _ = writeln!(out, "  {:<12} {:>5} KB  {}", name(entry), entry.2.div_ceil(1024), about(name(entry))); }
    } else {
        // Down the columns, as ls does, as many as fit.
        let width = names.iter().map(|e| e.1).max().unwrap_or(1) + 2;
        let columns = ((out.cols().saturating_sub(2)) / width).clamp(1, 8);
        let rows = count.div_ceil(columns);
        for row in 0..rows {
            let _ = write!(out, "  ");
            for column in 0..columns {
                if let Some(entry) = names.get(column * rows + row) { let _ = write!(out, "{:<w$}", name(entry), w = width); }
            }
            let _ = writeln!(out);
        }
    }
    let _ = write!(out, "SERVICES (STARTED AT BOOT; SVC SHOWS THEIR STATE):");
    for service in BOOT_SERVICES.iter().chain(SERVICE_INSTANCES.iter()) { let _ = write!(out, " {}", service); }
    let _ = writeln!(out, "\nCTRL+Z: BACK TO THE SHELL, THE PROGRAM KEEPS RUNNING. ESC: EXIT A PROGRAM. HELP: THE SHELL'S COMMANDS.");
}
