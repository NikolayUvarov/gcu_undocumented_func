//! `wifi` (550-APP-0033): `station` is what the program asks of the 802.11 station (idl/wifi.wit 1.1), with a stand-in
//! for tests; `view` the screen; `headless` `wifi join`, `status` and `leave` without it (550-APP-0078); `service` the
//! station's client. The first two build on the host and are tested there
//! (tests/wifi_host.rs).
#![no_std]
extern crate alloc;

pub use mind::{keys, tui};

pub mod countries;
pub mod headless;
pub mod service;
pub mod station;
pub mod view;
