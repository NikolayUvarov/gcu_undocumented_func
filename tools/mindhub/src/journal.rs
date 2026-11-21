//! The journal: one event a line, appended and synced before the event is applied, and read back at start (MC-10.3).
//! A last line cut by a crash before its sync was never answered, so it is dropped and said so; any other line that is
//! not an event stops the start, naming it, rather than be repaired.

use crate::event::Event;
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
