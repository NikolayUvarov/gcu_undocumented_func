//! The file manager `fm` (docs/tools §4.1): two panels over the boot disk, the built-in viewer, quick view, find.
//! Everything but the program's main builds on the host and is tested there with a disk in memory (tests/fm_host.rs).
#![no_std]
extern crate alloc;

pub use mind::{abi, keys, tui};

pub mod panel;
pub mod fm;
