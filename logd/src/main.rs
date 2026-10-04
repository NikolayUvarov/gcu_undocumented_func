#![no_std]
#![no_main]
// logd: the system log (idl/log.wit, MC-10.6). Records go into a ring of 256 (ring.rs) stamped with the time of
// arrival and with the sender's PID from IPC and its task name from the kernel's task records (the observe
// privilege): a message cannot name its own source. Every client may write; reading needs the read badge
// (LOG_BADGE_READ), which init gives to the shell's client only.
mod ring;

use mind::abi::*;
use mind::idl::{log, wire};
use mind::ipc::{self, Endpoint};
use mind::mem::{Mapping, Pages};
use mind::stat;
use ring::{Ring, NAME};

const RECEIVED: usize = 9;
const CACHE: usize = 32;

static mut RING: Ring = Ring::new();

// Task names by PID, from STAT_TASKS (PIDs are never reused, so a cached name stays right).
struct Names { cache: [(u64, [u8; NAME]); CACHE], next: usize, scratch: Option<Pages> }

impl Names {
    fn get(&mut self, pid: u64) -> [u8; NAME] {
        if let Some((_, name)) = self.cache.iter().find(|(p, _)| *p == pid && pid != 0) { return *name; }
        let mut name = *b"?\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";
        if let Some(scratch) = self.scratch.as_mut() {
            if let Ok(records) = stat::read(STAT_TASKS, 0, scratch.as_mut_slice()) {
                if let Some(task) = records.iter::<TaskStat>().find(|t| t.pid == pid) { name.copy_from_slice(&task.name[..NAME]); }
            }
        }
        self.cache[self.next] = (pid, name); self.next = (self.next + 1) % CACHE;
        name
    }
}

fn text(name: &[u8; NAME]) -> &str { let end = name.iter().position(|&b| b == 0).unwrap_or(NAME); core::str::from_utf8(&name[..end]).unwrap_or("?") }

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let ring = unsafe { &mut *core::ptr::addr_of_mut!(RING) };
    let mut names = Names { cache: [(0, [0; NAME]); CACHE], next: 0, scratch: Pages::new(64 * 1024) };
    let observe = names.scratch.as_mut().is_some_and(|s| stat::read(STAT_TASKS, 0, s.as_mut_slice()).is_ok());
    let own = names.scratch.as_mut().and_then(|s| stat::read(STAT_TASKS, 0, s.as_mut_slice()).ok().and_then(|r| r.iter::<TaskStat>().find(|t| text(&t.name[..NAME].try_into().unwrap()) == "logd").map(|t| t.pid))).unwrap_or(0);
    let now = || mind::time::uptime_ms() as u64;
    ring.push(now(), own, "logd", 1, if observe { "[LOGD] READY: 256 RECORDS OF 200 BYTES" } else { "[LOGD] NO OBSERVE PRIVILEGE: SOURCES ARE PIDS ONLY" });
    mind::println!("[LOGD] READY");
    loop {
        let Ok(request) = Endpoint::SERVICE.recv(RECEIVED) else { continue };
        let decoded = log::decode(&request, RECEIVED);
        let mut mapping = if request.cap_received { Mapping::new(RECEIVED).ok() } else { None };
        let mut empty = [0u8; 0];
        let bytes: &mut [u8] = match mapping.as_mut() { Some(m) => m.as_mut_slice(), None => &mut empty };
        let reader = request.badge & LOG_BADGE_READ != 0;
        let _ = match decoded {
            Ok(log::Request::Write { payload, level, .. }) => match log::args_write(bytes, payload) {
                Ok(line) => {
                    let name = names.get(request.sender);
                    ring.push(now(), request.sender, text(&name), level, line);
                    log::reply_write(Ok(()))
                }
                Err(reason) => wire::reject(reason),
            },
            Ok(log::Request::Read { from, .. }) if reader => {
                // As many records as the buffer holds: each takes its strings plus about 40 bytes.
                let start = from.max(ring.first());
                let mut entries: [Option<log::Entry>; 64] = [None; 64];
                let (mut count, mut size) = (0, 16);
                for seq in start..ring.next() {
                    let Some(r) = ring.get(seq) else { break };
                    let need = 48 + r.name().len() + r.text().len();
                    if count == entries.len() || size + need > bytes.len() { break; }
                    entries[count] = Some(log::Entry { seq: r.seq, time_ms: r.time_ms, pid: r.pid, name: r.name(), level: r.level, text: r.text() });
                    count += 1; size += need;
                }
                let list: [log::Entry; 64] = core::array::from_fn(|i| entries[i].unwrap_or(log::Entry { seq: 0, time_ms: 0, pid: 0, name: "", level: 0, text: "" }));
                log::reply_read(bytes, Ok(&list[..count]))
            }
            Ok(log::Request::State { .. }) if reader => log::reply_state(bytes, Ok(log::State { first: ring.first(), next: ring.next(), dropped: ring.dropped(), suppressed: ring.suppressed() })),
            Ok(log::Request::Read { .. }) => log::reply_read(bytes, Err(log::Error::Denied)),
            Ok(log::Request::State { .. }) => log::reply_state(bytes, Err(log::Error::Denied)),
            Err(reason) => wire::reject(reason),
        };
        drop(mapping);
        if request.cap_received { let _ = ipc::drop_cap(RECEIVED); }
    }
}
