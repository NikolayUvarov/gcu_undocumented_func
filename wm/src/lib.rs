//! The window manager `wm` (issue 088): the desktop — windows, their frames, focus and z-order, keys and the mouse,
//! snapping — builds on the host and is tested there (tests/wm_host.rs); `main.rs` connects it to the window broker.
#![no_std]
extern crate alloc;

pub use mind::{jpegdec, keys, png, tui};

pub mod background;
pub mod desk;
pub mod menu;
pub mod settings;
