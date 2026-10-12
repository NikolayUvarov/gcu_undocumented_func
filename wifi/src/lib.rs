//! `wifi` (550-APP-0033): `station` is what the program asks of the 802.11 station, with a stand-in for tests; `view`
//! the screen. Both build on the host and are tested there (tests/wifi_host.rs).
#![no_std]
extern crate alloc;

pub use mind::{keys, tui};

pub mod station;
pub mod view;
