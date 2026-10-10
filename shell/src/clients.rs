//! What the shell takes from a client of its commands (`idl/shell.wit`, 211-APP-0044). At once only what system
//! information gives any program that asks for it, the files' flush and `help`. In its own window, after the user agrees
//! there, what changes the machine or reaches beyond it: process control, the network on the shell's badge, a task's
//! logs, address space and capabilities, a line in the system log in the shell's name. Nothing that acts on the shell's
//! own screen, no script and no program. Host-tested in tests/shell_host.rs.

/// Taken at once.
pub const NOW: [&str; 15] = ["ps", "cpus", "free", "faults", "quotas", "heap", "devices", "irqs", "endpoints", "physmap", "time", "ip", "net", "netgrants", "sync"];
/// Taken after the user agrees in the shell's window (`date` with `set`, `netpolicy` with a change, `caps` with a PID).
pub const ASKED: [&str; 17] = ["kill", "stop", "reboot", "budget", "netrevoke", "netpolicy", "date", "logs", "stat", "pmap", "caps", "logger",
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
        _ if ASKED.contains(&word) => Taken::Asked,
        _ if NOW.contains(&word) => Taken::Now,
        _ => Taken::Refused,
    }
}
