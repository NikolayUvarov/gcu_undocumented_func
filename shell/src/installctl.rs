//! The shell's `install` (352-APP-0157, for 352-UPD-0048): on a live boot, MIND Core from the live medium onto a disk of
//! this machine, through vfs_server's installer (`install.wit` 1.0) in SLOT_INSTALL, which init mints on a live boot
//! alone. Installing erases the disk, so its words are typed back at this machine's own console, never by a remote
//! session or the link (MC-12.3); a disk that lists partitions is asked about a second time.

use crate::flowctl::say;
use crate::installer::{self, Asked, Disk};
use crate::Shell;
use alloc::format;
use alloc::string::String;
use core::fmt::Write;
use mind::idl::install;
use mind::input::Code;
use mind::ipc::Endpoint;

const INSTALLER: Endpoint = Endpoint(mind::abi::SLOT_INSTALL);
/// How long the disk's words may take to type.
const TYPE_MS: usize = 120_000;
/// The longest words the installer takes (`string<32>`).
const WORDS_MAX: usize = 32;

fn view(d: &install::Disk) -> Disk<'_> {
    Disk { number: d.number, bus: d.bus as usize, model: d.model.as_str(), mib: d.mib, signature: d.signature.as_str(), partitions: d.partitions, words: d.words.as_str(), refusal: d.refusal as usize }
}

fn say_and_log(shell: &mut Shell, line: &str) { let _ = writeln!(shell.term, "{}", line); mind::println!("[SHELL] {}", line); }

/// `install [N]`.
pub fn command(shell: &mut Shell, args: &[u8]) {
    let asked = match installer::parse(core::str::from_utf8(args).unwrap_or("")) { Ok(asked) => asked, Err(usage) => { let _ = writeln!(shell.term, "{}", usage); return } };
    if mind::dev::cap_info(mind::abi::SLOT_INSTALL).0 != mind::abi::CAP_KIND_ENDPOINT {
        let _ = writeln!(shell.term, "INSTALL: THIS IS NOT A LIVE BOOT: THE SHELL HOLDS NO CLIENT OF THE INSTALLER (SLOT_INSTALL)");
        return;
    }
    let disks = match install::disks(INSTALLER) {
        Ok(Ok(list)) => list,
        Ok(Err(error)) => { let _ = writeln!(shell.term, "{}", installer::error(error as usize)); return }
        Err(_) => { let _ = writeln!(shell.term, "INSTALL: THE INSTALLER DOES NOT ANSWER"); return }
    };
    let n = match asked {
        Asked::List => {
            if disks.as_slice().is_empty() { let _ = writeln!(shell.term, "INSTALL: THE INSTALLER LISTS NO DISK"); return; }
            let _ = writeln!(shell.term, "INSTALL: THE DISKS OF THIS MACHINE (INSTALL N WRITES MIND CORE ONTO DISK N, ERASING IT)");
            for d in disks.as_slice() { let _ = writeln!(shell.term, "{}", installer::line(&view(d))); }
            return;
        }
        Asked::Disk(n) => n,
    };
    let Some(d) = disks.as_slice().iter().find(|d| d.number == n) else { let _ = writeln!(shell.term, "{}", installer::error(1)); return };
    let disk = view(d);
    let _ = writeln!(shell.term, "{}", installer::shown(&disk));
    if let Some(why) = installer::refusal(disk.refusal) { let _ = writeln!(shell.term, "INSTALL: NOT INSTALLED: DISK {n} IS REFUSED: {why}"); return; }
    // A person at this machine erases its disk: no line of the link, of its console or of an SSH session does.
    if shell.unasked || shell.console_link {
        say_and_log(shell, &format!("INSTALL: NOT ASKED: INSTALLING ERASES DISK {n}, SO ITS WORDS ARE TYPED AT THIS MACHINE'S OWN CONSOLE, NOT IN A REMOTE SESSION OR BY THE LINK"));
        return;
    }
    let Some(typed) = read_line(shell, &format!("TO ERASE DISK {n} AND INSTALL, TYPE ITS WORDS: {} (ESC: NO) ", disk.words)) else {
        let _ = writeln!(shell.term, "INSTALL: NOT INSTALLED");
        return;
    };
    let erase = disk.partitions > 0;
    if erase && !crate::msh::ask(shell, &installer::erase_question(&disk)) { let _ = writeln!(shell.term, "INSTALL: NOT INSTALLED"); return; }
    say_and_log(shell, &format!("INSTALL: WRITING MIND CORE ONTO DISK {n}: A MINUTE OR MORE, AND THE FILE SERVICE ANSWERS NOTHING ELSE MEANWHILE"));
    shell.term.render(None);
    match install::install(INSTALLER, n, &typed, erase) {
        Ok(Ok(l)) => say_and_log(shell, &installer::layout(n, l.signature, [(l.esp_first, l.esp_sectors), (l.log_first, l.log_sectors), (l.store_first, l.store_sectors)])),
        Ok(Err(error)) => say_and_log(shell, &installer::error(error as usize)),
        Err(_) => say_and_log(shell, "INSTALL: THE INSTALLER DID NOT ANSWER: THE DISK MAY HAVE NO ESP: INSTALL AGAIN"),
    }
}

// What a key or a byte of the serial line does to the line being typed.
enum Got { Enter, Esc, Back, Char(char) }

// A line typed at this console, shown as it is typed: Enter ends it; Esc, or nothing for TYPE_MS, gives None.
fn read_line(shell: &mut Shell, prompt: &str) -> Option<String> {
    let _ = write!(shell.term, "{}", prompt);
    shell.term.render(None);
    let start = shell.term.position();
    let mut line = String::new();
    let until = mind::time::uptime_ms() + TYPE_MS;
    while mind::time::uptime_ms() < until {
        let got = shell.term.serial.as_ref().and_then(mind::dev::Uart::read).map(|b| match b {
            b'\r' | b'\n' => Got::Enter,
            0x1b => Got::Esc,
            0x08 | 0x7f => Got::Back,
            b => Got::Char(b as char),
        }).or_else(|| shell.next_key().and_then(|k| match k.code() {
            Code::Enter => Some(Got::Enter),
            Code::Esc => Some(Got::Esc),
            Code::Backspace => Some(Got::Back),
            _ => k.char().map(Got::Char),
        }));
        match got {
            Some(Got::Enter) => { let _ = writeln!(shell.term); return Some(line); }
            Some(Got::Esc) => { let _ = writeln!(shell.term, "(ESC)"); return None; }
            Some(Got::Back) => {
                if line.pop().is_some() {
                    shell.term.truncate(start);
                    let shown = line.clone();
                    say(shell, |term| { term.put_str(&shown); term.serial_str("\u{8} \u{8}"); });
                    shell.term.render(None);
                }
            }
            Some(Got::Char(c)) if (c.is_ascii_graphic() || c == ' ') && line.len() < WORDS_MAX => {
                line.push(c);
                shell.term.print_char(c as u8);
                shell.term.render(None);
            }
            _ => { mind::time::sleep(20); }
        }
    }
    let _ = writeln!(shell.term, "(NO ANSWER)");
    None
}
