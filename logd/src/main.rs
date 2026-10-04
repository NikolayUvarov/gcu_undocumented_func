#![no_std]
#![no_main]
// logd: the system log (idl/log.wit, MC-10.6). Records go into a ring of 256 (ring.rs) stamped with the time of
// arrival and with the sender's PID from IPC and its task name from the kernel's task records (the observe
// privilege): a message cannot name its own source. Every client may write; reading needs the read badge
// (`mind::log::BADGE_READ`), which init gives to the shell's client only.
mod ring;

use mind::abi::*;
use mind::idl::codec::Text;
use mind::idl::{log, wire};
use mind::ipc::Endpoint;
use mind::mem::Pages;
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
                if let Some(task) = records.iter::<StatTask>().find(|t| t.pid == pid) { name.copy_from_slice(&task.name[..NAME]); }
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
    let own = names.scratch.as_mut().and_then(|s| stat::read(STAT_TASKS, 0, s.as_mut_slice()).ok().and_then(|r| r.iter::<StatTask>().find(|t| text(&t.name[..NAME].try_into().unwrap()) == "logd").map(|t| t.pid))).unwrap_or(0);
    let now = || mind::time::uptime_ms() as u64;
    ring.push(now(), own, "logd", 1, if observe { "[LOGD] READY: 256 RECORDS OF 200 BYTES" } else { "[LOGD] NO OBSERVE PRIVILEGE: SOURCES ARE PIDS ONLY" });
    mind::println!("[LOGD] READY");
    loop {
        let Ok(request) = Endpoint::SERVICE.recv(RECEIVED) else { continue };
        let reader = request.badge & mind::log::BADGE_READ != 0;
        let _ = match log::decode(&request, RECEIVED) {
            Ok((log::Request::Write { level, text: line }, call)) => {
                let name = names.get(request.sender);
                ring.push(now(), request.sender, text(&name), level, line.as_str());
                log::reply_write(call, Ok(()))
            }
            Ok((log::Request::Read { from }, call)) if reader => {
                // At most 16 records from `from` on (from the oldest kept if that is later).
                let mut entries = [log::Entry::default(); 16];
                let mut count = 0;
                for seq in from.max(ring.first())..ring.next() {
                    let Some(r) = ring.get(seq) else { break };
                    if count == entries.len() { break; }
                    entries[count] = log::Entry { seq: r.seq, time_ms: r.time_ms, pid: r.pid, name: Text::new(r.name()).unwrap_or_default(), level: r.level, text: Text::new(r.text()).unwrap_or_default() };
                    count += 1;
                }
                log::reply_read(call, Ok(&entries[..count]))
            }
            Ok((log::Request::State, call)) if reader => log::reply_state(call, Ok(&log::State { first: ring.first(), next: ring.next(), dropped: ring.dropped(), suppressed: ring.suppressed() })),
            Ok((log::Request::Read { .. }, call)) => log::reply_read(call, Err(log::Error::Denied)),
            Ok((log::Request::State, call)) => log::reply_state(call, Err(log::Error::Denied)),
            Err(reason) => if request.is_call { wire::reject(reason) } else { Ok(()) },
        };
    }
}
