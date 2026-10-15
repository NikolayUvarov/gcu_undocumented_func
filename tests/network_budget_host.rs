//! Production deadlines and smoltcp ingress scheduling with a silent peer and a never-empty NIC (550-NET-0031).
#![allow(dead_code)]

#[path = "../libmind/src/deadline.rs"]
mod deadline;
#[path = "../netstack/src/poll.rs"]
mod poll;
#[path = "../netstack/src/turn.rs"]
mod turn;
#[path = "../wlan/src/service_turn.rs"]
mod wlan_turn;

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

    #[test]
    fn wlan_accepted_cached_info_and_queued_receive_precede_background_work() {
        for request in ["cached-info", "queued-receive"] {
            time::MS.set(0);
            let mut events = Vec::new();
            wlan_turn::run(&mut events, request,
                |events, request| events.push((request, time::MS.get())),
                |_| true,
                |events| wlan_turn::receive_batch(events, || time::MS.get(), |events| {
                    time::MS.set(time::MS.get() + 10);
                    events.push(("radio-receive", time::MS.get()));
                    true
                }));
            assert_eq!(events[0], (request, 0), "an accepted reply must not sit behind the radio queue");
            assert_eq!(events.len(), 1 + wlan_turn::RECEIVE_FRAMES);
            assert_eq!(time::MS.get(), 80);
        }
    }

    #[test]
    fn wlan_client_arriving_during_continuous_ten_ms_frames_gets_the_next_dispatch() {
        time::MS.set(0);
        let mut arrivals = Vec::new();
        wlan_turn::run(&mut arrivals, None::<u64>, |_, request| assert!(request.is_none()), |_| true,
            |arrivals| wlan_turn::receive_batch(arrivals, || time::MS.get(), |arrivals| {
                time::MS.set(time::MS.get() + 10);
                if arrivals.is_empty() { arrivals.push(time::MS.get()); }
                true
            }));
        assert_eq!(arrivals, [10]);
        assert_eq!(time::MS.get(), 80, "the old 64-frame batch would still be busy");
        let request = arrivals.pop();
        wlan_turn::run(&mut arrivals, request, |replies, arrival| {
            assert_eq!(arrival, Some(10));
            replies.push(time::MS.get());
        }, |_| true, |replies| {
            assert_eq!(replies, &[80], "dispatch must precede a second never-empty batch");
        });
        assert_eq!(arrivals, [80]);
    }

    #[test]
    fn wlan_slice_is_checked_before_each_frame_but_a_started_frame_can_finish() {
        time::MS.set(0);
        let mut starts = Vec::new();
        wlan_turn::receive_batch(&mut starts, || time::MS.get(), |starts| {
            starts.push(time::MS.get());
            time::MS.set(time::MS.get() + 30);
            true
        });
        assert_eq!(starts, [0, 30, 60, 90]);
        assert_eq!(time::MS.get(), 120, "a cooperative slice permits the final frame's overrun");
    }

    #[test]
    fn wlan_zero_cost_stream_still_yields_after_eight_frames() {
        let mut frames = 0;
        wlan_turn::receive_batch(&mut frames, || 0, |frames| { *frames += 1; true });
        assert_eq!(frames, 8, "the count limit also bounds submillisecond frames");
    }

    #[test]
    fn wlan_empty_radio_ends_the_batch_without_retries() {
        let mut calls = 0;
        wlan_turn::receive_batch(&mut calls, || 0, |calls| { *calls += 1; false });
        assert_eq!(calls, 1);
    }

    #[test]
    fn wlan_long_frame_actions_keep_the_original_shared_deadline() {
        time::MS.set(0);
        let deadline = Deadline::after(0, 2000);
        let mut actions = Vec::new();
        wlan_turn::run(&mut actions, (), |actions, ()| {
            assert_eq!(deadline.call(|| delayed_reply(600)), Ok(600));
            actions.push("client");
        }, |_| deadline.remaining(time::MS.get()).is_some(), |actions| {
            wlan_turn::receive_batch(actions, || time::MS.get(), |actions| {
                assert_eq!(deadline.call(|| delayed_reply(700)), Ok(700));
                actions.push("received");
                assert_eq!(deadline.call(|| delayed_reply(500)), Ok(500));
                actions.push("applied");
                true
            });
        });
        assert_eq!(actions, ["client", "received", "applied"], "a new short IPC deadline would lose the frame action");
        assert_eq!(time::MS.get(), 1800);
        assert_eq!(deadline.remaining(time::MS.get()), Some(200), "dispatch and pump cannot each get a fresh two seconds");
    }

    #[test]
    fn wlan_handler_exhaustion_skips_background_without_advancing_station_state() {
        time::MS.set(0);
        let deadline = Deadline::after(0, 2000);
        wlan_turn::run(&mut (), (), |_, ()| {
            assert_eq!(deadline.call(|| delayed_reply(2000)), Ok(2000));
        }, |_| deadline.remaining(time::MS.get()).is_some(), |_| panic!("expired handler must not start receive or station tick"));
        assert_eq!(time::MS.get(), 2000);
    }

    #[test]
    fn wlan_clock_reversal_does_not_extend_the_receive_slice() {
        let readings = std::cell::Cell::new(200);
        let mut calls = 0;
        wlan_turn::receive_batch(&mut calls, || readings.get(), |calls| {
            *calls += 1;
            readings.set(199);
            true
        });
        assert_eq!(calls, 1);
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

    fn run_turn(turn: &mut turn::Turn<3>, delays: [u64; 3], visits: &mut Vec<(usize, u64, sys::Result<u64>)>) {
        turn.begin(time::MS.get());
        while let Some(visit) = turn.next(time::MS.get()) {
            let deadline = Deadline::after(visit.started_ms, visit.remaining_ms);
            let result = deadline.call(|| delayed_reply(delays[visit.index]));
            visits.push((visit.index, visit.started_ms, result));
            turn.finish(time::MS.get());
        }
    }

    #[test]
    fn two_silent_peers_cannot_starve_healthy_last_card_or_client_opportunities() {
        time::MS.set(0);
        let mut turn = turn::Turn::<3>::new();
        let mut visits = Vec::new();
        let mut clients = Vec::new();
        for _ in 0..4 {
            let start = time::MS.get();
            run_turn(&mut turn, [10_000, 10_000, 1], &mut visits);
            assert!(time::MS.get() - start <= turn::TURN_MS as u64);
            clients.push(time::MS.get());
        }
        assert_eq!(visits[0], (0, 0, Err(sys::Error::Other(abi::ERR_TIMEOUT))));
        assert_eq!(visits[1], (1, 250, Err(sys::Error::Other(abi::ERR_TIMEOUT))));
        assert_eq!(visits[2], (2, 500, Ok(1)), "the healthy last card runs by the third turn");
        assert_eq!(clients, [250, 500, 750, 1000], "each turn returns to client handling");
        assert_eq!(visits[3].0, 0);
        assert_eq!(visits[3].1, 501, "short timeout never adds a five-second cooldown");
    }

    #[test]
    fn three_slow_successful_peers_share_the_total_budget_without_restarting_at_zero() {
        time::MS.set(0);
        let mut turn = turn::Turn::<3>::new();
        let mut visits = Vec::new();
        for expected in [0, 1, 2, 0, 1, 2] {
            let before = visits.len();
            run_turn(&mut turn, [250; 3], &mut visits);
            assert_eq!(visits.len(), before + 1);
            assert_eq!(visits[before].0, expected);
            assert_eq!(visits[before].2, Ok(250), "successful slow replies must also yield");
        }
        assert_eq!(time::MS.get(), 1500);
    }

    #[test]
    fn later_cards_inherit_only_remaining_time_and_no_call_is_given_zero() {
        time::MS.set(1000);
        let mut turn = turn::Turn::<3>::new();
        let mut seen = Vec::new();
        turn.begin(time::MS.get());
        while let Some(visit) = turn.next(time::MS.get()) {
            let deadline = Deadline::after(visit.started_ms, visit.remaining_ms);
            let result = deadline.call(|| {
                seen.push((visit.index, idl::wire::TIMEOUT.get()));
                delayed_reply(if visit.index == 0 { 200 } else { 500 })
            });
            if visit.index == 1 { assert_eq!(result, Err(sys::Error::Other(abi::ERR_TIMEOUT))); }
            turn.finish(time::MS.get());
        }
        assert_eq!(seen, [(0, 250), (1, 50)]);
        assert_eq!(time::MS.get(), 1250);
        assert_eq!(turn.next(1250), None);
        turn.begin(1250);
        assert_eq!(turn.next(1250).unwrap().index, 2);
    }

    #[test]
    fn a_peer_recovers_on_its_next_fair_visit_after_short_timeouts() {
        time::MS.set(0);
        let mut turn = turn::Turn::<3>::new();
        let mut visits = Vec::new();
        run_turn(&mut turn, [10_000; 3], &mut visits);
        run_turn(&mut turn, [10_000; 3], &mut visits);
        run_turn(&mut turn, [10_000; 3], &mut visits);
        run_turn(&mut turn, [2; 3], &mut visits);
        assert_eq!(&visits[3..], &[(0, 750, Ok(2)), (1, 752, Ok(2)), (2, 754, Ok(2))]);
    }

    #[test]
    fn fast_cards_idle_slots_and_saturated_clock_still_have_finite_turns() {
        let mut turn = turn::Turn::<3>::new();
        turn.begin(0);
        let mut slots = Vec::new();
        while let Some(visit) = turn.next(0) { slots.push(visit.index); turn.finish(0); }
        assert_eq!(slots, [0, 1, 2], "idle slots cannot cycle forever without consuming time");
        turn.begin(u64::MAX - 1);
        assert_eq!(turn.next(u64::MAX - 1).unwrap().remaining_ms, 1);
        turn.finish(u64::MAX);
        assert_eq!(turn.next(u64::MAX), None);
        turn.begin(u64::MAX);
        assert_eq!(turn.next(u64::MAX), None);
        let mut empty = turn::Turn::<0>::new();
        empty.begin(0);
        assert_eq!(empty.next(0), None);
    }

    #[test]
    fn slow_continuous_ingress_yields_the_next_turn_to_another_card() {
        time::MS.set(0);
        let mut turn = turn::Turn::<3>::new();
        let mut nic = Nic::new(true, 50);
        let (mut iface, mut sockets) = configured(&mut nic);
        turn.begin(0);
        let visit = turn.next(0).unwrap();
        let deadline = Deadline::after(visit.started_ms, visit.remaining_ms);
        poll::interface(&mut iface, &mut nic, &mut sockets, Instant::ZERO, || deadline.remaining(time::MS.get()).is_some());
        turn.finish(time::MS.get());
        assert_eq!(nic.received, 5);
        assert_eq!(turn.next(time::MS.get()), None);
        turn.begin(time::MS.get());
        assert_eq!(turn.next(time::MS.get()).unwrap().index, 1);
    }

    #[test]
    fn a_silent_card_spends_only_two_seconds_until_rolling_charges_expire() {
        time::MS.set(0);
        let mut turn = turn::Turn::<3>::new();
        let mut visits = Vec::new();
        for _ in 0..8 { run_turn(&mut turn, [10_000, 0, 0], &mut visits); }
        assert_eq!(time::MS.get(), 2000);
        assert_eq!(visits.iter().filter(|v| v.0 == 0).count(), 8);
        for now in [2000, 3000, 4999, 5000, 5499] {
            time::MS.set(now);
            let start = visits.len();
            run_turn(&mut turn, [10_000, 0, 0], &mut visits);
            assert!(visits[start..].iter().all(|v| v.0 != 0));
            assert!(visits[start..].iter().any(|v| v.0 == 2), "healthy last card stays eligible");
        }
        time::MS.set(5500);
        let start = visits.len();
        run_turn(&mut turn, [10_000, 0, 0], &mut visits);
        assert!(visits[start..].iter().any(|v| v.0 == 0));
        assert_eq!(time::MS.get(), 5750);
    }

    #[test]
    fn per_card_remainder_shortens_the_turn_and_success_does_not_reset_the_quota() {
        time::MS.set(0);
        let mut turn = turn::Turn::<3>::new();
        let mut visits = Vec::new();
        for _ in 0..8 { run_turn(&mut turn, [240, 0, 0], &mut visits); }
        assert_eq!(time::MS.get(), 1920);
        let start = visits.len();
        run_turn(&mut turn, [240, 0, 0], &mut visits);
        assert_eq!(time::MS.get(), 2000, "only the 80 ms card allowance remains");
        assert_eq!(visits[start].2, Err(sys::Error::Other(abi::ERR_TIMEOUT)));
        let start = visits.len();
        run_turn(&mut turn, [1, 1, 1], &mut visits);
        assert!(visits[start..].iter().all(|v| v.0 != 0));
    }

    #[test]
    fn live_wlan_finishing_after_two_seconds_is_not_marked_dead_and_recovers_when_charges_age() {
        time::MS.set(0);
        let mut turn = turn::Turn::<3>::new();
        let mut visits = Vec::new();
        for _ in 0..8 { run_turn(&mut turn, [10_000, 0, 0], &mut visits); }
        time::MS.set(2001);
        let before = visits.len();
        run_turn(&mut turn, [1, 1, 1], &mut visits);
        assert!(visits[before..].iter().all(|v| v.0 != 0), "ready WLAN still respects its work quota");
        time::MS.set(5500);
        let before = visits.len();
        run_turn(&mut turn, [1, 1, 1], &mut visits);
        assert!(visits[before..].iter().any(|v| v.0 == 0 && v.2 == Ok(1)), "eligibility is restored when old work charges expire, not failure time plus five seconds");
    }

    #[test]
    fn unfinished_visit_is_charged_before_another_turn_and_idle_time_does_not_accumulate_credit() {
        let mut turn = turn::Turn::<1>::new();
        turn.begin(0);
        assert_eq!(turn.next(0).unwrap().remaining_ms, 250);
        assert_eq!(turn.next(1), None, "a second visit cannot bypass accounting");
        turn.begin(2000);
        assert_eq!(turn.next(2000), None, "the unfinished visit consumed the card quota");
        turn.begin(50_000);
        assert_eq!(turn.next(50_000).unwrap().remaining_ms, 250);
        turn.finish(52_000);
        turn.begin(52_000);
        assert_eq!(turn.next(52_000), None, "idle time cannot bank future allowance");
    }

    #[test]
    fn a_fixed_window_boundary_cannot_double_the_five_second_allowance() {
        time::MS.set(0);
        let mut turn = turn::Turn::<3>::new();
        let mut visits = Vec::new();
        run_turn(&mut turn, [0; 3], &mut visits);
        time::MS.set(3000);
        for _ in 0..8 { run_turn(&mut turn, [10_000, 0, 0], &mut visits); }
        assert_eq!(time::MS.get(), 5000);
        for now in [5000, 6000, 7000, 8499] {
            time::MS.set(now);
            let before = visits.len();
            run_turn(&mut turn, [10_000, 0, 0], &mut visits);
            assert!(visits[before..].iter().all(|v| v.0 != 0), "there is no fresh two-second budget at a fixed boundary");
        }
        time::MS.set(8500);
        let before = visits.len();
        run_turn(&mut turn, [10_000, 0, 0], &mut visits);
        assert!(visits[before..].iter().any(|v| v.0 == 0));
    }

    #[test]
    fn cheap_frequent_healthy_visits_keep_running_beside_an_exhausted_card() {
        time::MS.set(0);
        let mut turn = turn::Turn::<3>::new();
        let mut visits = Vec::new();
        for _ in 0..8 { run_turn(&mut turn, [10_000, 0, 0], &mut visits); }
        let before = visits.len();
        for _ in 0..1000 { run_turn(&mut turn, [10_000, 1, 1], &mut visits); }
        assert_eq!(time::MS.get(), 4000);
        assert!(visits[before..].iter().all(|v| v.0 != 0));
        assert_eq!(visits[before..].iter().filter(|v| v.0 == 2 && v.2 == Ok(1)).count(), 1000);
    }

    #[test]
    fn completion_buckets_retain_work_for_at_least_five_and_at_most_five_and_a_half_seconds() {
        let mut turn = turn::Turn::<1>::new();
        turn.begin(0);
        turn.next(0).unwrap();
        turn.finish(2000);
        turn.begin(7000);
        assert_eq!(turn.next(7000), None, "even an exact boundary completion is retained conservatively");
        turn.begin(7499);
        assert_eq!(turn.next(7499), None);
        turn.begin(7500);
        assert!(turn.next(7500).is_some());
    }

    #[test]
    fn a_backwards_or_wrapped_clock_fails_closed_instead_of_renewing_allowance() {
        let mut turn = turn::Turn::<1>::new();
        turn.begin(100);
        turn.next(100).unwrap();
        turn.finish(99);
        turn.begin(10_000);
        assert_eq!(turn.next(10_000), None);
        let mut wrapped = turn::Turn::<1>::new();
        wrapped.begin(u64::MAX - 1);
        wrapped.next(u64::MAX - 1).unwrap();
        wrapped.finish(0);
        wrapped.begin(5000);
        assert_eq!(wrapped.next(5000), None);
    }
}
