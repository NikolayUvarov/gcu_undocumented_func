//! What the shell takes from a client of its commands (`idl/shell.wit`, 211-APP-0044). At once only what system
//! information gives any program that asks for it, the files' flush and `help`. In its own window, after the user agrees
//! there, what changes the machine or reaches beyond it: process control, the network on the shell's badge, a task's
//! logs, address space and capabilities, a line in the system log in the shell's name; a script by its file (`fm`'s
//! Enter, 000-APP-0067), which runs there; a command whose output goes into a file (`>`, 000-APP-0074). Nothing that acts on the shell's own screen, no code typed for `msh -c`, no
//! statement and no program. The link's client in developer mode (800-APP-0102, for 800-LNK-0015) has no one at the
//! shell's window to agree: it gets at once what reads and, by the maintainer's choice of 2026-10-22, what changes the
//! machine, but never a change of the network policy (800-NET-0038), the machine's modes or a script. In the owner's
//! `link-console` mode (`init.wit` 1.8, 800-APP-0108) the link's line is the console's: scripts by their file and the
//! policy's changes run too, never `mode`. An SSH session's line (112-APP-0136) is never asked on the machine's screen:
//! what the link runs at once, but for a file written by `>` (the key's `files` right's, through SFTP) and the TPM.
//! Host-tested in tests/shell_host.rs.

/// Taken at once.
pub const NOW: [&str; 18] = ["ps", "cpus", "free", "faults", "quotas", "heap", "devices", "irqs", "endpoints", "physmap", "time", "ip", "net", "netgrants", "sync", "battery",
    "volume", "mic"]; // the levels change no authority, as wm's Sound page sets them (000-APP-0076, 000-APP-0077)
/// Taken after the user agrees in the shell's window (`date` with `set`, `netpolicy` with a change, `caps` with a PID,
/// `hostname` with a name).
pub const ASKED: [&str; 21] = ["kill", "stop", "reboot", "poweroff", "shutdown", "budget", "netrevoke", "netpolicy", "date", "logs", "stat", "pmap", "caps", "logger",
    "ping", "nslookup", "fetch", "https", "tpm", "netstat", "hostname"];

/// Of `ASKED`, never taken from the link's client outside the `link-console` mode: the network policy's changes
/// (800-NET-0038). `mode` is refused to every client, the link's console too.
pub const NOT_OVER_THE_LINK: [&str; 2] = ["netrevoke", "netpolicy"];

/// Of what the link takes at once, refused to an SSH session as well: the TPM, the machine's own root of trust.
pub const NOT_REMOTE: [&str; 1] = ["tpm"];

/// The badge of the client `init` mints for `sshd` (112-NET-0072's `mind::process::BADGE_SHELL_REMOTE`, not in `main` yet).
pub const BADGE_REMOTE: u16 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Taken { Refused, Now, Asked }

/// Who calls: a program the shell lent its commands to, `link` in developer mode (`mind::process::BADGE_SHELL_LINK`),
/// `link` while init reports the owner's `link-console` mode (800-APP-0108), or `sshd` for a login's console (`BADGE_REMOTE`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Client { Program, Link, LinkConsole, Remote }

/// How the shell takes `line` from `client`: for the link, what a program's client is asked runs at once, but for the
/// policy's changes and scripts, which are refused; for the link's console those run at once too; for an SSH session as
/// for the link, but for a file written and the TPM.
pub fn taken_by(line: &str, client: Client) -> Taken {
    let line = line.trim();
    let asked = if client == Client::Program { Taken::Asked } else { Taken::Now };
    // `>` writes a file in the shell's name (000-APP-0074): asked, for a command taken at all.
    match crate::redirect::split(line) {
        Ok(Some(_)) if client == Client::Remote => return Taken::Refused,
        Ok(Some(split)) => return if taken_by(split.command, client) == Taken::Refused { Taken::Refused } else { asked },
        Err(_) => return Taken::Refused,
        Ok(None) => {}
    }
    let (word, args) = line.split_once(char::is_whitespace).map_or((line, ""), |(w, a)| (w, a.trim()));
    let word = word.to_ascii_lowercase();
    let word = word.as_str();
    match word {
        "date" | "netpolicy" | "hostname" if args.is_empty() => Taken::Now, // `hostname <name>` sets the machine's name (000-APP-0123)
        "help" => Taken::Now, // what a command or program does: the shell's lines and the program's file, read only
        "date" if !args.starts_with("set") => Taken::Refused,
        "caps" if args.is_empty() => Taken::Refused, // the caps tool, a program
        // A script by its file, in the shell's window once the user agrees (000-APP-0067), at once for the link's console;
        // not `-c` code or options.
        "msh" if !matches!(client, Client::Link | Client::Remote) && !args.is_empty() && !args.starts_with('-') && args.split_whitespace().count() == 1 => asked,
        "msh" => Taken::Refused,
        _ if matches!(client, Client::Link | Client::Remote) && NOT_OVER_THE_LINK.contains(&word) => Taken::Refused,
        _ if client == Client::Remote && NOT_REMOTE.contains(&word) => Taken::Refused,
        _ if ASKED.contains(&word) => asked,
        _ if NOW.contains(&word) => Taken::Now,
        _ => Taken::Refused,
    }
}
