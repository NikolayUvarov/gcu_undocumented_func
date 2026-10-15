//! Production session events with a fake clock, delayed transport and durable-store confirmations (750-EFF-0006).
#![allow(dead_code)]
#[path = "../libmind/src/sha256.rs"] mod sha256;
#[path = "../effector/src/session.rs"] mod session;
use session::*;

fn time(ms: u64) -> Time { Time { monotonic_ms: ms, unix_seconds: Some(1000 + ms / 1000) } }
fn command(id: &str) -> Command<'_> { Command { id, agent_id: "mind-test", action: "exec_v1", program: "test", payload: "{\"command\":\"echo forbidden\"}", expires_unix: None } }
fn online(records: &[Record]) -> Session {
    let mut s = Session::new("mind-test", records, 0).unwrap();
    assert_eq!(s.poll(0), Ok(None));
    let hb = s.heartbeat(0).unwrap().unwrap();
    s.heartbeat_done(hb, Outcome::Accepted, 0, 0).unwrap(); s
}
fn store(s: &mut Session, c: Command<'_>, at: Time) -> Record {
    let Accepted::Store(request) = s.receive(c, at).unwrap() else { panic!("expected durable write") };
    assert_eq!(s.next_ack(at), Ok(None), "result cannot be sent before durable confirmation");
    s.stored(request, true).unwrap(); request.record
}
fn acknowledge(s: &mut Session, at: Time) -> Ack {
    let ack = s.next_ack(at).unwrap().unwrap();
    s.ack_done(ack, Outcome::Accepted, at.monotonic_ms, 0).unwrap(); ack
}

#[test]
fn heartbeat_remains_schedulable_during_delayed_poll_and_unfinished_journal() {
    let mut s = online(&[]);
    let poll = s.poll(0).unwrap().unwrap();
    let Accepted::Store(write) = s.receive(command("one"), time(1)).unwrap() else { panic!() };
    assert!(s.busy());
    assert_eq!(s.poll(1500), Ok(None), "never overlap a still-owned transport request");
    assert_eq!(s.poll_overdue(1500), Some(poll));
    let heartbeat = s.heartbeat(1500).unwrap().unwrap();
    s.heartbeat_done(heartbeat, Outcome::Accepted, 1501, 0).unwrap();
    assert!(s.heartbeat(3000).unwrap().is_some());
    assert_eq!(s.next_ack(time(3000)), Ok(None));
    s.stored(write, true).unwrap();
    assert!(s.next_ack(time(3000)).unwrap().is_some());
}

#[test]
fn every_action_is_a_durable_explicit_denial_without_dispatch() {
    let mut s = online(&[]);
    for (i, action) in ["exec_v1", "start_service", "deliver_file", "update_agent", "cancel_command", "unknown"].iter().enumerate() {
        let id = format!("command-{i}");
        let c = Command { action, ..command(&id) };
        let record = store(&mut s, c, time(i as u64));
        assert_eq!(record.result, Final::Denied);
        assert_eq!(record.result.status(), "denied");
        assert_eq!(record.result.reason(), "action_not_allowed");
        let ack = acknowledge(&mut s, time(i as u64));
        assert_eq!(ack.record, record);
    }
    assert_eq!(s.record_count(), 6);
}

#[test]
fn uncertain_storage_failure_stops_acks_and_new_dispatch_until_recovery() {
    let mut s = online(&[]);
    let Accepted::Store(write) = s.receive(command("one"), time(1)).unwrap() else { panic!() };
    assert_eq!(s.stored(write, false), Err(Error::Journal));
    assert_eq!(s.stored(write, true), Err(Error::Stale));
    assert!(s.journal_failed());
    assert_eq!(s.record_count(), 0);
    assert_eq!(s.next_ack(time(100_000)), Ok(None));
    assert_eq!(s.receive(command("two"), time(100_000)), Err(Error::Journal));
    assert!(s.heartbeat(1500).unwrap().is_some(), "journal failure does not stop reporting");
}

#[test]
fn repeat_id_returns_original_result_without_a_second_store() {
    let mut s = online(&[]);
    let record = store(&mut s, command("one"), time(0));
    assert_eq!(s.receive(command("one"), time(1)), Ok(Accepted::InProgress));
    acknowledge(&mut s, time(2));
    assert_eq!(s.receive(command("one"), time(3)), Ok(Accepted::Replay));
    assert_eq!(acknowledge(&mut s, time(4)).record, record);
    assert_eq!(s.record_count(), 1);
}

#[test]
fn changed_payload_action_program_or_term_cannot_reuse_an_id() {
    let mut s = online(&[]);
    store(&mut s, command("one"), time(0)); acknowledge(&mut s, time(0));
    for c in [
        Command { payload: "different", ..command("one") },
        Command { action: "collect_logs", ..command("one") },
        Command { program: "another", ..command("one") },
        Command { expires_unix: Some(2000), ..command("one") },
    ] { assert_eq!(s.receive(c, time(1)), Err(Error::Conflict)); }
    assert_eq!(s.record_count(), 1);
}

#[test]
fn fingerprint_delimits_fields_and_includes_the_target_agent() {
    let mut one = online(&[]); let mut two = online(&[]);
    let a = store(&mut one, Command { program: "ab", payload: "c", ..command("one") }, time(0));
    let b = store(&mut two, Command { program: "a", payload: "bc", ..command("one") }, time(0));
    assert_ne!(a.identity, b.identity);
    assert!(matches!(Session::new("another", &[a], 0), Err(Error::WrongAgent)));
}

#[test]
fn recovery_retries_durable_results_in_order_and_never_regenerates_them() {
    let mut old = online(&[]);
    let a = store(&mut old, command("one"), time(0)); acknowledge(&mut old, time(0));
    let b = store(&mut old, command("two"), time(0));
    let mut restored = online(&[a, b]);
    assert!(restored.busy());
    assert_eq!(restored.receive(command("three"), time(0)), Err(Error::Busy));
    assert_eq!(acknowledge(&mut restored, time(0)).record, a);
    assert_eq!(acknowledge(&mut restored, time(0)).record, b);
    assert!(!restored.busy());
    assert_eq!(restored.record_count(), 2);
}

#[test]
fn ack_retry_uses_five_second_minimum_backoff_and_retains_exact_bytes() {
    let mut s = online(&[]);
    let record = store(&mut s, command("one"), time(0));
    let a = s.next_ack(time(0)).unwrap().unwrap();
    s.ack_done(a, Outcome::Retry, 10, 0).unwrap();
    assert_eq!(s.next_ack(time(5009)), Ok(None));
    let b = s.next_ack(time(5010)).unwrap().unwrap();
    assert_eq!(b.record.result.result_json(), record.result.result_json());
    assert_ne!(a.attempt.sequence, b.attempt.sequence);
    s.ack_done(b, Outcome::Retry, 5011, 250).unwrap();
    assert_eq!(s.next_ack(time(15_260)), Ok(None));
    assert!(s.next_ack(time(15_261)).unwrap().is_some());
}

#[test]
fn temporary_failure_backoff_is_capped_and_jitter_does_not_remove_the_floor() {
    let mut s = online(&[]); store(&mut s, command("one"), time(0));
    let mut now = 0;
    for failure in 0..12 {
        let ack = s.next_ack(time(now)).unwrap().unwrap();
        s.ack_done(ack, Outcome::Retry, now, u32::MAX).unwrap();
        let delay = (ACK_RETRY_MS * (1 << failure.min(6)) + (u32::MAX % 251) as u64).min(BACKOFF_MAX_MS);
        assert_eq!(s.next_ack(time(now + delay - 1)), Ok(None));
        now += delay;
    }
}

#[test]
fn permanent_ack_refusal_retains_result_and_does_not_become_executable() {
    let mut s = online(&[]); let record = store(&mut s, command("one"), time(0));
    let ack = s.next_ack(time(0)).unwrap().unwrap();
    s.ack_done(ack, Outcome::Permanent, 1, 0).unwrap();
    assert_eq!(s.next_ack(time(100_000)), Ok(None));
    assert_eq!(s.record(0), Some(record));
    assert_eq!(s.receive(command("one"), time(100_000)), Ok(Accepted::Replay));
    assert_eq!(s.next_ack(time(100_000)).unwrap().unwrap().record.result, Final::Denied);
}

#[test]
fn expiry_stops_retries_without_erasing_the_result_or_following_wall_clock_changes() {
    let mut s = online(&[]);
    let record = store(&mut s, Command { expires_unix: Some(1006), ..command("one") }, time(0));
    let ack = s.next_ack(time(0)).unwrap().unwrap();
    s.ack_done(ack, Outcome::Retry, 2000, 0).unwrap();
    assert_eq!(s.next_ack(Time { monotonic_ms: 6000, unix_seconds: Some(900) }), Ok(None));
    assert_eq!(s.record(0), Some(record));
    assert!(!s.busy());
}

#[test]
fn expired_delivery_gets_an_explicit_result_but_no_retry_past_its_term() {
    let mut s = online(&[]);
    let record = store(&mut s, Command { expires_unix: Some(999), ..command("one") }, time(0));
    assert_eq!(record.result, Final::Expired);
    let ack = s.next_ack(time(0)).unwrap().unwrap();
    s.ack_done(ack, Outcome::Retry, 0, 0).unwrap();
    assert_eq!(s.next_ack(time(5000)), Ok(None));
    assert_eq!(s.record(0), Some(record));
}

#[test]
fn missing_or_unrepresentable_clock_never_supplies_a_permissive_deadline() {
    let mut s = online(&[]);
    assert_eq!(s.receive(Command { expires_unix: Some(1001), ..command("one") }, Time { monotonic_ms: 0, unix_seconds: None }), Err(Error::Clock));
    assert_eq!(s.receive(Command { expires_unix: Some(u64::MAX), ..command("one") }, time(0)), Err(Error::ClockRange));
    assert_eq!(s.record_count(), 0);
}

#[test]
fn fixed_journal_capacity_never_evicts_a_completed_or_unacknowledged_record() {
    let mut s = online(&[]);
    for i in 0..RECORDS_MAX {
        let id = format!("id-{i}"); store(&mut s, command(&id), time(0)); acknowledge(&mut s, time(0));
    }
    let first = s.record(0);
    assert_eq!(s.receive(command("overflow"), time(0)), Err(Error::Full));
    assert_eq!(s.record(0), first);
    assert_eq!(s.record_count(), RECORDS_MAX);
    assert_eq!(s.receive(command("id-0"), time(0)), Ok(Accepted::Replay));
}

#[test]
fn malformed_or_foreign_commands_do_not_allocate_a_journal_slot() {
    let mut s = online(&[]);
    assert_eq!(s.receive(command("a&agent_id=other"), time(0)), Err(Error::Identifier));
    assert_eq!(s.receive(Command { agent_id: "other", ..command("one") }, time(0)), Err(Error::WrongAgent));
    let huge = "x".repeat(TEXT_MAX + 1);
    assert_eq!(s.receive(Command { payload: &huge, ..command("one") }, time(0)), Err(Error::Text));
    assert_eq!(s.receive(Command { action: "", ..command("one") }, time(0)), Err(Error::Text));
    assert_eq!(s.record_count(), 0);
}

#[test]
fn corrupt_journal_duplicate_ids_are_refused_before_recovery() {
    let mut s = online(&[]); let record = store(&mut s, command("one"), time(0));
    assert!(matches!(Session::new("mind-test", &[record, record], 0), Err(Error::Journal)));
    assert!(matches!(Session::new("mind-test", &[record; RECORDS_MAX + 1], 0), Err(Error::Full)));
}

#[test]
fn wakeup_during_a_poll_is_not_lost_and_does_not_bypass_failure_backoff() {
    let mut s = online(&[]); let poll = s.poll(0).unwrap().unwrap();
    s.wake(100);
    s.poll_done(poll, Outcome::Accepted, 500, 0).unwrap();
    let next = s.poll(500).unwrap().unwrap();
    s.poll_done(next, Outcome::Retry, 600, 0).unwrap();
    s.wake(700);
    assert_eq!(s.poll(1599), Ok(None));
    assert!(s.poll(1600).unwrap().is_some());
}

#[test]
fn stale_completions_cannot_complete_a_new_attempt_even_for_the_same_replayed_record() {
    let mut s = online(&[]); store(&mut s, command("one"), time(0));
    let first = acknowledge(&mut s, time(0));
    s.receive(command("one"), time(0)).unwrap();
    let second = s.next_ack(time(0)).unwrap().unwrap();
    assert_eq!(s.ack_done(first, Outcome::Accepted, 0, 0), Err(Error::Stale));
    assert_ne!(first.attempt.sequence, second.attempt.sequence);
    s.ack_done(second, Outcome::Accepted, 0, 0).unwrap();
}

#[test]
fn orderly_stop_sends_one_final_heartbeat_after_the_existing_one_and_stops_polling() {
    let mut s = online(&[]); let old = s.heartbeat(1500).unwrap().unwrap();
    s.stop(1501); assert!(s.stopping());
    assert_eq!(s.poll(2000), Ok(None));
    assert_eq!(s.receive(command("one"), time(2000)), Err(Error::Busy));
    s.heartbeat_done(old, Outcome::Accepted, 2000, 0).unwrap();
    assert!(!s.shutdown_done());
    let last = s.heartbeat(2000).unwrap().unwrap();
    s.heartbeat_done(last, Outcome::Accepted, 2001, 0).unwrap();
    assert!(s.shutdown_done());
    assert_eq!(s.heartbeat(100_000), Ok(None));
}

#[test]
fn http_classification_requires_valid_success_and_never_follows_redirects() {
    assert_eq!(outcome(200, true), Outcome::Accepted);
    assert_eq!(outcome(200, false), Outcome::Permanent);
    for status in [429, 500, 502, 599] { assert_eq!(outcome(status, false), Outcome::Retry); }
    for status in [204, 301, 302, 307, 400, 401, 403, 404, 405, 409] { assert_eq!(outcome(status, true), Outcome::Permanent); }
}
