//! Production deadlines and smoltcp ingress scheduling with a silent peer and a never-empty NIC (550-NET-0031).
#![allow(dead_code)]

#[path = "../libmind/src/deadline.rs"]
mod deadline;
#[path = "../netstack/src/poll.rs"]
mod poll;

mod abi { pub const ERR_TIMEOUT: usize = 99; }
mod sys {
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum Error { Other(usize) }
    pub type Result<T> = core::result::Result<T, Error>;
}
mod time {
    use std::cell::Cell;
    thread_local! { pub static MS: Cell<u64> = const { Cell::new(0) }; }
    pub fn monotonic_ns() -> u64 { MS.get() * 1_000_000 }
}
mod idl {
    pub mod wire {
        use std::cell::Cell;
        thread_local! { pub static TIMEOUT: Cell<u32> = const { Cell::new(0) }; }
        pub fn with_timeout<T>(ms: u32, f: impl FnOnce() -> T) -> T {
            let before = TIMEOUT.replace(ms);
            let result = f();
            TIMEOUT.set(before);
            result
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadline::Deadline;
    use smoltcp::iface::{Config, Interface, SocketSet};
    use smoltcp::phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken};
    use smoltcp::socket::udp;
    use smoltcp::time::Instant;
    use smoltcp::wire::{EthernetAddress, HardwareAddress, IpCidr, IpEndpoint, Ipv4Address, Ipv4Cidr};

    fn delayed_reply(delay: u64) -> sys::Result<u64> {
        let timeout = idl::wire::TIMEOUT.get() as u64;
        assert!(timeout > 0, "a zero IPC timeout would wait forever");
        time::MS.set(time::MS.get() + delay.min(timeout));
        if delay > timeout { Err(sys::Error::Other(abi::ERR_TIMEOUT)) } else { Ok(delay) }
    }

    #[test]
    fn slow_calls_share_one_budget_and_an_expired_call_never_contacts_the_peer() {
        time::MS.set(10);
        let deadline = Deadline::after(10, 2000);
        assert_eq!(deadline.call(|| delayed_reply(700)), Ok(700));
        assert_eq!(deadline.call(|| delayed_reply(800)), Ok(800));
        assert_eq!(deadline.call(|| delayed_reply(900)), Err(sys::Error::Other(abi::ERR_TIMEOUT)));
        assert_eq!(time::MS.get(), 2010);
        assert_eq!(deadline.call::<()>(|| panic!("expired peer call")), Err(sys::Error::Other(abi::ERR_TIMEOUT)));
        assert_eq!(idl::wire::TIMEOUT.get(), 0, "restore the enclosing scope");
        let next = Deadline::after(time::MS.get(), 2000);
        assert_eq!(next.call(|| delayed_reply(12)), Ok(12), "a later turn can recover");
    }

    #[test]
    fn silent_peer_consumes_only_the_remaining_time_and_zero_never_means_unlimited() {
        time::MS.set(123);
        assert_eq!(Deadline::after(123, 0).call::<()>(|| panic!("zero budget")), Err(sys::Error::Other(abi::ERR_TIMEOUT)));
        let deadline = Deadline::after(123, 2000);
        time::MS.set(2122);
        assert_eq!(deadline.call(|| delayed_reply(u32::MAX as u64)), Err(sys::Error::Other(abi::ERR_TIMEOUT)));
        assert_eq!(time::MS.get(), 2123);
        assert_eq!(deadline.remaining(2123), None);
        assert_eq!(Deadline::after(u64::MAX - 1, 2000).remaining(u64::MAX - 1), Some(1));
    }

    struct Nic { flood: bool, received: usize, sent: Vec<Vec<u8>>, frame: [u8; 60], cost_ms: u64 }
    impl Nic {
        fn new(flood: bool, cost_ms: u64) -> Self {
            let mut frame = [0; 60];
            frame[..6].fill(255);
            frame[6..12].copy_from_slice(&[2, 0, 0, 0, 0, 1]);
            frame[12..14].copy_from_slice(&[0x88, 0xB5]);
            Self { flood, received: 0, sent: Vec::new(), frame, cost_ms }
        }
    }
    struct Rx<'a>(&'a [u8]);
    struct Tx<'a>(&'a mut Vec<Vec<u8>>);
    impl RxToken for Rx<'_> {
        fn consume<R, F: FnOnce(&[u8]) -> R>(self, f: F) -> R { f(self.0) }
    }
    impl TxToken for Tx<'_> {
        fn consume<R, F: FnOnce(&mut [u8]) -> R>(self, len: usize, f: F) -> R {
            let mut bytes = vec![0; len];
            let result = f(&mut bytes);
            self.0.push(bytes);
            result
        }
    }
    impl Device for Nic {
        type RxToken<'a> = Rx<'a>;
        type TxToken<'a> = Tx<'a>;
        fn receive(&mut self, _: Instant) -> Option<(Rx<'_>, Tx<'_>)> {
            if !self.flood { return None; }
            self.received += 1;
            assert!(self.received <= poll::BURST, "an ingress turn did not yield");
            time::MS.set(time::MS.get() + self.cost_ms);
            Some((Rx(&self.frame), Tx(&mut self.sent)))
        }
        fn transmit(&mut self, _: Instant) -> Option<Tx<'_>> { Some(Tx(&mut self.sent)) }
        fn capabilities(&self) -> DeviceCapabilities {
            let mut caps = DeviceCapabilities::default();
            caps.medium = Medium::Ethernet;
            caps.max_transmission_unit = 1514;
            caps
        }
    }

    fn configured(nic: &mut Nic) -> (Interface, SocketSet<'static>) {
        let cfg = Config::new(HardwareAddress::Ethernet(EthernetAddress([2, 0, 0, 0, 0, 2])));
        let mut iface = Interface::new(cfg, nic, Instant::ZERO);
        iface.update_ip_addrs(|addrs| { addrs.push(IpCidr::Ipv4(Ipv4Cidr::new(Ipv4Address::new(10, 0, 0, 2), 24))).unwrap(); });
        let mut socket = udp::Socket::new(
            udp::PacketBuffer::new(vec![udp::PacketMetadata::EMPTY; 2], vec![0; 128]),
            udp::PacketBuffer::new(vec![udp::PacketMetadata::EMPTY; 2], vec![0; 128]));
        socket.bind(31337).unwrap();
        socket.send_slice(b"progress", IpEndpoint::new(Ipv4Address::BROADCAST.into(), 31337)).unwrap();
        let mut sockets = SocketSet::new(Vec::new());
        sockets.add(socket);
        (iface, sockets)
    }

    #[test]
    fn continuous_ingress_yields_and_still_allows_egress_and_the_next_card() {
        time::MS.set(0);
        let mut flood = Nic::new(true, 0);
        let (mut iface, mut sockets) = configured(&mut flood);
        poll::interface(&mut iface, &mut flood, &mut sockets, Instant::ZERO, || true);
        assert_eq!(flood.received, poll::BURST);
        assert!(flood.sent.iter().any(|f| f.ends_with(b"progress")), "egress must survive continuous ingress");
        let mut next = Nic::new(false, 0);
        let (mut iface, mut sockets) = configured(&mut next);
        poll::interface(&mut iface, &mut next, &mut sockets, Instant::ZERO, || true);
        assert!(next.sent.iter().any(|f| f.ends_with(b"progress")), "the next card must get a turn");
    }

    #[test]
    fn a_slow_ingress_turn_stops_at_the_shared_deadline() {
        time::MS.set(0);
        let mut nic = Nic::new(true, 500);
        let (mut iface, mut sockets) = configured(&mut nic);
        let deadline = Deadline::after(0, 2000);
        poll::interface(&mut iface, &mut nic, &mut sockets, Instant::ZERO, || deadline.remaining(time::MS.get()).is_some());
        assert_eq!(nic.received, 4);
        assert_eq!(time::MS.get(), 2000);
        assert!(nic.sent.is_empty(), "no new egress after the budget expired");
        let attempted_timeout = std::cell::Cell::new(false);
        assert_eq!(deadline.call(|| {
            let result = delayed_reply(5000);
            if result.is_err() { attempted_timeout.set(true); }
            result
        }), Err(sys::Error::Other(abi::ERR_TIMEOUT)));
        assert!(!attempted_timeout.get(), "work-budget exhaustion is not a failed peer call");
    }
}
