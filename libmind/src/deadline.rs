//! A shared deadline for a sequence of IDL calls (550-NET-0031).

#[derive(Clone, Copy)]
pub struct Deadline { end: u64 }

impl Deadline {
    pub fn after(now_ms: u64, milliseconds: u32) -> Self { Self { end: now_ms.saturating_add(milliseconds as u64) } }

    pub fn remaining(self, now_ms: u64) -> Option<u32> {
        let left = self.end.saturating_sub(now_ms).min(u32::MAX as u64) as u32;
        (left != 0).then_some(left)
    }

    /// An expired deadline makes no call: zero would mean an unlimited IPC wait.
    pub fn call<T>(self, call: impl FnOnce() -> crate::sys::Result<T>) -> crate::sys::Result<T> {
        let left = self.remaining(crate::time::monotonic_ns() / 1_000_000).ok_or(crate::sys::Error::Other(crate::abi::ERR_TIMEOUT))?;
        crate::idl::wire::with_timeout(left, call)
    }
}
