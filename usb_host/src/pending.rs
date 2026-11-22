//! A deferred bulk transfer's rules (000-DRV-0101): where its bytes go, and what the loop does with it. No system calls:
//! tests/usb_pending_host.rs.

/// Where a transfer's bytes go: a pool page of its own (up to a page), DATA (one transfer a controller at a time), or
/// nowhere yet, so it waits until DATA is free.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place { Page, Data, Wait }

/// The place of a transfer of `length` bytes, with a pool page free for it or not and DATA held by another or not.
pub fn place(length: usize, page: usize, page_free: bool, data_busy: bool) -> Place {
    if length <= page && page_free { Place::Page } else if !data_busy { Place::Data } else { Place::Wait }
}

/// What the loop does with a transfer on its ring: answer it, give it up as timed out, or look again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next { Answer, TimedOut, Wait }

/// The next step of a transfer that `ended` or not, `waited_ms` after it was asked, with a time limit of `limit_ms`.
pub fn next(ended: bool, waited_ms: u64, limit_ms: u64) -> Next {
    if ended { Next::Answer } else if waited_ms >= limit_ms { Next::TimedOut } else { Next::Wait }
}
