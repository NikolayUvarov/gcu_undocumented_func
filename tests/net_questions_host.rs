//! The stack's bookkeeping of a grant's questions for a flow (netstack/src/questions.rs, socket.wit 2.8, 000-NET-0049).
extern crate alloc;

#[path = "../netstack/src/questions.rs"]
mod questions;

use questions::{Questions, Refusal, State, WAITING};

#[test]
fn a_question_is_asked_answered_and_read_by_its_grant_alone() {
    let mut q = Questions::default();
    let id = q.ask(7, "example.org", 443, "fetch a page").unwrap();
    assert_eq!(q.asked(7, id), Ok(State::Pending));
    assert_eq!(q.asked(8, id), Err(Refusal::NotFound)); // another grant's question is not its to read
    assert_eq!(q.pending(id), Some((7, 443)));
    let waiting: Vec<_> = q.waiting().map(|w| (w.id, w.badge, w.host.clone(), w.port, w.why.clone())).collect();
    assert_eq!(waiting, vec![(id, 7, "example.org".to_string(), 443, "fetch a page".to_string())]);
    q.settle(id, State::Allowed(0x0B00_0250)).unwrap();
    assert_eq!(q.asked(7, id), Ok(State::Allowed(0x0B00_0250)));
    assert_eq!(q.pending(id), None);
    assert_eq!(q.waiting().count(), 0);
    assert_eq!(q.settle(id, State::Refused), Err(Refusal::NotFound)); // answered once only
}

#[test]
fn an_empty_host_or_port_zero_is_refused() {
    let mut q = Questions::default();
    assert_eq!(q.ask(1, "", 80, ""), Err(Refusal::Invalid));
    assert_eq!(q.ask(1, "host", 0, ""), Err(Refusal::Invalid));
}

#[test]
fn a_grant_waits_on_one_question_at_a_time() {
    let mut q = Questions::default();
    let first = q.ask(3, "a.example", 80, "").unwrap();
    assert_eq!(q.ask(3, "b.example", 80, ""), Err(Refusal::Limit));
    assert!(q.ask(4, "b.example", 80, "").is_ok()); // another grant may
    q.settle(first, State::Refused).unwrap();
    let second = q.ask(3, "b.example", 80, "").unwrap();
    assert_ne!(first, second);
}

#[test]
fn at_most_eight_wait_in_all() {
    let mut q = Questions::default();
    for badge in 1..=WAITING as u16 { q.ask(badge, "h", 80, "").unwrap(); }
    assert_eq!(q.ask(100, "h", 80, ""), Err(Refusal::Limit));
    let oldest = q.waiting().next().unwrap().id;
    q.settle(oldest, State::Refused).unwrap();
    assert!(q.ask(100, "h", 80, "").is_ok());
}

#[test]
fn a_grant_gone_takes_its_questions() {
    let mut q = Questions::default();
    let id = q.ask(5, "h", 80, "").unwrap();
    let other = q.ask(6, "h", 80, "").unwrap();
    q.forget(5);
    assert_eq!(q.asked(5, id), Err(Refusal::NotFound));
    assert_eq!(q.pending(id), None);
    assert_eq!(q.asked(6, other), Ok(State::Pending));
}

#[test]
fn answered_questions_are_kept_a_while_not_for_ever() {
    let mut q = Questions::default();
    let first = q.ask(9, "h", 80, "").unwrap();
    q.settle(first, State::Refused).unwrap();
    for _ in 0..100 {
        let id = q.ask(9, "h", 80, "").unwrap();
        q.settle(id, State::Refused).unwrap();
    }
    assert_eq!(q.asked(9, first), Err(Refusal::NotFound)); // the oldest answered ones went first
    let last = q.ask(9, "h", 80, "").unwrap();
    assert_eq!(q.asked(9, last), Ok(State::Pending));
}
