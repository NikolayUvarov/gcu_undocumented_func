//! `list [-l]`: the programs on the boot disk, sorted, in columns that fit the screen, and the services; with `-l` one
//! line per program with its size and what it does (the first line of its `mind::about!` text). 55 entries one per
//! line scrolled the first ones (`fm`) off a 50-line screen. `help <name>`: what a command or program does.
use crate::console::Console;
use core::fmt::Write;
use mind::abi::{BOOT_SERVICES, NAME_MAX, SERVICE_INSTANCES};
use mind::fs::File;
use mind::util::FixedBuf;
use mind::idl::loader;
use mind::ipc::Endpoint;

/// What a program says about itself (`mind::about!`: the `.mind_about` section of `name.elf`), read from the file
/// without starting the program, into `out`; None for a program without it, or no such program.
pub fn about<'a>(name: &str, out: &'a mut [u8]) -> Option<&'a str> {
    if name.is_empty() || name.len() > NAME_MAX || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') { return None; }
    let mut path = FixedBuf::<{ NAME_MAX + 4 }>::new();
    let _ = write!(path, "{}.elf", name);
    let file = File::open(core::str::from_utf8(path.as_bytes()).ok()?).ok()?;
    let mut read = |at: usize, buffer: &mut [u8]| file.read_at(at, buffer).unwrap_or(0);
    let (offset, size) = mind::process::section(&mut read, ".mind_about")?;
    let length = size.min(out.len());
    if read(offset, &mut out[..length]) < length { return None; }
    let text = &out[..length];
    Some(core::str::from_utf8(text).unwrap_or_else(|e| core::str::from_utf8(&text[..e.valid_up_to()]).unwrap_or("")))
}

fn name(entry: &([u8; NAME_MAX], usize, u64)) -> &str { core::str::from_utf8(&entry.0[..entry.1]).unwrap_or("?") }

/// The first line of what `name` says about itself, without `name — ` (`list -l`).
fn summary<'a>(name: &str, out: &'a mut [u8]) -> &'a str {
    let line = about(name, out).and_then(|text| text.lines().next()).unwrap_or("");
    line.split_once(" — ").filter(|(first, _)| first.eq_ignore_ascii_case(name)).map_or(line, |(_, rest)| rest)
}

/// `help <name>`: the lines of the shell's `commands` that name it, and what the program of that name says about
/// itself (or that it is a service).
pub fn help(out: &mut Console, name: &[u8], commands: &str) {
    let name = core::str::from_utf8(name).unwrap_or("").trim();
    let mut found = false;
    for line in commands.lines().filter(|line| line.starts_with("- ")) {
        let part = line[2..].split(": ").next().unwrap_or("");
        if part.split(", ").filter_map(|item| item.split_whitespace().next()).any(|word| word.eq_ignore_ascii_case(name)) {
            let _ = writeln!(out, "{}", line);
            found = true;
        }
    }
    let mut text = [0u8; 2048];
    if let Some(about) = about(name, &mut text) {
        if found { let _ = writeln!(out, "PROGRAM {}:", name); }
        let _ = writeln!(out, "{}", about.trim_end());
        found = true;
    } else if BOOT_SERVICES.iter().chain(SERVICE_INSTANCES.iter()).any(|s| s.eq_ignore_ascii_case(name)) {
        let _ = writeln!(out, "{} — a service init starts at boot: svc shows its state, top what it uses.", name);
        found = true;
    }
    if !found { let _ = writeln!(out, "ERROR: NO COMMAND OR PROGRAM CALLED {}. HELP: THE SHELL'S COMMANDS. LIST: THE PROGRAMS.", name); }
}

/// `<program> --help` for a program with a screen: what it would print goes to its own screen and COM1, so the shell
/// shows the same text from its file instead of starting it. False if it has none.
pub fn show_about(out: &mut Console, name: &str) -> bool {
    let mut text = [0u8; 2048];
    match about(name, &mut text) { Some(about) => { let _ = writeln!(out, "{}", about.trim_end()); true } None => false }
}

/// `list` (programs in columns) or `list -l` (one per line with what it does); `masks` (`a*`, `*mon*`, several
/// separated by spaces or commas; empty: all) keeps the programs and services whose names match.
pub fn list(out: &mut Console, long: bool, masks: &str) {
    let wanted = |name: &str| masks.is_empty() || mind::mask::matches(masks, name);
    let programs = match loader::list(Endpoint::LOADER) { Ok(programs) => programs, Err(_) => { let _ = writeln!(out, "ERROR: CANNOT LIST THE BOOT DISK"); return; } };
    // Applications only, sorted by name (services are listed below).
    let mut names: [([u8; NAME_MAX], usize, u64); 64] = [([0; NAME_MAX], 0, 0); 64];
    let mut count = 0;
    for program in programs.as_slice().iter().filter(|p| !p.service && wanted(p.name.as_str())) {
        let name = program.name.as_str().as_bytes();
        let len = name.len().min(NAME_MAX);
        if count < names.len() { names[count].0[..len].copy_from_slice(&name[..len]); names[count].1 = len; names[count].2 = program.size; count += 1; }
    }
    let names = &mut names[..count];
    names.sort_unstable_by(|a, b| a.0[..a.1].cmp(&b.0[..b.1]));

    if masks.is_empty() {
        let _ = writeln!(out, "PROGRAMS ON DISK ({}): RUN <NAME> [ARGS] [&], OR JUST <NAME> [ARGS].{}", count, if long { "" } else { " LIST -L: WHAT EACH ONE DOES." });
    } else {
        let _ = writeln!(out, "PROGRAMS ON DISK MATCHING {} ({}){}", masks, count, if count == 0 { "." } else { ":" });
    }
    if long {
        let mut text = [0u8; 2048];
        for entry in names.iter() { let _ = writeln!(out, "  {:<12} {:>5} KB  {}", name(entry), entry.2.div_ceil(1024), summary(name(entry), &mut text)); }
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
    let mut services = BOOT_SERVICES.iter().chain(SERVICE_INSTANCES.iter()).filter(|s| wanted(s)).peekable();
    if services.peek().is_some() {
        let _ = write!(out, "SERVICES (STARTED AT BOOT; SVC SHOWS THEIR STATE):");
        for service in services { let _ = write!(out, " {}", service); }
        let _ = writeln!(out);
    }
    if masks.is_empty() { let _ = writeln!(out, "CTRL+Z: BACK TO THE SHELL, THE PROGRAM KEEPS RUNNING. ESC: EXIT A PROGRAM. HELP: THE SHELL'S COMMANDS."); }
}
