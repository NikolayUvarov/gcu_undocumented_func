//! The text editor `edit` (docs/tools §4.2): a piece table with undo, the editor's keys, dialogs and drawing.
//! Everything but the program's main builds on the host and is tested there (tests/edit_host.rs).
#![no_std]
extern crate alloc;

pub use mind::{abi, keys, tui};

pub mod buffer;
pub mod editor;
