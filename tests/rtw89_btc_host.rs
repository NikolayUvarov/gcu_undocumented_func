#![allow(dead_code)]
#[path = "../rtw89_wifi/src/io.rs"] mod io;
#[path = "../rtw89_wifi/src/btc.rs"] mod btc;
use io::{Fault, FaultKind, RegisterIo};

struct Device { now: u64, busy: u32, clear_at: u64, reads: usize, read_cost: u64, stalled: bool, backwards: bool }
impl Device {
    fn new(busy: u32, clear_at: u64) -> Self { Self { now: 0, busy, clear_at, reads: 0, read_cost: 0, stalled: false, backwards: false } }
}
impl RegisterIo for Device {
    fn read32(&mut self, at: usize) -> io::Result<u32> {
        assert_eq!(at, btc::SCOREBOARD); self.reads += 1;
        self.now = if self.backwards { 0 } else { self.now + self.read_cost };
        Ok(if self.now >= self.clear_at { 0x4001 } else { self.busy })
    }
    fn now_us(&mut self) -> u64 { self.now }
    fn delay_us(&mut self, us: u32) -> io::Result<()> { if !self.stalled { self.now += u64::from(us); } Ok(()) }
    fn read8(&mut self, _: usize) -> io::Result<u8> { panic!("unexpected read8") }
    fn read16(&mut self, _: usize) -> io::Result<u16> { panic!("unexpected read16") }
    fn write8(&mut self, _: usize, _: u8) -> io::Result<()> { panic!("arbitration observation must not write") }
    fn write16(&mut self, _: usize, _: u16) -> io::Result<()> { panic!("arbitration observation must not write") }
    fn write32(&mut self, _: usize, _: u32) -> io::Result<()> { panic!("arbitration observation must not write") }
}

#[test]
fn active_request_and_run_bits_must_each_clear_before_snapshot() {
    for busy in [btc::BT_RFK_RUN, btc::BT_RFK_REQUEST, btc::BT_RFK_RUN | btc::BT_RFK_REQUEST] {
        let mut d = Device::new(busy, 80);
        assert_eq!(btc::wait_bt_clear(&mut d, 100).unwrap().raw(), 0x4001);
        assert_eq!(d.reads, 3); assert_eq!(d.now, 80);
    }
}
#[test]
fn unavailable_read_does_not_fall_back_to_bt_disabled() {
    let mut d = Device::new(u32::MAX, u64::MAX);
    assert_eq!(btc::wait_bt_clear(&mut d, 100).unwrap_err().kind, FaultKind::Unavailable);
    assert_eq!(d.reads, 1);
}
#[test]
fn busy_bt_times_out_without_inventing_timeout_report_override() {
    let mut d = Device::new(btc::BT_RFK_RUN, u64::MAX);
    assert_eq!(btc::wait_bt_clear(&mut d, 100).unwrap_err().kind, FaultKind::Timeout);
    assert_eq!(d.now, 100); assert_eq!(d.reads, 3);
}
#[test]
fn slow_final_read_cannot_return_late_clear_snapshot() {
    let mut d = Device::new(0, 0); d.read_cost = 100;
    assert_eq!(btc::wait_bt_clear(&mut d, 100).unwrap_err().kind, FaultKind::Timeout);
}
#[test]
fn stuck_or_backward_clock_still_terminates_with_error() {
    let mut d = Device::new(btc::BT_RFK_REQUEST, u64::MAX); d.stalled = true;
    assert_eq!(btc::wait_bt_clear(&mut d, 100).unwrap_err().kind, FaultKind::Timeout);
    assert!(d.reads <= 4);
    let mut d = Device::new(0, 0); d.now = 50; d.backwards = true;
    assert_eq!(btc::wait_bt_clear(&mut d, 100).unwrap_err().kind, FaultKind::Clock);
}
#[test]
fn invalid_budget_is_rejected_before_hardware_access() {
    for budget in [0, btc::MAX_WAIT_US + 1] {
        let mut d = Device::new(0, 0);
        assert_eq!(btc::wait_bt_clear(&mut d, budget), Err(Fault::new(FaultKind::Access, btc::SCOREBOARD, budget)));
        assert_eq!(d.reads, 0);
    }
}
