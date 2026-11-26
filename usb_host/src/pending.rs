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

/// What a deferred transfer's client is answered, and what usb_host does first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    /// It ended: the bytes it moved.
    Moved(u32),
    /// It ended with an error, its completion code: a stall or a transaction error halted the endpoint, which is reset
    /// on both sides before the client is answered invalid (BOT recovery, 211-DRV-0053).
    Reset(u32),
    /// Its device or controller went: answered not-found, with nothing left on a ring to stop.
    Gone,
    /// No completion within the limit: its endpoint stopped and its TRB left behind, answered as timed out.
    TimedOut,
    /// Its client attached anew (the earlier instance gone): its endpoint stopped, the earlier instance answered not-found.
    Stopped,
}

/// The answer to a deferred transfer of `length` bytes from what the loop finds: whether its device is still `here`,
/// whether its client `renewed` itself, what `ended` (Ok(residue) or Err(completion code)), and how long it waited
/// against `limit_ms`. None while it waits. A device gone or a client renewed outweighs a completion found with it.
pub fn answer(here: bool, renewed: bool, ended: Option<Result<u32, u32>>, length: usize, waited_ms: u64, limit_ms: u64) -> Option<Answer> {
    if !here { return Some(Answer::Gone); }
    if renewed { return Some(Answer::Stopped); }
    match ended {
        Some(Ok(residue)) => Some(Answer::Moved(length.saturating_sub(residue as usize) as u32)),
        Some(Err(code)) => Some(Answer::Reset(code)),
        None if waited_ms >= limit_ms => Some(Answer::TimedOut),
        None => None,
    }
}
