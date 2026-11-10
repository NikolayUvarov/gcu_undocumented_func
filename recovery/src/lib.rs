//! `recovery` (308-APP-0122): `keyfile` is the key's written form and its file, `view` the screen against a `Store`,
//! `service` the store's client the shell lends. The first two build on the host and are tested there
//! (tests/recovery_host.rs).
#![no_std]
extern crate alloc;

pub use mind::{blockstore, keys, tui};

pub mod keyfile;
pub mod service;
pub mod view;
