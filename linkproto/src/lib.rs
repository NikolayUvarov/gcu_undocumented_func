//! MIND Link's protocol (800-LNK-0001, `docs/link/protocol.md`): frames COBS-encoded between zero bytes, each with a
//! header and a CRC-32C; a master that starts every exchange and a slave that only answers, so a half-duplex line
//! (RS-485) never has two senders; channels for control, the console, files, the log and input. No I/O of its own:
//! each end gives it a `Line`.
#![cfg_attr(not(test), no_std)]

pub mod app;
pub mod cobs;
pub mod crc;
pub mod endpoint;
pub mod frame;
#[path = "../../libmind/src/sha256.rs"]
pub mod sha256;

pub use endpoint::{Error, Line, Master, Service, Slave};
