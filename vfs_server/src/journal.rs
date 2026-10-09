// The system log on the log volume (211-KRN-0019): each boot's records from logd are appended to log:/bootNNNN.log
// and flushed every SAVE_MS, so a machine without a serial port leaves its log where any computer can read it.
use crate::fat::{Node, Sectors, Volume};
use alloc::format;
use alloc::string::String;
use core::fmt::Write;
use mind::idl::log;
use mind::ipc::Endpoint;

pub const SAVE_MS: u64 = 2000;
const KEEP: u32 = 50; // boot logs kept on the volume; older ones are removed
const CHUNK: usize = 16 * 1024; // written per save at most; the rest goes with the next one
const LOW: u64 = 4 * 1024 * 1024; // free space kept: older boot logs make room, then this one stops

pub struct Journal { pub name: String, next: u64, pub due: u64, full: bool }

// The number of a boot log's name (`boot0012.log`, a short name every FAT reader shows the same).
fn number(name: &str) -> Option<u32> {
    let lower = name.to_ascii_lowercase();
    lower.strip_prefix("boot")?.strip_suffix(".log")?.parse().ok()
}

impl Journal {
    /// This boot's file, numbered after the highest one there; logs older than the last KEEP are removed first.
    pub fn start<S: Sectors>(volume: &mut Volume<S>, header: &str, stamp: u32) -> Option<Self> {
        let root = volume.root();
        let names: alloc::vec::Vec<String> = volume.list(&root).ok()?.into_iter().map(|e| e.name).filter(|n| number(n).is_some()).collect();
        let this = names.iter().filter_map(|n| number(n)).max().unwrap_or(0) + 1;
        for name in names.iter().filter(|n| number(n).is_some_and(|k| k + KEEP <= this)) { let _ = volume.remove(&root, name); }
        let name = format!("boot{:04}.log", this);
        let mut node = volume.create(&root, &name, false, stamp).ok()?;
        volume.write(&mut node, 0, header.as_bytes(), stamp).ok()?;
        volume.flush().ok()?;
        Some(Self { name, next: 0, due: 0, full: false })
    }

    /// Appends the records logd has received since the last save, at the file's end as it is now; the file's node
    /// afterwards, for the handles open on it.
    pub fn save<S: Sectors>(&mut self, volume: &mut Volume<S>, logd: Endpoint, stamp: u32) -> Option<Node> {
        if self.full { return None; }
        let mut text = String::new();
        while text.len() < CHUNK {
            let Ok(Ok(list)) = log::read(logd, self.next) else { break };
            if list.is_empty() { break; }
            for entry in list.as_slice() {
                if entry.seq > self.next { let _ = writeln!(text, "... {} RECORDS LOST (THE LOG'S RING WAS FULL)", entry.seq - self.next); }
                let _ = writeln!(text, "[{:5}.{:03}] {}({}) {}", entry.time_ms / 1000, entry.time_ms % 1000, entry.name, entry.pid, entry.text);
                self.next = entry.seq + 1;
            }
        }
        if text.is_empty() { return None; }
        if !self.room(volume, text.len()) {
            self.full = true;
            text = String::from("... THE LOG VOLUME IS FULL: THE REST OF THIS BOOT'S LOG IS NOT KEPT\n");
        }
        // Found again each time: a user may have removed or truncated it meanwhile.
        let root = volume.root();
        let mut node = match volume.find(&root, &self.name) { Ok(entry) => entry.node, Err(_) => volume.create(&root, &self.name, false, stamp).ok()? };
        let end = node.size;
        volume.write(&mut node, end, text.as_bytes(), stamp).ok()?;
        volume.flush().ok()?;
        Some(node)
    }

    // Whether `bytes` more leave LOW free, after removing the oldest boot logs but this one as needed.
    fn room<S: Sectors>(&self, volume: &mut Volume<S>, bytes: usize) -> bool {
        loop {
            let Ok(free) = volume.free_clusters() else { return false };
            if free as u64 * volume.cluster_bytes() as u64 >= LOW + bytes as u64 { return true; }
            let root = volume.root();
            let Ok(entries) = volume.list(&root) else { return false };
            let Some(oldest) = entries.into_iter().map(|e| e.name).filter(|n| *n != self.name).min_by_key(|n| number(n).unwrap_or(u32::MAX)).filter(|n| number(n).is_some()) else { return false };
            if volume.remove(&root, &oldest).is_err() { return false; }
        }
    }
}
