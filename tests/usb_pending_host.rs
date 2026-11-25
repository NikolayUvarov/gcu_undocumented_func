//! usb_host's deferred bulk transfers on the host (000-DRV-0101): where a transfer's bytes go, and what its client is
//! answered: the bytes, a reset after a halt, its device gone, a timeout, its client's new instance.
#[path = "../usb_host/src/pending.rs"]
mod pending;
use pending::{answer, place, Answer, Place};

#[test]
fn a_transfer_of_a_page_takes_a_page_and_a_longer_one_data_or_waits() {
    // A serial write of 256 bytes goes beside a stick's read of 64 KiB that holds DATA.
    assert_eq!(place(256, 4096, true, true), Place::Page);
    assert_eq!(place(4096, 4096, true, true), Place::Page);
    // No page free: DATA if free, else it waits.
    assert_eq!(place(256, 4096, false, false), Place::Data);
    assert_eq!(place(256, 4096, false, true), Place::Wait);
    // More than a page: DATA only, one at a time.
    assert_eq!(place(4097, 4096, true, false), Place::Data);
    assert_eq!(place(65536, 4096, true, true), Place::Wait);
}

#[test]
fn an_ended_transfer_is_answered_with_its_bytes() {
    assert_eq!(answer(true, false, Some(Ok(0)), 65536, 3, 30_000), Some(Answer::Moved(65536)));
    // A short packet: what came; a residue past the length gives none, not a wrapped count.
    assert_eq!(answer(true, false, Some(Ok(100)), 512, 3, 30_000), Some(Answer::Moved(412)));
    assert_eq!(answer(true, false, Some(Ok(600)), 512, 3, 30_000), Some(Answer::Moved(0)));
    // One that ended is answered even past the limit: its bytes came.
    assert_eq!(answer(true, false, Some(Ok(0)), 31, 40_000, 30_000), Some(Answer::Moved(31)));
}

#[test]
fn a_deferred_transfer_that_halts_resets_its_endpoint() {
    // A stall (6) and a transaction error (4) both halt the endpoint: reset, the client answered invalid.
    assert_eq!(answer(true, false, Some(Err(6)), 31, 5, 30_000), Some(Answer::Reset(6)));
    assert_eq!(answer(true, false, Some(Err(4)), 4096, 5, 30_000), Some(Answer::Reset(4)));
}

#[test]
fn a_deferred_transfer_without_an_end_times_out_at_its_limit() {
    assert_eq!(answer(true, false, None, 256, 29_999, 30_000), None);
    assert_eq!(answer(true, false, None, 256, 30_000, 30_000), Some(Answer::TimedOut));
}

#[test]
fn a_client_or_device_gone_before_its_answer_outweighs_a_completion() {
    // Its device or controller went: not-found, a completion found with it unused.
    assert_eq!(answer(false, false, None, 256, 5, 30_000), Some(Answer::Gone));
    assert_eq!(answer(false, false, Some(Ok(0)), 256, 5, 30_000), Some(Answer::Gone));
    // Its client attached anew: the earlier instance's transfer stopped, answered not-found.
    assert_eq!(answer(true, true, None, 256, 5, 30_000), Some(Answer::Stopped));
    assert_eq!(answer(true, true, Some(Ok(0)), 256, 5, 30_000), Some(Answer::Stopped));
    // Gone comes first.
    assert_eq!(answer(false, true, None, 256, 5, 30_000), Some(Answer::Gone));
}
