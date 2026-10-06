//! The pins of an ARM board through the gpio service (issues u015, u017): `tool` is what `pins` says and does, `view`
//! the header `pinmap` draws. Both build on the host and are tested there (tests/pins_host.rs) against the register
//! models of mind::gpio; the programs wrap idl/gpio.wit in `tool::Gpio` (`service`).
#![no_std]
extern crate alloc;

pub use mind::{keys, tui};

pub mod tool;
pub mod view;
#[cfg(target_os = "none")]
pub mod service;
