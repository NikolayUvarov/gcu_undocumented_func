//! The journal: one event a line, appended and synced before the event is applied, and read back at start (MC-10.3).
//! A last line cut by a crash before its sync was never answered, so it is dropped and said so; any other line that is
//! not an event stops the start, naming it, rather than be repaired.

use crate::event::{Event, Redaction};
use crate::util::hex;
use std::collections::HashSet;
use crate::hub::Hub;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

pub struct Journal { file: File, path: PathBuf }

impl Journal {
    /// Opens the journal at `path`, making it if there is none, and applies its events to `hub`.
    pub fn open(path: &Path, hub: &mut Hub) -> Result<Journal, String> {
        let name = path.display();
        let mut file = OpenOptions::new().read(true).append(true).create(true).open(path).map_err(|e| format!("{name}: {e}"))?;
        let mut text = Vec::new();
        file.read_to_end(&mut text).map_err(|e| format!("{name}: {e}"))?;
        let whole = text.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
        if whole < text.len() {
            eprintln!("MINDHUB: THE JOURNAL'S LAST LINE WAS CUT ({} BYTES), DROPPED: IT WAS NEVER ANSWERED", text.len() - whole);
            file.set_len(whole as u64).map_err(|e| format!("{name}: {e}"))?;
            file.sync_all().map_err(|e| format!("{name}: {e}"))?;
        }
        let text = std::str::from_utf8(&text[..whole]).map_err(|e| format!("{name}: not UTF-8: {e}"))?;
        for (n, line) in text.lines().enumerate() {
            let event = Event::decode(line).map_err(|why| format!("{name}, line {}: {why}", n + 1))?;
            hub.apply(&event).map_err(|why| format!("{name}, line {}: {why}", n + 1))?;
        }
        Ok(Journal { file, path: path.to_path_buf() })
    }

    pub fn path(&self) -> &Path { &self.path }

    /// The journal written again with each event as `f` gives it, in a file of its own that then takes the journal's
    /// place, so that a crash leaves the old one or the new one whole. Deleted texts leave it this way (design 9).
    pub fn rewrite(&mut self, f: impl Fn(&Event) -> Event) -> io::Result<bool> {
        let text = std::fs::read_to_string(&self.path)?;
        let mut out = String::with_capacity(text.len());
        for line in text.lines() {
            let e = Event::decode(line).map_err(io::Error::other)?;
            out.push_str(&f(&e).encode());
            out.push('\n');
        }
        if out == text { return Ok(false); }
        let tmp = self.path.with_extension("jsonl.rewrite");
        {
            let mut t = File::create(&tmp)?;
            t.write_all(out.as_bytes())?;
            t.sync_all()?;
        }
        std::fs::rename(&tmp, &self.path)?;
        if let Some(dir) = self.path.parent().filter(|d| !d.as_os_str().is_empty()) { File::open(dir)?.sync_all()?; }
        self.file = OpenOptions::new().read(true).append(true).open(&self.path)?;
        Ok(true)
    }

    /// The journal's last `count` lines from line `from` on (1 is the first), for the admin.
    pub fn lines(&self, from: u64, count: usize) -> io::Result<Vec<(u64, String)>> {
        let text = std::fs::read_to_string(&self.path)?;
        Ok(text.lines().enumerate().map(|(i, l)| (i as u64 + 1, l.to_string())).skip(from.saturating_sub(1) as usize).take(count).collect())
    }
}

/// Events written, then applied: the one way the running hub changes (MC-10.3). Each is checked against the state
/// before it is written, so the journal never holds one the state did not take; one that does not fit is a fault of
/// the command that made it, and the events after it are not written. All are synced before any is answered;
/// a journal that cannot be written stops the hub, as the state would run ahead of it.
pub fn commit(hub: &mut Hub, journal: &mut Journal, events: Vec<Event>) -> io::Result<()> {
    if events.is_empty() { return Ok(()); }
    let mut fault = None;
    for e in &events {
        if let Err(why) = hub.check(e) { fault = Some(why); break; }
        let mut line = e.encode();
        line.push('\n');
        if let Err(e) = journal.file.write_all(line.as_bytes()) { stop(&journal.path, e); }
        hub.apply(e).expect("an event checked");
    }
    if let Err(e) = journal.file.sync_data() { stop(&journal.path, e); }
    match fault { Some(why) => Err(io::Error::other(format!("an event that does not fit the state: {why}"))), None => Ok(()) }
}

fn stop(path: &Path, e: io::Error) -> ! {
    eprintln!("MINDHUB: THE JOURNAL {} CANNOT BE WRITTEN ({e}): THE HUB STOPS, AND ITS NEXT START DROPS A CUT LAST LINE", path.display());
    std::process::exit(70)
}

/// The deleted requests' and erased users' texts out of the journal, and the attachments no request keeps any more out
/// of the store (design 9). The journal then says what the state says. A start does it too, in case a crash came
/// between a deletion and its erasure.
pub fn erase(hub: &Hub, journal: &mut Journal, store: &Path, now: u64) -> io::Result<()> {
    let requests: HashSet<u64> = hub.requests().iter().filter(|r| r.deleted).map(|r| r.id).collect();
    let users: HashSet<u64> = hub.users().iter().filter(|u| u.erased).map(|u| u.id).collect();
    if requests.is_empty() && users.is_empty() { return Ok(()); }
    let devices: HashSet<[u8; 32]> = hub.users().iter().filter(|u| u.erased).flat_map(|u| u.devices.iter().copied()).collect();
    journal.rewrite(|e| e.redacted(&Redaction { requests: &requests, users: &users, devices: &devices, now }))?;
    let named = |deleted: bool| -> HashSet<String> {
        hub.requests().iter().filter(|r| r.deleted == deleted)
            .flat_map(|r| r.form.attachments.iter().chain(r.thread.iter().flat_map(|p| p.message.attachments.iter())))
            .map(|a| hex(&a.sha256)).collect()
    };
    let kept = named(false);
    for h in named(true).difference(&kept) {
        match std::fs::remove_file(store.join(h)) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
    }
    Ok(())
}
