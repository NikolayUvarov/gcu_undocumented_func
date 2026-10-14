//! What the shell takes from a client of its commands (`idl/shell.wit`, 211-APP-0044). At once only what system
//! information gives any program that asks for it, the files' flush and `help`. In its own window, after the user agrees
//! there, what changes the machine or reaches beyond it: process control, the network on the shell's badge, a task's
//! logs, address space and capabilities, a line in the system log in the shell's name; a script by its file (`fm`'s
//! Enter, 000-APP-0067), which runs there. Nothing that acts on the shell's own screen, no code typed for `msh -c`, no
//! statement and no program. Host-tested in tests/shell_host.rs.

/// Taken at once.
pub const NOW: [&str; 16] = ["ps", "cpus", "free", "faults", "quotas", "heap", "devices", "irqs", "endpoints", "physmap", "time", "ip", "net", "netgrants", "sync", "battery"];
/// Taken after the user agrees in the shell's window (`date` with `set`, `netpolicy` with a change, `caps` with a PID).
pub const ASKED: [&str; 19] = ["kill", "stop", "reboot", "poweroff", "shutdown", "budget", "netrevoke", "netpolicy", "date", "logs", "stat", "pmap", "caps", "logger",
    "ping", "nslookup", "fetch", "https", "tpm"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Taken { Refused, Now, Asked }

/// How the shell takes `line` from a client.
pub fn taken(line: &str) -> Taken {
    let line = line.trim();
    let (word, args) = line.split_once(char::is_whitespace).map_or((line, ""), |(w, a)| (w, a.trim()));
    let word = word.to_ascii_lowercase();
    let word = word.as_str();
    match word {
        "date" | "netpolicy" if args.is_empty() => Taken::Now,
        "help" => Taken::Now, // what a command or program does: the shell's lines and the program's file, read only
        "date" if !args.starts_with("set") => Taken::Refused,
        "caps" if args.is_empty() => Taken::Refused, // the caps tool, a program
        // A script by its file, in the shell's window once the user agrees (000-APP-0067); not `-c` code or options.
        "msh" if !args.is_empty() && !args.starts_with('-') && args.split_whitespace().count() == 1 => Taken::Asked,
        "msh" => Taken::Refused,
        _ if ASKED.contains(&word) => Taken::Asked,
        _ if NOW.contains(&word) => Taken::Now,
        _ => Taken::Refused,
    }
}
