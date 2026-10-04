#![no_std]
#![no_main]
// Network stack: IPv4 with DHCP (static QEMU fallback), ARP, ICMP echo, a DNS resolver, UDP and TCP (smoltcp) over the
// network card drivers' frames. Serves idl/socket.wit 2.1. Holds clients of up to CARDS card drivers (slots 2 and 3)
// and nothing of the devices themselves (Appendix B.6: packet and flow endpoints with quotas, no power over the
// device); each card is an interface of its own (issue 105) and a flow goes out through the one its route names. What
// each client may reach comes from the badge of its capability (policy.rs, MC-11.6).
extern crate alloc;

mod dns;
mod policy;

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use mind::abi::{BootInfo, SLOT_DEV0, SLOT_DEV1};
use mind::idl::codec::List;
use mind::idl::{net, socket, wire};
use mind::ipc::Endpoint;
use smoltcp::iface::{Config, Interface, SocketHandle, SocketSet};
use smoltcp::phy::{Device, DeviceCapabilities, Medium};
use smoltcp::socket::{dhcpv4, icmp, tcp, udp};
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress, HardwareAddress, IpAddress, IpCidr, IpEndpoint, Ipv4Address, Ipv4Cidr};
use policy::{Access, Policy};
use socket::{Error, Protocol};

const RECEIVED: usize = 9;
const FRAME_MAX: usize = 1514;
const SOCKETS: usize = 32; // all clients together
const PER_CLIENT: usize = 8;
const TCP_BUFFER: usize = 8192;
const UDP_PACKETS: usize = 8;
const UDP_BUFFER: usize = 8192;
const POLL_MS: u32 = 10;
const FALLBACK_MS: u64 = 3000; // without a DHCP lease by then: QEMU user networking's static setup
const ICMP_IDENT: u16 = 0x4D49;
const EPHEMERAL: u16 = 49152;
const CARDS: usize = 2; // card drivers: SLOT_DEV0, SLOT_DEV1 (init starts one virtio_net instance per card)
const DRIVERS: [usize; CARDS] = [SLOT_DEV0, SLOT_DEV1];

// Transmit checksum offload (issue 106, socket.wit `offload`): off until the operator turns it on; the benchmark
// (docs/profile/network.md) showed no gain worth trusting the device with it by default.
static OFFLOAD: AtomicBool = AtomicBool::new(false);

// Frames each card sent and received.
static SENT: [AtomicU64; CARDS] = [const { AtomicU64::new(0) }; CARDS];
static RECEIVED_FRAMES: [AtomicU64; CARDS] = [const { AtomicU64::new(0) }; CARDS];

// A driver's frames as a smoltcp device: one IDL call per frame.
// `offload`: the driver completes TCP and UDP checksums (it offers it and OFFLOAD is on).
struct Card { endpoint: Endpoint, index: usize, offers: bool, offload: bool, frame: [u8; FRAME_MAX] }
struct Rx<'a>(&'a [u8]);
struct Tx(Endpoint, usize, bool);

// For an IPv4 TCP or UDP frame (not a fragment): puts the pseudo-header sum in its checksum field and returns where the
// summed bytes start, where the field is from there (virtio_net_hdr `csum_start`, `csum_offset`) and where the IP
// packet ends.
fn partial(frame: &mut [u8]) -> Option<(usize, usize, usize)> {
    if frame.len() < 34 || frame[12..14] != [0x08, 0x00] || frame[14] >> 4 != 4 { return None; }
    let ihl = (frame[14] & 0x0F) as usize * 4;
    let field = match frame[23] { 6 => 16, 17 => 6, _ => return None };
    if frame[20] & 0x3F != 0 || frame[21] != 0 { return None; } // more fragments or an offset
    let total = u16::from_be_bytes([frame[16], frame[17]]) as usize;
    let (start, length) = (14 + ihl, total.checked_sub(ihl)?);
    if ihl < 20 || 14 + total > frame.len() || field + 2 > length { return None; }
    let mut sum = frame[26..34].chunks(2).map(|w| u16::from_be_bytes([w[0], w[1]]) as u32).sum::<u32>() + frame[23] as u32 + length as u32;
    while sum > 0xFFFF { sum = (sum & 0xFFFF) + (sum >> 16); }
    frame[start + field..start + field + 2].copy_from_slice(&(sum as u16).to_be_bytes());
    Some((start, field, 14 + total))
}

// Completes a partial checksum in software: the one's complement of the sum from `start` to `end`.
fn complete(frame: &mut [u8], start: usize, field: usize, end: usize) {
    let data = &frame[start..end];
    let mut sum = data.chunks_exact(2).map(|w| u16::from_be_bytes([w[0], w[1]]) as u32).sum::<u32>();
    if data.len() % 2 == 1 { sum += (data[data.len() - 1] as u32) << 8; }
    while sum > 0xFFFF { sum = (sum & 0xFFFF) + (sum >> 16); }
    let mut value = !(sum as u16);
    if value == 0 && frame[23] == 17 { value = 0xFFFF; } // UDP: 0 means "no checksum"
    frame[start + field..start + field + 2].copy_from_slice(&value.to_be_bytes());
}
impl smoltcp::phy::RxToken for Rx<'_> {
    fn consume<R, F: FnOnce(&[u8]) -> R>(self, f: F) -> R { f(self.0) }
}
impl smoltcp::phy::TxToken for Tx {
    fn consume<R, F: FnOnce(&mut [u8]) -> R>(self, len: usize, f: F) -> R {
        let mut frame = [0u8; FRAME_MAX];
        let len = len.min(FRAME_MAX);
        let result = f(&mut frame[..len]);
        // TCP and UDP checksums are finished here: by the card with offload, else in software (smoltcp leaves them).
        let offloaded = match partial(&mut frame[..len]) {
            Some((start, field, _)) if self.2 && matches!(net::send_partial(self.0, &frame[..len], start as u16, field as u16), Ok(Ok(()))) => Some(true),
            Some((start, field, end)) => { complete(&mut frame[..len], start, field, end); None }
            None => None,
        };
        let sent = offloaded.unwrap_or_else(|| matches!(net::send(self.0, &frame[..len]), Ok(Ok(()))));
        if sent { SENT[self.1].fetch_add(1, Ordering::Relaxed); }
        result
    }
}
impl Device for Card {
    type RxToken<'a> = Rx<'a>;
    type TxToken<'a> = Tx;
    fn receive(&mut self, _: Instant) -> Option<(Rx<'_>, Tx)> {
        match net::receive(self.endpoint, &mut self.frame) {
            Ok(Ok(len)) => { RECEIVED_FRAMES[self.index].fetch_add(1, Ordering::Relaxed); Some((Rx(&self.frame[..len.min(FRAME_MAX)]), Tx(self.endpoint, self.index, self.offload))) }
            _ => None,
        }
    }
    fn transmit(&mut self, _: Instant) -> Option<Tx> { Some(Tx(self.endpoint, self.index, self.offload)) }
    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = Medium::Ethernet; caps.max_transmission_unit = FRAME_MAX;
        // smoltcp verifies received checksums; TCP and UDP ones to send are finished in `Tx` (by the card or in software),
        // so offload can be switched while the interface runs.
        caps.checksum.tcp = smoltcp::phy::Checksum::Rx; caps.checksum.udp = smoltcp::phy::Checksum::Rx;
        caps
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Kind { Udp, Tcp }
// A client's socket: its interface (`link`) and its handle in that interface's socket set.
#[derive(Clone, Copy)]
struct Slot { owner: (u64, u16), kind: Kind, link: usize, handle: SocketHandle, generation: u32 }

enum Waiting {
    Ping { call: wire::Call, link: usize, sequence: u16, sent: u64, deadline: u64 },
    Resolve { call: wire::Call, link: usize, handle: SocketHandle, id: u16, deadline: u64 },
    Connect { call: wire::Call, index: usize, deadline: u64 },
}

// One interface: a card and its configuration, with the sockets that go out through it.
struct Link {
    card: Card, iface: Interface, sockets: SocketSet<'static>, dhcp: SocketHandle, icmp: SocketHandle,
    mac: u64, address: Option<Ipv4Cidr>, gateway: Option<Ipv4Address>, dns: Option<Ipv4Address>, leased: bool, started: u64,
}

struct Stack {
    links: [Option<Link>; CARDS], tried: [u64; CARDS],
    slots: [Option<Slot>; SOCKETS], generations: [u32; SOCKETS], waiting: Vec<Waiting>, closing: Vec<(usize, SocketHandle, u64)>,
    sequence: u16, next_port: u16,
}

fn now_ms() -> u64 { mind::time::monotonic_ns() / 1_000_000 }
fn instant() -> Instant { Instant::from_micros((mind::time::monotonic_ns() / 1000) as i64) }
fn ip(address: u32) -> IpAddress { IpAddress::Ipv4(Ipv4Address::from(address)) }
fn text(address: Ipv4Address) -> [u8; 4] { address.octets() }

impl Link {
    // The interface of card `index`, or None while its driver does not answer.
    fn new(index: usize) -> Option<Self> {
        let endpoint = Endpoint(DRIVERS[index]);
        let info = match net::info(endpoint) { Ok(Ok(info)) => info, _ => return None };
        let bytes = info.mac.to_be_bytes();
        let offers = net::offloads(endpoint).is_ok_and(|o| o & 1 != 0);
        let mut card = Card { endpoint, index, offers, offload: offers && OFFLOAD.load(Ordering::Relaxed), frame: [0; FRAME_MAX] };
        let mut config = Config::new(HardwareAddress::Ethernet(EthernetAddress([bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]])));
        config.random_seed = mind::time::rdtsc();
        let iface = Interface::new(config, &mut card, instant());
        let mut sockets = SocketSet::new(Vec::new());
        let dhcp = sockets.add(dhcpv4::Socket::new());
        let mut echo = icmp::Socket::new(icmp::PacketBuffer::new(vec![icmp::PacketMetadata::EMPTY; 8], vec![0; 4096]),
                                         icmp::PacketBuffer::new(vec![icmp::PacketMetadata::EMPTY; 8], vec![0; 4096]));
        let _ = echo.bind(icmp::Endpoint::Ident(ICMP_IDENT));
        let icmp = sockets.add(echo);
        mind::println!("[NETSTACK] CARD {} READY{}, WAITING FOR DHCP", index, if offers { " (CHECKSUM OFFLOAD AVAILABLE)" } else { "" });
        Some(Self { card, iface, sockets, dhcp, icmp, mac: info.mac, address: None, gateway: None, dns: None, leased: false, started: now_ms() })
    }

    fn configure(&mut self, address: Ipv4Cidr, gateway: Option<Ipv4Address>, dns: Option<Ipv4Address>, leased: bool) {
        self.iface.update_ip_addrs(|addrs| { addrs.clear(); let _ = addrs.push(IpCidr::Ipv4(address)); });
        self.iface.routes_mut().remove_default_ipv4_route();
        if let Some(gateway) = gateway { let _ = self.iface.routes_mut().add_default_ipv4_route(gateway); }
        let (old, new) = (self.address, Some(address));
        self.address = new; self.gateway = gateway; self.dns = dns; self.leased = leased;
        if old != new {
            let [a, b, c, d] = text(address.address());
            let g = gateway.map_or([0; 4], text); let n = dns.map_or([0; 4], text);
            mind::println!("[NETSTACK] {} {}.{}.{}.{}/{} GATEWAY {}.{}.{}.{} DNS {}.{}.{}.{} ON CARD {}", if leased { "DHCP" } else { "STATIC" }, a, b, c, d,
                           address.prefix_len(), g[0], g[1], g[2], g[3], n[0], n[1], n[2], n[3], self.card.index);
        }
    }

    // Runs the interface and applies DHCP results.
    fn poll(&mut self) {
        let _ = self.iface.poll(instant(), &mut self.card, &mut self.sockets);
        let event = self.sockets.get_mut::<dhcpv4::Socket>(self.dhcp).poll().map(|event| match event {
            dhcpv4::Event::Configured(config) => Some((config.address, config.router, config.dns_servers.first().copied())),
            dhcpv4::Event::Deconfigured => None,
        });
        match event {
            Some(Some((address, router, dns))) => self.configure(address, router, dns, true),
            Some(None) => { self.address = None; self.leased = false; }
            // QEMU user networking's static setup, for the first card only (another card's network is unknown).
            None => if self.card.index == 0 && self.address.is_none() && now_ms() - self.started > FALLBACK_MS {
                self.configure(Ipv4Cidr::new(Ipv4Address::new(10, 0, 2, 15), 24), Some(Ipv4Address::new(10, 0, 2, 2)), Some(Ipv4Address::new(10, 0, 2, 3)), false);
            },
        }
    }

    fn describe(&self) -> socket::Interface {
        socket::Interface { card: self.card.index as u8, address: self.address.map_or(0, |c| u32::from(c.address())), prefix: self.address.map_or(0, |c| c.prefix_len()),
                            gateway: self.gateway.map_or(0, u32::from), dns: self.dns.map_or(0, u32::from), mac: self.mac, dhcp: self.leased,
                            sent: SENT[self.card.index].load(Ordering::Relaxed), received: RECEIVED_FRAMES[self.card.index].load(Ordering::Relaxed) }
    }
}

impl Stack {
    fn new() -> Self {
        Self { links: [const { None }; CARDS], tried: [0; CARDS], slots: [None; SOCKETS], generations: [0; SOCKETS], waiting: Vec::new(), closing: Vec::new(),
               sequence: 0, next_port: EPHEMERAL }
    }

    fn link(&mut self, index: usize) -> &mut Link { self.links[index].as_mut().expect("sockets live only on present links") }

    // The interface a flow to `address` goes out through: the one whose network holds it, else the first with a
    // gateway, else the first configured one.
    fn route(&self, address: u32) -> Option<usize> {
        let target = Ipv4Address::from(address);
        let configured = |l: &Option<Link>| l.as_ref().and_then(|l| l.address.map(|cidr| (cidr, l.gateway)));
        self.links.iter().position(|l| configured(l).is_some_and(|(cidr, _)| address != 0 && cidr.contains_addr(&target)))
            .or_else(|| self.links.iter().position(|l| configured(l).is_some_and(|(_, gateway)| gateway.is_some())))
            .or_else(|| self.links.iter().position(|l| configured(l).is_some()))
    }

    fn port(&mut self) -> u16 { let port = self.next_port; self.next_port = if port == u16::MAX { EPHEMERAL } else { port + 1 }; port }

    // Asks absent drivers again (every second), runs the interfaces, finishes waiting calls and frees closed sockets.
    fn poll(&mut self) {
        let now = now_ms();
        for index in 0..CARDS {
            if self.links[index].is_none() && now - self.tried[index] >= 1000 { self.tried[index] = now; self.links[index] = Link::new(index); }
            if let Some(link) = self.links[index].as_mut() { link.poll(); }
        }
        self.finish_pings();
        let mut index = 0;
        while index < self.waiting.len() {
            if let Some(result) = self.check(index, now) {
                let waiting = self.waiting.swap_remove(index);
                let _ = match (waiting, result) {
                    (Waiting::Ping { call, .. }, value) => socket::reply_ping(call, value),
                    (Waiting::Resolve { call, link, handle, .. }, value) => { self.link(link).sockets.remove(handle); socket::reply_resolve(call, value) }
                    (Waiting::Connect { call, index, .. }, value) => {
                        if value.is_err() { self.free(index); }
                        socket::reply_tcp_connect(call, value)
                    }
                };
            } else { index += 1; }
        }
        // Closed TCP connections leave once their queued data was sent, or after 10 s.
        let links = &mut self.links;
        self.closing.retain(|&(link, handle, since)| {
            let Some(link) = links[link].as_mut() else { return false };
            let done = matches!(link.sockets.get_mut::<tcp::Socket>(handle).state(), tcp::State::Closed | tcp::State::TimeWait) || now - since > 10_000;
            if done { link.sockets.remove(handle); }
            !done
        });
    }

    // The result of waiting call `index`, or None while it still waits.
    fn check(&mut self, index: usize, now: u64) -> Option<Result<u32, Error>> {
        match self.waiting[index] {
            Waiting::Ping { deadline, .. } => (now >= deadline).then_some(Err(Error::Timeout)),
            Waiting::Resolve { link, handle, id, deadline, .. } => {
                let socket = self.link(link).sockets.get_mut::<udp::Socket>(handle);
                while let Ok((data, _)) = socket.recv() {
                    if let Some(answer) = dns::answer(data, id) { return Some(answer.map_err(|missing| if missing { Error::NotFound } else { Error::Invalid })); }
                }
                (now >= deadline).then_some(Err(Error::Timeout))
            }
            Waiting::Connect { index: slot, deadline, .. } => {
                let Some(s) = self.slots[slot] else { return Some(Err(Error::Closed)) };
                match self.link(s.link).sockets.get_mut::<tcp::Socket>(s.handle).state() {
                    tcp::State::Established => Some(Ok(self.id(slot))),
                    tcp::State::Closed => Some(Err(Error::Refused)),
                    _ => (now >= deadline).then_some(Err(Error::Timeout)),
                }
            }
        }
    }

    fn finish_pings(&mut self) {
        let mut replies: Vec<(usize, u16)> = Vec::new();
        for (index, link) in self.links.iter_mut().enumerate() {
            let Some(link) = link.as_mut() else { continue };
            let socket = link.sockets.get_mut::<icmp::Socket>(link.icmp);
            while let Ok((data, _)) = socket.recv() {
                if data.len() >= 8 && data[0] == 0 && u16::from_be_bytes([data[4], data[5]]) == ICMP_IDENT { replies.push((index, u16::from_be_bytes([data[6], data[7]]))); }
            }
        }
        let now = mind::time::monotonic_ns();
        for (from, sequence) in replies {
            if let Some(index) = self.waiting.iter().position(|w| matches!(w, Waiting::Ping { link, sequence: s, .. } if *s == sequence && *link == from)) {
                if let Waiting::Ping { call, sent, .. } = self.waiting.swap_remove(index) { let _ = socket::reply_ping(call, Ok(((now - sent) / 1000) as u32)); }
            }
        }
    }

    fn id(&self, index: usize) -> u32 { (index as u32 + 1) | self.generations[index] << 8 }
    // The slot of handle `id` if it belongs to `owner`.
    fn slot(&self, id: u32, owner: (u64, u16), kind: Kind) -> Result<(usize, Slot), Error> {
        let index = (id & 0xFF) as usize;
        if index == 0 || index > SOCKETS { return Err(Error::NoSocket); }
        match self.slots[index - 1] {
            Some(s) if s.owner == owner && s.kind == kind && s.generation == id >> 8 => Ok((index - 1, s)),
            _ => Err(Error::NoSocket),
        }
    }
    fn take(&mut self, owner: (u64, u16), kind: Kind, link: usize, handle: SocketHandle) -> Result<usize, Error> {
        let full = self.slots.iter().flatten().filter(|s| s.owner == owner).count() >= PER_CLIENT;
        let free = self.slots.iter().position(Option::is_none);
        let (false, Some(index)) = (full, free) else { self.link(link).sockets.remove(handle); return Err(Error::Limit) };
        self.generations[index] = (self.generations[index] + 1) & 0xFF_FFFF;
        self.slots[index] = Some(Slot { owner, kind, link, handle, generation: self.generations[index] });
        Ok(index)
    }
    fn free(&mut self, index: usize) {
        let Some(slot) = self.slots[index].take() else { return };
        let Some(link) = self.links[slot.link].as_mut() else { return };
        match slot.kind {
            Kind::Udp => { link.sockets.remove(slot.handle); }
            Kind::Tcp => { link.sockets.get_mut::<tcp::Socket>(slot.handle).close(); self.closing.push((slot.link, slot.handle, now_ms())); }
        }
    }
    // Sockets of clients that ended.
    fn reap(&mut self) {
        for index in 0..SOCKETS {
            if let Some(slot) = self.slots[index] { if !mind::process::alive(slot.owner.0) { self.free(index); } }
        }
    }

    fn udp_socket() -> udp::Socket<'static> {
        udp::Socket::new(udp::PacketBuffer::new(vec![udp::PacketMetadata::EMPTY; UDP_PACKETS], vec![0; UDP_BUFFER]),
                         udp::PacketBuffer::new(vec![udp::PacketMetadata::EMPTY; UDP_PACKETS], vec![0; UDP_BUFFER]))
    }

    // A UDP socket follows its datagrams: it moves to the interface the next destination is routed through.
    fn udp_link(&mut self, index: usize, address: u32) -> Result<SocketHandle, Error> {
        let slot = self.slots[index].ok_or(Error::NoSocket)?;
        let link = self.route(address).ok_or(Error::NoNetwork)?;
        if link == slot.link { return Ok(slot.handle); }
        let socket = self.link(slot.link).sockets.remove(slot.handle);
        let handle = self.link(link).sockets.add(socket);
        self.slots[index] = Some(Slot { link, handle, ..slot });
        Ok(handle)
    }

    fn ping(&mut self, address: u32, timeout: u32) -> Result<(usize, u16, u64), Error> {
        let link = self.route(address).ok_or(Error::NoNetwork)?;
        self.sequence = self.sequence.wrapping_add(1);
        let mut packet = [0u8; 16];
        packet[0] = 8; packet[4..6].copy_from_slice(&ICMP_IDENT.to_be_bytes()); packet[6..8].copy_from_slice(&self.sequence.to_be_bytes());
        packet[8..16].copy_from_slice(b"MINDCORE");
        let sum = dns::checksum(&packet); packet[2..4].copy_from_slice(&sum.to_be_bytes());
        let l = self.link(link);
        l.sockets.get_mut::<icmp::Socket>(l.icmp).send_slice(&packet, ip(address)).map_err(|_| Error::Again)?;
        Ok((link, self.sequence, now_ms() + timeout.clamp(100, 30_000) as u64))
    }

    fn resolve(&mut self, name: &str, server: u32, port: u16, timeout: u32) -> Result<(usize, SocketHandle, u16, u64), Error> {
        let primary = self.route(0).ok_or(Error::NoNetwork)?;
        let server = if server != 0 { server } else { u32::from(self.link(primary).dns.ok_or(Error::NoNetwork)?) };
        let link = self.route(server).ok_or(Error::NoNetwork)?;
        let id = mind::time::rdtsc() as u16;
        let mut query = [0u8; 512];
        let len = dns::query(name, id, &mut query).ok_or(Error::Invalid)?;
        let mut socket = Self::udp_socket();
        let local = self.port();
        socket.bind(local).map_err(|_| Error::Limit)?;
        socket.send_slice(&query[..len], IpEndpoint::new(ip(server), if port == 0 { 53 } else { port })).map_err(|_| Error::Again)?;
        Ok((link, self.link(link).sockets.add(socket), id, now_ms() + timeout.clamp(100, 30_000) as u64))
    }

    // The default interface's configuration (idl/socket.wit `config`).
    fn config(&self) -> Option<socket::Config> {
        let link = self.links[self.route(0)?].as_ref()?;
        let cidr = link.address?;
        Some(socket::Config { address: u32::from(cidr.address()), prefix: cidr.prefix_len(), gateway: link.gateway.map_or(0, u32::from), dns: link.dns.map_or(0, u32::from), mac: link.mac, dhcp: link.leased })
    }
    // The DNS server of the default interface, as 0 without one.
    fn dns(&self) -> u32 { self.config().map_or(0, |c| c.dns) }

    // Answers one request; returns the bytes it moved (sent, received). `source` tells which datagrams the client may see.
    fn handle(&mut self, request: socket::Request, call: wire::Call, owner: (u64, u16), source: &dyn Fn(u32, u16) -> bool) -> (usize, usize) {
        let mut moved = (0, 0);
        let _ = match request {
            socket::Request::Config => socket::reply_config(call, self.config().as_ref().ok_or(Error::NoNetwork)),
            socket::Request::Interfaces => {
                let mut list = List::<socket::Interface, 4>::default();
                for link in self.links.iter().flatten() { list.push(link.describe()); }
                socket::reply_interfaces(call, Ok(list.as_slice()))
            }
            socket::Request::Offload { enable } => {
                OFFLOAD.store(enable, Ordering::Relaxed);
                let mut cards = 0;
                for link in self.links.iter_mut().flatten() { link.card.offload = enable && link.card.offers; cards |= (link.card.offload as u32) << link.card.index; }
                mind::println!("[NETSTACK] CHECKSUM OFFLOAD {} (CARDS {:#b})", if enable { "ON" } else { "OFF" }, cards);
                socket::reply_offload(call, Ok(cards))
            }
            socket::Request::Ping { address, timeout_ms } => match self.ping(address, timeout_ms) {
                Ok((link, sequence, deadline)) => self.wait(call, |call| Waiting::Ping { call, link, sequence, sent: mind::time::monotonic_ns(), deadline }),
                Err(error) => socket::reply_ping(call, Err(error)),
            },
            socket::Request::Resolve { name, server, port, timeout_ms } => match self.resolve(name.as_str(), server, port, timeout_ms) {
                Ok((link, handle, id, deadline)) => self.wait(call, |call| Waiting::Resolve { call, link, handle, id, deadline }),
                Err(error) => socket::reply_resolve(call, Err(error)),
            },
            socket::Request::UdpOpen { port } => {
                let mut socket = Self::udp_socket();
                let port = if port == 0 { self.port() } else { port };
                let result = match (socket.bind(port), self.route(0)) {
                    (Err(_), _) => Err(Error::Invalid),
                    (Ok(()), None) => Err(Error::NoNetwork),
                    (Ok(()), Some(link)) => { let handle = self.link(link).sockets.add(socket); self.take(owner, Kind::Udp, link, handle).map(|index| self.id(index)) }
                };
                socket::reply_udp_open(call, result)
            }
            socket::Request::UdpSend { socket: id, address, port, data } => {
                let result = self.slot(id, owner, Kind::Udp).and_then(|(index, _)| {
                    let handle = self.udp_link(index, address)?;
                    let link = self.slots[index].map_or(0, |s| s.link);
                    self.link(link).sockets.get_mut::<udp::Socket>(handle).send_slice(data, IpEndpoint::new(ip(address), port)).map_err(|_| Error::Again)
                });
                if result.is_ok() { moved.0 = data.len(); }
                socket::reply_udp_send(call, result)
            }
            socket::Request::UdpReceive { socket: id } => {
                let mut datagram = socket::Datagram { address: 0, port: 0, data: List::default() };
                let result = self.slot(id, owner, Kind::Udp).and_then(|(_, slot)| {
                    let (data, meta) = self.link(slot.link).sockets.get_mut::<udp::Socket>(slot.handle).recv().map_err(|_| Error::Again)?;
                    let IpAddress::Ipv4(from) = meta.endpoint.addr;
                    if !source(u32::from(from), meta.endpoint.port) { return Err(Error::Again); } // a source the grant does not name: dropped
                    moved.1 = data.len();
                    datagram.address = u32::from(from); datagram.port = meta.endpoint.port;
                    datagram.data = List::from_slice(&data[..data.len().min(1472)]).unwrap_or_default();
                    Ok(())
                });
                socket::reply_udp_receive(call, result.map(|()| &datagram))
            }
            socket::Request::TcpConnect { address, port, timeout_ms } => {
                let Some(link) = self.route(address) else { let _ = socket::reply_tcp_connect(call, Err(Error::NoNetwork)); return moved };
                let socket = tcp::Socket::new(tcp::SocketBuffer::new(vec![0; TCP_BUFFER]), tcp::SocketBuffer::new(vec![0; TCP_BUFFER]));
                let local = self.port();
                let l = self.link(link);
                let handle = l.sockets.add(socket);
                let connected = l.sockets.get_mut::<tcp::Socket>(handle).connect(l.iface.context(), IpEndpoint::new(ip(address), port), local);
                match connected.map_err(|_| Error::Invalid).and_then(|()| self.take(owner, Kind::Tcp, link, handle)) {
                    Ok(index) => { let deadline = now_ms() + timeout_ms.clamp(100, 60_000) as u64; self.wait(call, |call| Waiting::Connect { call, index, deadline }) }
                    Err(error) => {
                        let sockets = &mut self.link(link).sockets;
                        if sockets.iter().any(|(h, _)| h == handle) { sockets.remove(handle); }
                        socket::reply_tcp_connect(call, Err(error))
                    }
                }
            }
            socket::Request::TcpSend { socket: id, data } => {
                let result = self.slot(id, owner, Kind::Tcp).and_then(|(_, slot)| {
                    let socket = self.link(slot.link).sockets.get_mut::<tcp::Socket>(slot.handle);
                    if !socket.may_send() { return Err(Error::Closed); }
                    socket.send_slice(data).map(|n| n as u32).map_err(|_| Error::Closed)
                });
                if let Ok(n) = result { moved.0 = n as usize; }
                socket::reply_tcp_send(call, result)
            }
            socket::Request::TcpReceive { socket: id, length } => {
                let mut buffer = [0u8; 4096];
                let result = self.slot(id, owner, Kind::Tcp).and_then(|(_, slot)| {
                    let socket = self.link(slot.link).sockets.get_mut::<tcp::Socket>(slot.handle);
                    if socket.can_recv() {
                        let want = (length as usize).clamp(1, buffer.len());
                        socket.recv_slice(&mut buffer[..want]).map_err(|_| Error::Closed)
                    } else if !socket.may_recv() { Err(Error::Closed) } else { Err(Error::Again) }
                });
                if let Ok(n) = result { moved.1 = n; }
                socket::reply_tcp_receive(call, result.map(|n| &buffer[..n]))
            }
            socket::Request::Close { socket: id } => {
                let result = self.slot(id, owner, Kind::Udp).or_else(|_| self.slot(id, owner, Kind::Tcp)).map(|(index, _)| self.free(index));
                socket::reply_close(call, result)
            }
            // Policy requests are answered in main.
            _ => Ok(()),
        };
        moved
    }

    /// Closes every socket held with `badge`; returns how many.
    fn close_badge(&mut self, badge: u16) -> u32 {
        let mut closed = 0;
        for index in 0..SOCKETS { if self.slots[index].is_some_and(|s| s.owner.1 == badge) { self.free(index); closed += 1; } }
        closed
    }
    fn sockets_of(&self, badge: u16) -> u32 { self.slots.iter().flatten().filter(|s| s.owner.1 == badge).count() as u32 }

    // Parks a call that waits for the network; it is answered from `poll`.
    fn wait(&mut self, mut call: wire::Call, make: impl FnOnce(wire::Call) -> Waiting) -> mind::sys::Result<()> {
        call.defer()?;
        self.waiting.push(make(call));
        Ok(())
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    // Drivers may start later (or not at all, without a card): each absent one is asked again every second.
    let mut stack = Box::new(Stack::new());
    let mut scratch: Box<[u8; socket::REQUEST_MAX]> = vec![0u8; socket::REQUEST_MAX].into_boxed_slice().try_into().unwrap();
    let mut reaped = 0u64;
    let mut policy = Policy::default();
    loop {
        let now = now_ms();
        stack.poll();
        if now - reaped >= 1000 { reaped = now; stack.reap(); }
        // Grants that ran out of time or volume lose their flows.
        for badge in policy.ended(now) { let closed = stack.close_badge(badge); mind::println!("[NETSTACK] GRANT {} ENDED, {} SOCKETS CLOSED", badge, closed); }
        let Ok(request) = Endpoint::SERVICE.recv_timeout(RECEIVED, POLL_MS) else { continue };
        let owner = (request.sender, request.badge);
        let (decoded, call) = match socket::decode(&request, RECEIVED, &mut scratch) { Ok(decoded) => decoded, Err(reason) => { if request.is_call { let _ = wire::reject(reason); } continue; } };
        let now = now_ms();
        let badge = owner.1;
        // Policy requests: only from the broker's control client.
        match decoded {
            socket::Request::PolicySet { badge: grant, rules, seconds, bytes } => {
                let result = if matches!(policy::access(badge), Access::Policy) { policy.set(grant, rules.as_slice(), seconds, bytes, now) } else { Err(Error::Denied) };
                let _ = socket::reply_policy_set(call, result); continue;
            }
            socket::Request::PolicyDrop { badge: grant } => {
                let result = if !matches!(policy::access(badge), Access::Policy) { Err(Error::Denied) } else if policy.drop(grant) { Ok(stack.close_badge(grant)) } else { Err(Error::NotFound) };
                let _ = socket::reply_policy_drop(call, result); continue;
            }
            socket::Request::PolicyUsage { badge: grant } => {
                let sockets = stack.sockets_of(grant);
                let usage = if matches!(policy::access(badge), Access::Policy) { policy.usage(grant, now, sockets).ok_or(Error::NotFound) } else { Err(Error::Denied) };
                let _ = socket::reply_policy_usage(call, usage.as_ref().map_err(|e| *e)); continue;
            }
            _ => {}
        }
        // Flow requests: the operator may do anything, a grant what its rules name, nobody else anything.
        let dns = stack.dns();
        let allowed = match policy::access(badge) {
            Access::Operator => Ok(()),
            Access::Policy => if matches!(decoded, socket::Request::Config) { Ok(()) } else { Err(Error::Denied) }, // the broker reads the DNS server
            Access::Grant(_) if matches!(decoded, socket::Request::Interfaces) => Ok(()),
            Access::Grant(_) if matches!(decoded, socket::Request::Offload { .. }) => Err(Error::Denied),
            Access::Nothing => Err(Error::Denied),
            Access::Grant(grant) => match &decoded {
                socket::Request::Config => Ok(()),
                socket::Request::Ping { address, .. } => policy.allows(grant, Protocol::Icmp, *address, 0, now),
                socket::Request::Resolve { server, port, .. } => policy.allows(grant, Protocol::Udp, if *server != 0 { *server } else { dns }, if *port != 0 { *port } else { 53 }, now),
                socket::Request::UdpOpen { .. } => policy.allows(grant, Protocol::Udp, 0, 0, now),
                socket::Request::UdpSend { address, port, .. } => policy.allows(grant, Protocol::Udp, *address, *port, now),
                socket::Request::TcpConnect { address, port, .. } => policy.allows(grant, Protocol::Tcp, *address, *port, now),
                socket::Request::Close { .. } => Ok(()),
                _ => policy.alive(grant, now),
            },
        };
        match allowed {
            Err(error) => { if matches!(error, Error::Denied) && matches!(policy::access(badge), Access::Grant(_)) { mind::println!("[NETSTACK] DENIED FOR GRANT {}", badge); } let _ = refuse(decoded, call, error); }
            Ok(()) => {
                let grant = match policy::access(badge) { Access::Grant(grant) => Some(grant), _ => None };
                let source = |address: u32, port: u16| grant.is_none_or(|g| policy.allows(g, Protocol::Udp, address, port, now).is_ok());
                let (sent, received) = stack.handle(decoded, call, owner, &source);
                if let Some(grant) = grant { policy.charge(grant, sent, received); }
                stack.poll();
            }
        }
    }
}

// Answers a request with `error` (no network card, or refused by the policy).
fn refuse(request: socket::Request, call: wire::Call, error: Error) -> mind::sys::Result<()> {
    match request {
        socket::Request::Config => socket::reply_config(call, Err(error)),
        socket::Request::Ping { .. } => socket::reply_ping(call, Err(error)),
        socket::Request::Resolve { .. } => socket::reply_resolve(call, Err(error)),
        socket::Request::UdpOpen { .. } => socket::reply_udp_open(call, Err(error)),
        socket::Request::UdpSend { .. } => socket::reply_udp_send(call, Err(error)),
        socket::Request::UdpReceive { .. } => socket::reply_udp_receive(call, Err(error)),
        socket::Request::TcpConnect { .. } => socket::reply_tcp_connect(call, Err(error)),
        socket::Request::TcpSend { .. } => socket::reply_tcp_send(call, Err(error)),
        socket::Request::TcpReceive { .. } => socket::reply_tcp_receive(call, Err(error)),
        socket::Request::Close { .. } => socket::reply_close(call, Err(error)),
        socket::Request::PolicySet { .. } => socket::reply_policy_set(call, Err(error)),
        socket::Request::PolicyDrop { .. } => socket::reply_policy_drop(call, Err(error)),
        socket::Request::PolicyUsage { .. } => socket::reply_policy_usage(call, Err(error)),
        socket::Request::Interfaces => socket::reply_interfaces(call, Err(error)),
        socket::Request::Offload { .. } => socket::reply_offload(call, Err(error)),
    }
}
