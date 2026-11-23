//! usb_host's deferred bulk transfers on the host (000-DRV-0101): where a transfer's bytes go, and the loop's next step.
#[path = "../usb_host/src/pending.rs"]
mod pending;
use pending::{next, place, Next, Place};

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
fn the_loop_answers_an_ended_one_and_gives_up_one_past_its_limit() {
    assert_eq!(next(true, 0, 30_000), Next::Answer);
    // One that ended is answered even past the limit: its bytes came.
    assert_eq!(next(true, 40_000, 30_000), Next::Answer);
    assert_eq!(next(false, 29_999, 30_000), Next::Wait);
    assert_eq!(next(false, 30_000, 30_000), Next::TimedOut);
}
