//! Host tests of a listening port's backlog and its half-open limit (netstack/src/listen.rs, 110-NET-0052), on smoltcp's
//! own TCP over a loopback wire that can lose a client's acknowledgements.
#![allow(dead_code)]
extern crate alloc;

#[path = "../netstack/src/listen.rs"]
mod listen;

use listen::{admit, Listener, Refusal, BACKLOG_MAX, HALF_OPEN_MS, LISTENING_MAX};
use smoltcp::iface::{Config, Interface, SocketHandle, SocketSet};
use smoltcp::phy::{self, ChecksumCapabilities, Device, DeviceCapabilities, Medium};
use smoltcp::socket::tcp;
use smoltcp::time::Instant;
use smoltcp::wire::{HardwareAddress, IpAddress, IpCidr, IpListenEndpoint};
use std::collections::{HashSet, VecDeque};

const PORT: u16 = 7000;
const BUFFER: usize = 2048;
const NEAR: [u8; 4] = [10, 0, 0, 1];
const FAR: [u8; 4] = [10, 0, 0, 2];

// IP packets sent come back in order, except a client's acknowledgements to PORT from the ports in `deaf`.
#[derive(Default)]
struct Wire { queue: VecDeque<Vec<u8>>, deaf: HashSet<u16> }

impl Wire {
    fn lost(&self, packet: &[u8]) -> bool {
        let ihl = (packet[0] & 0x0F) as usize * 4;
        if packet.len() < ihl + 14 || packet[9] != 6 { return false; }
        let tcp = &packet[ihl..];
        let (from, to, flags) = (u16::from_be_bytes([tcp[0], tcp[1]]), u16::from_be_bytes([tcp[2], tcp[3]]), tcp[13]);
        to == PORT && self.deaf.contains(&from) && flags & 0x10 != 0 && flags & 0x06 == 0
    }
}

struct Rx(Vec<u8>);
impl phy::RxToken for Rx {
    fn consume<R, F: FnOnce(&[u8]) -> R>(self, f: F) -> R { f(&self.0) }
}
struct Tx<'a>(&'a mut Wire);
impl phy::TxToken for Tx<'_> {
    fn consume<R, F: FnOnce(&mut [u8]) -> R>(self, len: usize, f: F) -> R {
        let mut packet = vec![0; len];
        let result = f(&mut packet);
        if !self.0.lost(&packet) { self.0.queue.push_back(packet); }
        result
    }
}
impl Device for Wire {
    type RxToken<'a> = Rx;
    type TxToken<'a> = Tx<'a>;
    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = Medium::Ip;
        caps.max_transmission_unit = 1500;
        caps.checksum = ChecksumCapabilities::ignored();
        caps
    }
    fn receive(&mut self, _: Instant) -> Option<(Rx, Tx<'_>)> {
        let packet = self.queue.pop_front()?;
        Some((Rx(packet), Tx(self)))
    }
    fn transmit(&mut self, _: Instant) -> Option<Tx<'_>> { Some(Tx(self)) }
}

// One interface holding both addresses: the listener's peers are its own clients.
struct Net { wire: Wire, iface: Interface, sockets: SocketSet<'static>, now: i64, refused: Vec<(u32, u16)> }

impl Net {
    fn new() -> Self {
        let mut wire = Wire::default();
        let mut iface = Interface::new(Config::new(HardwareAddress::Ip), &mut wire, Instant::from_millis(0));
        iface.update_ip_addrs(|addrs| {
            addrs.push(IpCidr::new(IpAddress::v4(NEAR[0], NEAR[1], NEAR[2], NEAR[3]), 24)).unwrap();
            addrs.push(IpCidr::new(IpAddress::v4(FAR[0], FAR[1], FAR[2], FAR[3]), 24)).unwrap();
        });
        Self { wire, iface, sockets: SocketSet::new(Vec::new()), now: 0, refused: Vec::new() }
    }
    // A client connecting to PORT from `from`:`port`.
    fn connect(&mut self, from: [u8; 4], port: u16) -> SocketHandle {
        let mut socket = tcp::Socket::new(tcp::SocketBuffer::new(vec![0; BUFFER]), tcp::SocketBuffer::new(vec![0; BUFFER]));
        let local = IpListenEndpoint { addr: Some(IpAddress::v4(from[0], from[1], from[2], from[3])), port };
        socket.connect(self.iface.context(), (IpAddress::v4(NEAR[0], NEAR[1], NEAR[2], NEAR[3]), PORT), local).unwrap();
        self.sockets.add(socket)
    }
    // The interface runs until the wire is quiet, then the listener takes its turn, as netstack's loop does.
    fn run(&mut self, listener: &mut Listener, admits: impl Fn(u32) -> bool) {
        for _ in 0..8 {
            for _ in 0..32 {
                self.iface.poll(Instant::from_millis(self.now), &mut self.wire, &mut self.sockets);
                if self.wire.queue.is_empty() { break; }
            }
            let refused = &mut self.refused;
            listener.turn(&mut self.sockets, BUFFER, &admits, |_, address, port| refused.push((address, port)));
        }
    }
    // Time passes, the interface running every 100 ms.
    fn wait(&mut self, listener: &mut Listener, ms: i64, admits: impl Fn(u32) -> bool + Copy) {
        let end = self.now + ms;
        while self.now < end {
            self.now = (self.now + 100).min(end);
            self.run(listener, admits);
        }
    }
    fn state(&self, handle: SocketHandle) -> tcp::State { self.sockets.get::<tcp::Socket>(handle).state() }
    fn listening(&self, listener: &Listener) -> usize {
        listener.listening.iter().filter(|&&h| self.state(h) == tcp::State::Listen).count()
    }
}

fn near(_: u32) -> bool { true }

#[test]
fn the_port_backlog_and_listening_sockets_are_bounded() {
    assert_eq!(admit(0, 4, std::iter::empty()), Err(Refusal::Invalid));
    assert_eq!(admit(PORT, 0, std::iter::empty()), Err(Refusal::Invalid));
    assert_eq!(admit(PORT, BACKLOG_MAX + 1, std::iter::empty()), Err(Refusal::Invalid));
    assert_eq!(admit(PORT, BACKLOG_MAX, std::iter::empty()), Ok(()));
    assert_eq!(admit(PORT, 1, [(7001, 2), (PORT, 1)].into_iter()), Err(Refusal::Taken));
    // Every port's listening sockets count together.
    let held = [(7001, BACKLOG_MAX as usize), (7002, LISTENING_MAX - BACKLOG_MAX as usize - 2)];
    assert_eq!(admit(PORT, 2, held.into_iter()), Ok(()));
    assert_eq!(admit(PORT, 3, held.into_iter()), Err(Refusal::Limit));
}

#[test]
fn a_backlog_holds_that_many_connections_and_a_further_peer_is_reset() {
    let mut net = Net::new();
    let mut listener = Listener::new(&mut net.sockets, PORT, 2, BUFFER).unwrap();
    assert_eq!(net.listening(&listener), 2);
    let (a, b) = (net.connect(NEAR, 50001), net.connect(NEAR, 50002));
    net.run(&mut listener, near);
    assert_eq!(listener.ready.iter().map(|&(_, address, port)| (address, port)).collect::<Vec<_>>(),
               [(u32::from_be_bytes(NEAR), 50001), (u32::from_be_bytes(NEAR), 50002)]);
    assert!(listener.listening.is_empty(), "two set up and not accepted fill a backlog of two");
    assert_eq!((net.state(a), net.state(b)), (tcp::State::Established, tcp::State::Established));
    // No socket listens: smoltcp resets the third.
    let c = net.connect(NEAR, 50003);
    net.run(&mut listener, near);
    assert_eq!(net.state(c), tcp::State::Closed);
    assert_eq!(listener.ready.len(), 2);
    // One accepted leaves room for one more.
    listener.ready.remove(0);
    net.run(&mut listener, near);
    assert_eq!(net.listening(&listener), 1);
    let d = net.connect(NEAR, 50004);
    net.run(&mut listener, near);
    assert_eq!(net.state(d), tcp::State::Established);
    assert_eq!(listener.ready.last().map(|&(_, _, port)| port), Some(50004));
    assert!(net.refused.is_empty());
}

#[test]
fn a_peer_outside_the_grants_networks_is_reset_and_never_ready() {
    let mut net = Net::new();
    let mut listener = Listener::new(&mut net.sockets, PORT, 2, BUFFER).unwrap();
    let only_near = |address: u32| address == u32::from_be_bytes(NEAR);
    let far = net.connect(FAR, 50001);
    net.run(&mut listener, only_near);
    assert_eq!(net.refused, [(u32::from_be_bytes(FAR), 50001)]);
    assert_eq!(net.state(far), tcp::State::Closed, "the peer is reset");
    assert!(listener.ready.is_empty());
    assert_eq!(net.listening(&listener), 2, "the backlog is filled again");
    let near = net.connect(NEAR, 50002);
    net.run(&mut listener, only_near);
    assert_eq!(net.state(near), tcp::State::Established);
    assert_eq!(listener.ready.len(), 1);
}

#[test]
fn connections_left_half_open_hold_the_backlog_until_they_go_after_ten_seconds() {
    let mut net = Net::new();
    let mut listener = Listener::new(&mut net.sockets, PORT, 2, BUFFER).unwrap();
    // A flood: two peers whose acknowledgements never arrive.
    net.wire.deaf.extend([50001, 50002]);
    let (a, b) = (net.connect(NEAR, 50001), net.connect(NEAR, 50002));
    net.run(&mut listener, near);
    let half: Vec<_> = listener.listening.iter().map(|&h| net.state(h)).collect();
    assert_eq!(half, [tcp::State::SynReceived, tcp::State::SynReceived]);
    assert!(listener.ready.is_empty());
    // Their half-open sockets hold the backlog: a third peer is reset.
    let c = net.connect(NEAR, 50003);
    net.run(&mut listener, near);
    assert_eq!(net.state(c), tcp::State::Closed);
    // Still held a little before HALF_OPEN_MS.
    net.wait(&mut listener, HALF_OPEN_MS as i64 - 500, near);
    assert_eq!(listener.listening.iter().filter(|&&h| net.state(h) == tcp::State::SynReceived).count(), 2);
    // Past it they go, and the backlog listens again.
    net.wait(&mut listener, 1000, near);
    assert_eq!(net.listening(&listener), 2, "{:?}", listener.listening.iter().map(|&h| net.state(h)).collect::<Vec<_>>());
    assert!(listener.ready.is_empty(), "a half-open connection is never ready");
    let _ = (a, b);
    let d = net.connect(NEAR, 50004);
    net.run(&mut listener, near);
    assert_eq!(net.state(d), tcp::State::Established);
    assert_eq!(listener.ready.len(), 1);
}

#[test]
fn a_listener_that_cannot_listen_leaves_nothing() {
    let mut net = Net::new();
    assert!(Listener::new(&mut net.sockets, 0, 3, BUFFER).is_none(), "port 0 cannot listen");
    assert_eq!(net.sockets.iter().count(), 0);
}
