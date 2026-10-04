//! Host test of sysmon's per-client rate limit (sysmon/src/limit.rs).
#[path = "../sysmon/src/limit.rs"]
mod limit;
use limit::{Limiter, BURST, PER_SECOND};

#[test]
fn burst_then_steady_rate_per_client() {
    let mut limiter = Limiter::new();
    let admitted = (0..BURST + 10).filter(|_| limiter.admit(7, 1000)).count() as u32;
    assert_eq!(admitted, BURST, "a burst is cut at BURST requests");
    assert!(limiter.admit(8, 1000), "another client has its own bucket");
    // One second later the client has PER_SECOND new tokens, capped at BURST.
    let later = (0..PER_SECOND + 10).filter(|_| limiter.admit(7, 2000)).count() as u32;
    assert_eq!(later, BURST.min(PER_SECOND));
    // A steady 20 requests per second is always served.
    let mut limiter = Limiter::new();
    assert!((0..200).all(|i| limiter.admit(3, 5000 + i * 50)));
}

#[test]
fn new_clients_reuse_the_oldest_entry() {
    let mut limiter = Limiter::new();
    for pid in 1..=40 { assert!(limiter.admit(pid, pid * 10)); }
}
