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
fn a_full_journal_drops_only_its_oldest_acknowledged_expired_record() {
    let mut s = online(&[]);
    // id-0 and id-1 expire at 1010, the rest never; id-1 is left unacknowledged at first.
    for i in 0..RECORDS_MAX {
        let id = format!("id-{i}");
        let c = Command { expires_unix: if i < 2 { Some(1010) } else { None }, ..command(&id) };
        store(&mut s, c, time(0));
        if i != 1 { acknowledge(&mut s, time(0)); } else { let ack = s.next_ack(time(0)).unwrap().unwrap(); s.ack_done(ack, Outcome::Permanent, 0, 0).unwrap(); }
    }
    assert!(!s.runnable(&command("early"), time(5000)), "no room: an action would not run");
    assert_eq!(s.receive(command("early"), time(5000)), Err(Error::Full), "nothing has expired yet");
    assert!(s.runnable(&command("late"), time(20_000)));
    assert!(!s.runnable(&Command { expires_unix: Some(1015), ..command("late") }, time(20_000)), "expired: answered, never run");
    let Accepted::Store(write) = s.receive(command("late"), time(20_000)).unwrap() else { panic!() };
    assert!(write.rewrite);
    assert_eq!(write.slot, 0, "id-0 was the oldest");
    s.stored(write, true).unwrap();
    let entries: Vec<(Record, bool)> = s.journal().collect();
    assert_eq!(entries.len(), RECORDS_MAX);
    assert_eq!(entries[0].0.id.as_str(), "id-1");
    assert_eq!(entries[RECORDS_MAX - 1].0.id.as_str(), "late");
    assert!(!entries[RECORDS_MAX - 1].1, "the new answer still waits for its ACK");
    // A redelivered id-0 is a new command now, and it has expired: answered `expired`, never run.
    acknowledge(&mut s, time(20_000));
    let Accepted::Store(again) = s.receive_with(Command { expires_unix: Some(1010), ..command("id-0") }, time(21_000), Final::Completed).unwrap() else { panic!() };
    assert_eq!((again.slot, again.record.result, again.rewrite), (1, Final::Expired, true), "id-1's slot, the next oldest");
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

#[test]
fn server_command_ids_with_a_dot_are_accepted_agent_ids_are_not() {
    let mut s = online(&[]);
    // The Effector server's IDs: `<UTC yyyymmddHHMMSS.nanoseconds>_<counter>` (server/commands.go).
    assert!(matches!(s.receive(command("20261021064900.000000001_000042"), time(0)), Ok(Accepted::Store(_))));
    assert_eq!(s.receive(command(".hidden"), time(0)), Err(Error::Identifier));
    assert_eq!(Id::command("a.b").map(|i| i.as_str().to_owned()), Ok("a.b".to_owned()));
    assert_eq!(Id::new("mind.test"), Err(Error::Identifier), "an agent ID keeps the contract's narrower grammar");
    assert!(matches!(Session::new("mind.test", &[], 0), Err(Error::Identifier)));
}

#[test]
fn an_action_runs_once_and_its_result_is_what_is_saved_and_repeated() {
    let mut s = online(&[]);
    let c = Command { action: "collect_file", payload: "{\"path\":\"log:/boot0001.log\"}", ..command("collect-1") };
    assert_eq!(s.fresh(&c), Ok(true), "a command never seen runs");
    let Accepted::Store(write) = s.receive_with(c, time(1), Final::Completed).unwrap() else { panic!() };
    s.stored(write, true).unwrap();
    assert_eq!(write.record.result, Final::Completed);
    assert_eq!(write.record.result.result_json(), r#"{"status":"completed","exit_code":0,"stdout":"","stderr":"","truncated":false,"run_as":"effector"}"#);
    assert_eq!(s.fresh(&c), Ok(false), "a saved command does not run again");
    assert_eq!(s.fresh(&Command { payload: "{\"path\":\"other\"}", ..c }), Err(Error::Conflict));
    let ack = acknowledge(&mut s, time(2));
    assert_eq!(ack.record.result, Final::Completed);
    assert_eq!(s.receive_with(c, time(3), Final::Failed(Failure::Io)), Ok(Accepted::Replay), "a redelivery keeps the saved answer");
    assert_eq!(s.next_ack(time(3)).unwrap().unwrap().record.result, Final::Completed);
    // A failure names the gateway's reason; an expired command stays expired whatever the action did.
    let failed = Final::Failed(Failure::NoPolicy);
    assert_eq!((failed.status(), failed.reason()), ("failed", "no-policy"));
    assert!(failed.result_json().ends_with(r#""error":"no-policy"}"#));
    for f in Failure::ALL { assert_eq!(Failure::from_name(f.name()), Some(f)); }
    let mut t = online(&[]);
    let old = Command { expires_unix: Some(900), ..command("late") };
    let Accepted::Store(late) = t.receive_with(old, time(0), Final::Completed).unwrap() else { panic!() };
    assert_eq!(late.record.result, Final::Expired);
}
