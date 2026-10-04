//! System monitors: `top`, `memmap`, `load`, `hw`, `ipc` and `caps` (docs/tools §4.4–4.7). Each tool is a `model::Tool` that reads
//! sysmon through a `model::Source` and draws into a `tui::Grid`; everything but `app` builds on the host and is
//! tested there with a fake source (tests/monitor_host.rs).
#![no_std]
extern crate alloc;

pub use mind::{abi, keys, tui};

pub mod model;
pub mod text;
pub mod top;
pub mod memmap;
pub mod load;
pub mod hw;
pub mod ipc;
pub mod caps;
#[cfg(target_os = "none")]
pub mod app;
