#![no_std]
//! The Effector agent's session core and JSON writer; the service and its transport are 750-EFF-0006.
extern crate alloc;
pub use mind::sha256;
pub mod bench;
pub mod config;
pub mod journal;
pub mod json;
pub mod session;
pub mod telemetry;
pub mod wire;
