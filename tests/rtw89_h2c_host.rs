#![allow(dead_code)]
extern crate self as rtw89_wifi;
#[path = "../rtw89_wifi/src/io.rs"] pub mod io;
#[path = "../rtw89_wifi/src/registers.rs"] mod registers;
#[path = "../rtw89_wifi/src/power.rs"] mod power;
#[path = "../rtw89_wifi/src/efuse.rs"] mod efuse;
#[path = "../rtw89_wifi/src/board.rs"] mod board;
#[path = "../rtw89_wifi/src/fwcmd.rs"] pub mod fwcmd;
#[path = "../rtw89_wifi/src/mac.rs"] mod mac;
#[path = "../rtw89_wifi/src/pcie.rs"] mod pcie;
#[path = "../rtw89_wifi/src/startup.rs"] mod startup;
#[path = "../rtw89_wifi/src/c2h.rs"] mod c2h;
#[path = "../rtw89_wifi/src/h2c.rs"] mod h2c;
#[path = "rtw89_pipeline_device.rs"] mod model;

use c2h::{Ack, Identity};
use h2c::{Acks, Command, Engine, Error, Failure, Observation, Outcome, Progress};
const COMMAND: Command = Command { category: 1, class: 8, function: 23 };
const NONE: Acks = Acks { received: false, done: false };
const BOTH: Acks = Acks { received: true, done: true };

#[derive(Default)]
struct Sender { now: u64, step: u64, error: Option<fwcmd::Error>, packets: Vec<Vec<u8>>, reads: usize, jump: Option<(usize, u64)>, budget: Option<u32>, after_submit: Option<u64> }
impl h2c::Transport for Sender {
    fn now_us(&mut self) -> u64 {
        self.reads += 1;
        if let Some((at, value)) = self.jump { if at == self.reads { self.now = value; } }
        self.now
    }
    fn submit(&mut self, packet: &[u8], budget: u32) -> Result<(), fwcmd::Error> {
        self.budget = Some(budget);
        self.now += self.step;
        if let Some(value) = self.after_submit { self.now = value; }
        if let Some(error) = self.error { return Err(error); }
        self.packets.push(packet.to_vec()); Ok(())
    }
}
fn done(ticket: h2c::Ticket, status: u8) -> Ack { Ack::Done { command: ticket.identity(), status } }
fn ack_packet(ticket: h2c::Ticket, completed: bool, status: u8) -> Vec<u8> {
    let id = ticket.identity(); let mut bytes = vec![0; 12];
    bytes[..4].copy_from_slice(&(1u32 | u32::from(completed) << 8).to_le_bytes());
    bytes[4..8].copy_from_slice(&12u32.to_le_bytes());
    let w2 = u32::from(id.category) | u32::from(id.class) << 2 | u32::from(id.function) << 8
        | if completed { u32::from(status) << 16 | u32::from(id.sequence) << 24 } else { u32::from(id.sequence) << 16 };
    bytes[8..].copy_from_slice(&w2.to_le_bytes()); bytes
}

#[test]
fn normal_header_length_payload_and_every_fourth_receipt_follow_pinned_source() {
    let mut engine = Engine::new(0); let mut sender = Sender::default();
    for sequence in 0..6u8 {
        let ticket = engine.submit(&mut sender, COMMAND, &[7, 8, 9], NONE, 100).unwrap();
        let packet = sender.packets.last().unwrap();
        assert_eq!(&packet[..4], &[0x21, 23, 0, sequence]);
        assert_eq!(&packet[4..], if sequence % 4 == 0 { &[11, 0x40, 0, 0, 7, 8, 9] } else { &[11, 0, 0, 0, 7, 8, 9] });
        if sequence % 4 == 0 { engine.acknowledge(0, Ack::Received(ticket.identity())).unwrap(); }
        assert_eq!(engine.take(0, ticket).unwrap(), Some(if sequence % 4 == 0 { Outcome::Received } else { Outcome::Submitted }));
    }
}

#[test]
fn completion_and_receipt_are_separate_even_if_done_arrives_first() {
    let mut engine = Engine::new(0); let mut sender = Sender::default();
    let ticket = engine.submit(&mut sender, COMMAND, &[], BOTH, 100).unwrap();
    assert_eq!(engine.acknowledge(1, done(ticket, 0)), Ok(Observation::Progress));
    assert_eq!(engine.progress(1, ticket), Ok(Progress::Pending { received: false, done: true }));
    assert_eq!(engine.acknowledge(2, done(ticket, 0)), Ok(Observation::Duplicate));
    assert_eq!(engine.acknowledge(3, Ack::Received(ticket.identity())), Ok(Observation::Complete));
    assert_eq!(engine.take(3, ticket), Ok(Some(Outcome::Done)));
    assert_eq!(engine.acknowledge(4, done(ticket, 0)), Ok(Observation::Unrelated));
}

#[test]
fn unsolicited_done_and_each_mismatched_identity_field_cannot_complete() {
    let mut engine = Engine::new(0); let mut sender = Sender::default();
    let ticket = engine.submit(&mut sender, COMMAND, &[], NONE, 100).unwrap();
    assert_eq!(engine.acknowledge(1, done(ticket, 0)), Ok(Observation::Unexpected));
    let id = ticket.identity();
    for wrong in [Identity { category: 0, ..id }, Identity { class: 9, ..id }, Identity { function: 24, ..id }, Identity { sequence: 1, ..id }] {
        assert_eq!(engine.acknowledge(2, Ack::Received(wrong)), Ok(Observation::Unrelated));
    }
    assert_eq!(engine.take(2, ticket), Ok(None));
    assert_eq!(engine.acknowledge(3, Ack::Received(id)), Ok(Observation::Complete));
    assert_eq!(engine.take(3, ticket), Ok(Some(Outcome::Received)));
}

#[test]
fn firmware_rejection_is_reported_without_false_success_or_resend() {
    let mut engine = Engine::new(0); let mut sender = Sender::default();
    let ticket = engine.submit(&mut sender, COMMAND, &[], BOTH, 100).unwrap();
    assert_eq!(engine.acknowledge(1, done(ticket, 0x81)), Ok(Observation::Complete));
    assert_eq!(engine.take(1, ticket), Ok(Some(Outcome::Rejected(0x81))));
    assert_eq!(sender.packets.len(), 1); assert_eq!(engine.failed(), None);
}

#[test]
fn full_transport_and_full_pending_table_do_not_advance_sequence() {
    let mut engine = Engine::new(0); let mut sender = Sender { error: Some(fwcmd::Error::Full), ..Sender::default() };
    assert_eq!(engine.submit(&mut sender, COMMAND, &[], BOTH, 100), Err(Error::Full));
    assert_eq!(engine.submitted(), 0); assert_eq!(engine.failed(), None);
    sender.error = None;
    for _ in 0..h2c::PENDING { engine.submit(&mut sender, COMMAND, &[], BOTH, 100).unwrap(); }
    assert_eq!(engine.submit(&mut sender, COMMAND, &[], BOTH, 100), Err(Error::Full));
    assert_eq!(engine.submitted(), h2c::PENDING as u16); assert_eq!(sender.packets.len(), h2c::PENDING);
}

#[test]
fn uncertain_publication_poisoning_refuses_all_further_commands() {
    let cause = fwcmd::Error::Io(io::Fault::new(io::FaultKind::Access, fwcmd::INDEX, 0));
    let mut engine = Engine::new(0); let mut sender = Sender { error: Some(cause), ..Sender::default() };
    assert_eq!(engine.submit(&mut sender, COMMAND, &[], BOTH, 100), Err(Error::Failed(Failure::Transport(cause))));
    sender.error = None;
    assert_eq!(engine.submit(&mut sender, COMMAND, &[], BOTH, 100), Err(Error::Failed(Failure::Transport(cause))));
    assert!(sender.packets.is_empty());
}

#[test]
fn exact_deadline_or_late_ack_poison_without_reusing_sequence() {
    let mut engine = Engine::new(0); let mut sender = Sender::default();
    let ticket = engine.submit(&mut sender, COMMAND, &[], BOTH, 100).unwrap();
    assert_eq!(engine.acknowledge(100, done(ticket, 0)), Err(Error::Failed(Failure::Timeout)));
    assert_eq!(engine.take(100, ticket), Err(Error::Failed(Failure::Timeout)));
    assert_eq!(engine.submitted(), 1);
}

#[test]
fn final_transport_overrun_and_older_pending_deadline_are_observed() {
    let mut engine = Engine::new(0); let mut sender = Sender { step: 100, ..Sender::default() };
    assert_eq!(engine.submit(&mut sender, COMMAND, &[], NONE, 100), Err(Error::Failed(Failure::Timeout)));
    assert_eq!(engine.submitted(), 1); assert_eq!(sender.packets.len(), 1);
    let mut engine = Engine::new(0); let mut sender = Sender::default();
    engine.submit(&mut sender, COMMAND, &[], BOTH, 10).unwrap();
    sender.step = 10;
    assert_eq!(engine.submit(&mut sender, COMMAND, &[], NONE, 100), Err(Error::Failed(Failure::Timeout)));
    assert_eq!(engine.submitted(), 2);
}

#[test]
fn prepublication_clock_advance_refuses_late_send_or_reduces_transport_budget() {
    let mut engine = Engine::new(0); let mut sender = Sender { jump: Some((2, 100)), ..Sender::default() };
    assert_eq!(engine.submit(&mut sender, COMMAND, &[4; h2c::MAX_BODY], BOTH, 100), Err(Error::Failed(Failure::Timeout)));
    assert!(sender.packets.is_empty()); assert_eq!(sender.budget, None); assert_eq!(engine.submitted(), 0);
    let mut engine = Engine::new(0); let mut sender = Sender { jump: Some((2, 40)), ..Sender::default() };
    engine.submit(&mut sender, COMMAND, &[], BOTH, 100).unwrap();
    assert_eq!(sender.budget, Some(60));
    let mut engine = Engine::new(10); let mut sender = Sender { now: 10, jump: Some((2, 9)), ..Sender::default() };
    assert_eq!(engine.submit(&mut sender, COMMAND, &[], BOTH, 100), Err(Error::Failed(Failure::Clock)));
    assert!(sender.packets.is_empty());
    let mut engine = Engine::new(0);
    let mut sender = Sender { jump: Some((2, 40)), after_submit: Some(30), ..Sender::default() };
    assert_eq!(engine.submit(&mut sender, COMMAND, &[], BOTH, 100), Err(Error::Failed(Failure::Clock)));
    assert_eq!(sender.packets.len(), 1);
}

#[test]
fn clock_regression_and_deadline_overflow_fail_without_sending() {
    let mut engine = Engine::new(10); let mut sender = Sender::default();
    assert_eq!(engine.submit(&mut sender, COMMAND, &[], NONE, 100), Err(Error::Failed(Failure::Clock)));
    assert!(sender.packets.is_empty());
    let mut engine = Engine::new(u64::MAX); sender.now = u64::MAX;
    assert_eq!(engine.submit(&mut sender, COMMAND, &[], NONE, 1), Err(Error::Request));
    assert!(sender.packets.is_empty());
}

#[test]
fn invalid_request_or_payload_never_reaches_transport() {
    let mut engine = Engine::new(0); let mut sender = Sender::default();
    for command in [Command { category: 4, ..COMMAND }, Command { class: 64, ..COMMAND }] {
        assert_eq!(engine.submit(&mut sender, command, &[], NONE, 100), Err(Error::Request));
    }
    for timeout in [0, h2c::MAX_TIMEOUT_US + 1] { assert_eq!(engine.submit(&mut sender, COMMAND, &[], NONE, timeout), Err(Error::Request)); }
    assert_eq!(engine.submit(&mut sender, COMMAND, &[0; h2c::MAX_BODY + 1], NONE, 100), Err(Error::Request));
    assert!(sender.packets.is_empty()); assert_eq!(engine.submitted(), 0);
    engine.submit(&mut sender, COMMAND, &[9; h2c::MAX_BODY], NONE, 100).unwrap();
    assert_eq!(sender.packets[0].len(), fwcmd::MAX_PAYLOAD);
}

#[test]
fn all_256_unique_sequences_are_usable_but_wrap_is_explicitly_refused() {
    let mut engine = Engine::new(0); let mut sender = Sender::default();
    for sequence in 0..256 {
        let ticket = engine.submit(&mut sender, COMMAND, &[], BOTH, 100).unwrap();
        assert_eq!(ticket.identity().sequence, sequence as u8);
        engine.acknowledge(0, Ack::Received(ticket.identity())).unwrap(); engine.acknowledge(0, done(ticket, 0)).unwrap();
        assert_eq!(engine.take(0, ticket), Ok(Some(Outcome::Done)));
    }
    assert_eq!(engine.submit(&mut sender, COMMAND, &[], BOTH, 100), Err(Error::Exhausted));
    assert_eq!(sender.packets.len(), 256);
}

#[test]
fn production_dma_adapter_and_c2h_parser_share_the_correlated_header_contract() {
    let mut device = model::Device::new(1); model::read_board(&mut device);
    let mut region = model::Region::new(); let mut queue = model::prepare(&mut device, &mut region);
    let mut engine = Engine::new(device.now);
    let ticket = engine.submit(&mut h2c::CommandQueue { bus: &mut device, queue: &mut queue }, COMMAND, &[3, 2, 1], BOTH, 100).unwrap();
    assert_eq!(device.packets.len(), 1); assert!(!device.packets[0].0);
    assert_eq!(&device.packets[0].1, &[0x21, 23, 0, 0, 11, 0xc0, 0, 0, 3, 2, 1]);
    assert_eq!(engine.progress(device.now, ticket), Ok(Progress::Pending { received: false, done: false }));
    for complete in [false, true] {
        let bytes = ack_packet(ticket, complete, 0);
        engine.acknowledge(device.now, c2h::parse(&bytes).unwrap().ack().unwrap().unwrap()).unwrap();
    }
    assert_eq!(engine.take(device.now, ticket), Ok(Some(Outcome::Done)));
    // The fake consumes doorbells synchronously; model disabling the device before releasing its allocation.
    device.command = 0;
}
