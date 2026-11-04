//! A listening TCP port's sockets (socket.wit 2.9, 110-NET-0052): smoltcp sockets listening on the port, as many as the
//! backlog leaves room for, and the connections peers set up on them that wait to be accepted. A peer outside the grant's
//! networks is reset, a connection left half set up goes after HALF_OPEN_MS, and the backlog is filled again each turn.
use alloc::vec;
use alloc::vec::Vec;
use smoltcp::iface::{SocketHandle, SocketSet};
use smoltcp::socket::tcp;
use smoltcp::wire::IpAddress;

/// Connections a listening port holds set up and not yet accepted, at most.
pub const BACKLOG_MAX: u8 = 16;
/// Listening sockets of every port together: what a flood of SYNs can hold.
pub const LISTENING_MAX: usize = 32;
/// A connection set up no further than this long is dropped (smoltcp's timeout).
pub const HALF_OPEN_MS: u64 = 10_000;

/// Why a port may not listen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Port 0, or a backlog of 0 or above BACKLOG_MAX.
    Invalid,
    /// Another listener holds the port.
    Taken,
    /// The listening sockets of every port would pass LISTENING_MAX.
    Limit,
}

/// Whether `port` may listen with `backlog` beside `others`, each listener's port and listening sockets.
pub fn admit(port: u16, backlog: u8, others: impl Iterator<Item = (u16, usize)>) -> Result<(), Refusal> {
    if port == 0 || !(1..=BACKLOG_MAX).contains(&backlog) { return Err(Refusal::Invalid); }
    let mut held = 0;
    for (other, listening) in others {
        if other == port { return Err(Refusal::Taken); }
        held += listening;
    }
    if held + backlog as usize > LISTENING_MAX { return Err(Refusal::Limit); }
    Ok(())
}

pub struct Listener { pub port: u16, pub backlog: usize, pub listening: Vec<SocketHandle>, pub ready: Vec<(SocketHandle, u32, u16)> }

// A socket listening on `port` with buffers of `buffer` bytes; one set up no further than HALF_OPEN_MS is dropped.
fn socket(port: u16, buffer: usize) -> Option<tcp::Socket<'static>> {
    let mut socket = tcp::Socket::new(tcp::SocketBuffer::new(vec![0; buffer]), tcp::SocketBuffer::new(vec![0; buffer]));
    socket.set_timeout(Some(smoltcp::time::Duration::from_millis(HALF_OPEN_MS)));
    socket.listen(port).ok()?;
    Some(socket)
}

impl Listener {
    /// `backlog` sockets listening on `port` in `sockets`; none, and nothing left in the set, if one cannot listen.
    pub fn new(sockets: &mut SocketSet<'static>, port: u16, backlog: u8, buffer: usize) -> Option<Self> {
        let mut listening = Vec::with_capacity(backlog as usize);
        for _ in 0..backlog {
            let Some(socket) = socket(port, buffer) else {
                for handle in listening { sockets.remove(handle); }
                return None;
            };
            listening.push(sockets.add(socket));
        }
        Some(Self { port, backlog: backlog as usize, listening, ready: Vec::new() })
    }

    /// One turn once the interface ran. A connection a peer is setting up, or set up, is kept if `admits` its address,
    /// else aborted at once and given to `refused` (its handle and the peer). One dropped half set up, or closed before it
    /// was set up, leaves the set. The backlog is filled again with listening sockets.
    pub fn turn(&mut self, sockets: &mut SocketSet<'static>, buffer: usize, admits: impl Fn(u32) -> bool, mut refused: impl FnMut(SocketHandle, u32, u16)) {
        let mut kept = Vec::with_capacity(self.listening.len());
        for handle in self.listening.drain(..) {
            let tcp = sockets.get_mut::<tcp::Socket>(handle);
            let peer = tcp.remote_endpoint().map(|e| { let IpAddress::Ipv4(a) = e.addr; (u32::from(a), e.port) });
            match (tcp.state(), peer) {
                (tcp::State::Listen, _) | (tcp::State::SynReceived, None) => kept.push(handle),
                (tcp::State::SynReceived | tcp::State::Established | tcp::State::CloseWait, Some((address, port))) => {
                    if !admits(address) {
                        tcp.abort();
                        refused(handle, address, port);
                    } else if tcp.state() == tcp::State::SynReceived {
                        kept.push(handle);
                    } else {
                        tcp.set_timeout(None);
                        self.ready.push((handle, address, port));
                    }
                }
                _ => { sockets.remove(handle); }
            }
        }
        self.listening = kept;
        while self.listening.len() + self.ready.len() < self.backlog {
            let Some(socket) = socket(self.port, buffer) else { break };
            self.listening.push(sockets.add(socket));
        }
    }

    /// One of its sockets that lives: a listening one, else a ready one.
    pub fn handle(&self) -> Option<SocketHandle> { self.listening.first().or(self.ready.first().map(|(h, ..)| h)).copied() }
}
