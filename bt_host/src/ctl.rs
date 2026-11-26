//! The controller as the stack sees it: commands wait for its credit (Num_HCI_Command_Packets), ACL packets for its
//! buffers (Core 5.3 Vol 4 Part E 4.1: host to controller data flow control, the Number Of Completed Packets event).
//! LE has its own buffers unless the controller says 0 for them (LE Read Buffer Size), then it shares BR/EDR's.
use crate::acl;
use crate::buf::{Log, Queue};

pub const LINKS: usize = 8;
pub const OUT_BYTES: usize = 16384;
/// How long a command may stay unanswered before its credit is given back and it is called lost.
pub const COMMAND_MS: u64 = 3_000;

/// A buffer pool: the longest ACL data the controller takes, how many packets it holds, how many are free.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pool { pub len: u16, pub total: u16, pub free: u16 }

/// A link the controller holds packets for: its handle, whether LE, how many of its packets the controller holds.
#[derive(Clone, Copy, Debug, Default)]
struct Link { handle: u16, le: bool, held: u16, live: bool }

pub struct Ctl {
    /// Commands the controller takes now.
    pub credits: u8,
    /// The command sent and not yet answered, and when it went.
    pub waiting: Option<(u16, u64)>,
    cmds: Queue<4096>,
    pub bredr: Pool,
    pub le: Pool,
    /// Queued ACL packets, BR/EDR's then LE's.
    out: [Queue<OUT_BYTES>; 2],
    links: [Link; LINKS],
    pub log: Log,
    pub now: u64,
    pub lost_commands: u32,
    /// For the stream's line (700-BLT-0007): ACL packets given the controller, packets and events of Number Of Completed
    /// Packets, frames not queued for want of room.
    pub acl_sent: u32,
    pub completed_packets: u32,
    pub completed_events: u32,
    pub not_queued: u32,
}

impl Default for Ctl { fn default() -> Self { Self::new() } }

impl Ctl {
    pub const fn new() -> Self {
        Self { credits: 1, waiting: None, cmds: Queue::new(), bredr: Pool { len: 0, total: 0, free: 0 }, le: Pool { len: 0, total: 0, free: 0 }, acl_sent: 0, completed_packets: 0, completed_events: 0, not_queued: 0,
               out: [Queue::new(), Queue::new()], links: [Link { handle: 0, le: false, held: 0, live: false }; LINKS], log: Log::new(), now: 0, lost_commands: 0 }
    }

    pub fn say(&mut self, args: core::fmt::Arguments) { self.log.say(args) }

    /// Queues a command.
    pub fn command(&mut self, opcode: u16, params: &[u8]) {
        let n = params.len().min(255);
        if !self.cmds.push(&[&[opcode as u8, (opcode >> 8) as u8, n as u8], &params[..n]]) {
            self.log.say(format_args!("COMMAND {:04X} DROPPED: THE QUEUE IS FULL", opcode));
        }
    }

    /// The next command packet for the controller in `out`, when it takes one; its length.
    pub fn next_command(&mut self, out: &mut [u8]) -> Option<usize> {
        if self.credits == 0 {
            match self.waiting {
                Some((op, at)) if self.now > at + COMMAND_MS => {
                    self.lost_commands += 1;
                    self.log.say(format_args!("COMMAND {:04X} NOT ANSWERED IN {} MS: GOING ON", op, COMMAND_MS));
                    self.credits = 1;
                }
                _ => return None,
            }
        }
        let n = self.cmds.take(out)?;
        self.credits -= 1;
        self.waiting = Some((u16::from_le_bytes([out[0], out[1]]), self.now));
        Some(n)
    }

    /// A Command Complete or Command Status gave the controller's credit.
    pub fn answered(&mut self, credits: u8, opcode: u16) {
        self.credits = credits;
        if self.waiting.is_some_and(|(op, _)| op == opcode) || opcode == 0 { self.waiting = None; }
    }

    pub fn commands_queued(&self) -> usize { self.cmds.len() }

    /// A link came up: its packets go through its transport's pool.
    pub fn link_up(&mut self, handle: u16, le: bool) {
        if let Some(l) = self.links.iter_mut().find(|l| !l.live) { *l = Link { handle, le, held: 0, live: true }; }
    }

    /// A link went down: the controller dropped the packets it held for it, which are free again (Vol 4 Part E 4.3).
    pub fn link_down(&mut self, handle: u16) {
        let Some(i) = self.links.iter().position(|l| l.live && l.handle == handle) else { return };
        let l = self.links[i];
        self.links[i].live = false;
        let pool = if l.le && self.le.total != 0 { &mut self.le } else { &mut self.bredr };
        pool.free = (pool.free + l.held).min(pool.total);
    }

    /// Number Of Completed Packets: `count` of `handle`'s packets left the controller.
    pub fn completed(&mut self, handle: u16, count: u16) {
        let handle = handle & 0x0FFF;
        self.completed_events += 1;
        self.completed_packets += count as u32;
        let Some(l) = self.links.iter_mut().find(|l| l.live && l.handle == handle) else { return };
        let count = count.min(l.held);
        l.held -= count;
        let pool = if l.le && self.le.total != 0 { &mut self.le } else { &mut self.bredr };
        pool.free = (pool.free + count).min(pool.total);
    }

    /// Packets of `handle` the controller holds.
    pub fn held(&self, handle: u16) -> u16 { self.links.iter().find(|l| l.live && l.handle == handle).map_or(0, |l| l.held) }

    /// Packets of `handle` queued or held by the controller.
    pub fn backlog(&self, handle: u16) -> usize {
        let held = self.links.iter().find(|l| l.live && l.handle == handle).map_or(0, |l| l.held as usize);
        held + self.out[0].len() + self.out[1].len()
    }

    /// Queues an L2CAP basic frame (`cid` and the parts as its payload) for link `handle`, cut into ACL packets of the
    /// controller's length; false when the queue had no room for all of it (then nothing is queued).
    pub fn l2cap(&mut self, handle: u16, cid: u16, parts: &[&[u8]]) -> bool {
        let le = self.links.iter().find(|l| l.live && l.handle == handle).is_some_and(|l| l.le);
        let len: usize = parts.iter().map(|p| p.len()).sum();
        if len + 4 > acl::PDU_MAX { self.log.say(format_args!("L2CAP FRAME OF {} BYTES ON {:04X}: TOO LONG, DROPPED", len, cid)); return false; }
        let mut pdu = [0u8; acl::PDU_MAX];
        pdu[..4].copy_from_slice(&[len as u8, (len >> 8) as u8, cid as u8, (cid >> 8) as u8]);
        let mut at = 4;
        for p in parts { pdu[at..at + p.len()].copy_from_slice(p); at += p.len(); }
        let pool_len = if le && self.le.total != 0 { self.le.len } else { self.bredr.len } as usize;
        if pool_len == 0 { return false; }
        let queue = &mut self.out[le as usize];
        let fragments = at.div_ceil(pool_len);
        if queue.room() < at + fragments * (acl::HEADER + 2) { queue.dropped += 1; self.not_queued += 1; return false; }
        for (i, chunk) in pdu[..at].chunks(pool_len).enumerate() {
            let pb = if i > 0 { acl::PB_CONTINUING } else if le { acl::PB_FIRST_NO_FLUSH } else { acl::PB_FIRST };
            queue.push(&[&acl::header(handle, pb, chunk.len()), chunk]);
        }
        true
    }

    /// The next ACL packet the controller has a buffer for, in `out`; its length. Packets of a link gone are dropped.
    pub fn next_acl(&mut self, out: &mut [u8]) -> Option<usize> {
        for le in [false, true] {
            loop {
                let queue = &mut self.out[le as usize];
                let Some(packet) = queue.front() else { break };
                let handle = u16::from_le_bytes([packet[0], packet[1]]) & 0x0FFF;
                let Some(i) = self.links.iter().position(|l| l.live && l.handle == handle) else { queue.pop(); continue };
                let pool = if le && self.le.total != 0 { &mut self.le } else { &mut self.bredr };
                if pool.free == 0 { break; }
                pool.free -= 1;
                self.links[i].held += 1;
                self.acl_sent += 1;
                let queue = &mut self.out[le as usize];
                return queue.take(out);
            }
        }
        None
    }
}
