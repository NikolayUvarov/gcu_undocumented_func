//! The store's client badged `BADGE_RECOVER` that the shell lends in `SLOT_RECOVER` for `REQUEST_RECOVER` (308-KRN-0204):
//! `blockstore.wit` 2.4's recovery calls, each with a deadline.
use crate::view::{Refused, Store};
use alloc::string::String;
use mind::idl::{blockstore, wire};
use mind::ipc::Endpoint;

// How long one call may take: `recovery-new` writes and flushes a record, `recover` mounts the store.
const CALL_MS: u32 = 30_000;

pub struct Service(pub Endpoint);

fn answer<T>(result: mind::sys::Result<Result<T, blockstore::Error>>) -> Result<T, Refused> {
    match result {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(Refused::Store(error as u8)),
        Err(_) => Err(Refused::NoAnswer),
    }
}

impl Store for Service {
    fn records(&mut self) -> Result<u32, Refused> { let e = self.0; answer(wire::with_timeout(CALL_MS, || blockstore::recoveries(e))) }
    fn make(&mut self) -> Result<String, Refused> { let e = self.0; answer(wire::with_timeout(CALL_MS, || blockstore::recovery_new(e))).map(|key| String::from(key.as_str())) }
    fn keep(&mut self, key: &str) -> Result<u32, Refused> { let e = self.0; answer(wire::with_timeout(CALL_MS, || blockstore::recovery_keep(e, key))) }
    fn open(&mut self, key: &str) -> Result<(), Refused> { let e = self.0; answer(wire::with_timeout(CALL_MS, || blockstore::recover(e, key))) }
}

/// No client was lent: every call says so.
pub struct Absent;

impl Store for Absent {
    fn records(&mut self) -> Result<u32, Refused> { Err(Refused::NoClient) }
    fn make(&mut self) -> Result<String, Refused> { Err(Refused::NoClient) }
    fn keep(&mut self, _: &str) -> Result<u32, Refused> { Err(Refused::NoClient) }
    fn open(&mut self, _: &str) -> Result<(), Refused> { Err(Refused::NoClient) }
}
