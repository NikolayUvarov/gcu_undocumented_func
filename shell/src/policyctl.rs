//! `effector policy`'s files (801-APP-0140): data/effector/policy read, edited by `policyedit.rs` and written so that a
//! lost write never leaves the machine without a policy that parses: the new file whole and read back, the old one
//! renamed aside, the new one given its name, the old one removed. `recover` finishes or undoes an edit a restart cut.
use crate::policyedit::{self, Asked, DIR, FILE, NEW, OLD};
use alloc::string::String;
use core::fmt::Write;
use mind::effector::policy::{Policy, FILE_MAX};
use mind::fs::{self, Error, File};

// The parser the gateway and init use: the rules' count, or the line refused and why.
fn check(text: &str) -> Result<u16, (u16, &'static str)> {
    Policy::parse(text.as_bytes()).map(|p| p.rules()).map_err(|e| (e.line, policyedit::why(&alloc::format!("{:?}", e.kind))))
}

// A file's text, None when there is none; one past FILE_MAX is read, so that a larger file is refused as the gateway does.
fn read(path: &str) -> Result<Option<String>, Error> {
    let file = match File::open(path) { Ok(file) => file, Err(Error::NotFound) => return Ok(None), Err(e) => return Err(e) };
    let mut bytes = alloc::vec![0u8; file.size().min(FILE_MAX + 1)];
    let mut at = 0;
    while at < bytes.len() {
        match file.read_at(at, &mut bytes[at..])? { 0 => break, n => at += n }
    }
    bytes.truncate(at);
    Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
}

fn gone(path: &str) -> Result<(), Error> { match fs::remove(path) { Err(Error::NotFound) => Ok(()), other => other } }

// The new policy in place of the old: written whole and read back under NEW, the old renamed to OLD, NEW renamed to
// FILE, OLD removed.
fn write(text: &str) -> Result<(), Error> {
    fs::mkdir(DIR)?;
    gone(NEW)?;
    let mut file = File::create(NEW)?;
    let mut at = 0;
    while at < text.len() { at += file.write(&text.as_bytes()[at..])?; }
    file.flush()?;
    drop(file);
    if read(NEW)?.as_deref() != Some(text) { let _ = gone(NEW); return Err(Error::Io); }
    gone(OLD)?;
    match fs::rename(FILE, OLD) { Ok(()) | Err(Error::NotFound) => {} Err(e) => return Err(e) }
    fs::rename(NEW, FILE)?;
    let _ = gone(OLD);
    Ok(())
}

// `effector policy [show | allow <line> | deny <line>]`.
pub fn command(shell: &mut crate::Shell, args: &[u8]) {
    let asked = match policyedit::parse(core::str::from_utf8(args).unwrap_or("")) { Ok(asked) => asked, Err(usage) => { let _ = writeln!(shell.term, "{}", usage); return } };
    let current = match read(FILE) { Ok(text) => text, Err(e) => { let _ = writeln!(shell.term, "EFFECTOR POLICY: {} NOT READ: {}", FILE, crate::files::text(e)); return } };
    if asked == Asked::Show {
        if let Some(text) = &current { for line in text.lines() { let _ = writeln!(shell.term, "  {}", line); } }
        let _ = writeln!(shell.term, "{}", policyedit::state(current.is_some(), current.as_deref().map_or(Ok(0), check)));
        return;
    }
    // Only at this console: no one at the link, an SSH session or a lent client agrees to a policy for this machine.
    if shell.unasked { let _ = writeln!(shell.term, "EFFECTOR POLICY: NOT CHANGED: A LINE TYPED ELSEWHERE MAY NOT CHANGE THE POLICY"); return; }
    let (text, change, rules) = match policyedit::edit(current.as_deref(), asked, check) { Ok(edited) => edited, Err(why) => { let _ = writeln!(shell.term, "{}", why); return } };
    if let (Asked::Allow(_), policyedit::Change::Added(rule) | policyedit::Change::Replaced { new: rule, .. }) = (asked, &change) {
        if !crate::msh::ask(shell, &policyedit::question(rule)) { let _ = writeln!(shell.term, "EFFECTOR POLICY: NOT CHANGED"); return; }
    }
    match write(&text) {
        Ok(()) => {
            let line = policyedit::done(&change, rules, None);
            let _ = writeln!(shell.term, "{}", line);
            mind::println!("[SHELL] {}", line);
        }
        Err(e) => { let _ = writeln!(shell.term, "EFFECTOR POLICY: NOT CHANGED: {} NOT WRITTEN: {}", FILE, crate::files::text(e)); }
    }
}

// At the shell's start: an edit a restart cut is finished (the new file parses and the old name is free) or undone (the
// old file back), and what is left over is removed, each logged.
pub fn recover() {
    let exists = |path| matches!(File::open(path), Ok(_));
    if !exists(NEW) && !exists(OLD) { return; }
    if !exists(FILE) {
        let new_ok = matches!(read(NEW), Ok(Some(text)) if check(&text).is_ok());
        let (from, what) = if new_ok { (NEW, "THE NEW ONE TOOK ITS NAME") } else { (OLD, "THE OLD ONE WAS PUT BACK") };
        match fs::rename(from, FILE) {
            Ok(()) => mind::println!("[SHELL] EFFECTOR POLICY: AN EDIT WAS CUT BY A RESTART: {}", what),
            Err(e) => mind::println!("[SHELL] EFFECTOR POLICY: AN EDIT WAS CUT BY A RESTART, AND {} WAS NOT RENAMED: {}", from, crate::files::text(e)),
        }
    }
    for path in [NEW, OLD] { if exists(path) && gone(path).is_ok() { mind::println!("[SHELL] EFFECTOR POLICY: {} LEFT BY A CUT EDIT, REMOVED", path); } }
}
