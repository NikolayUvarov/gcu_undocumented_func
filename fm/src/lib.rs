//! The file manager `fm` (docs/tools §4.1): two panels over the boot disk and `ram:`, the built-in viewer and editor
//! (`edit`'s library), quick view, find, copy/move/mkdir/delete.
//! Everything but the program's main builds on the host and is tested there with a disk in memory (tests/fm_host.rs).
#![no_std]
extern crate alloc;

pub use edit::{buffer, editor};
pub use mind::{abi, cid, dag, keys, pattern, tui};

pub mod panel;
pub mod fm;
pub mod store;
