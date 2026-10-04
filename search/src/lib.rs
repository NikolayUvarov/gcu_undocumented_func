//! `find` and `grep` (docs/tools §2.1): option parsing, the walk over a directory tree and line matching. Everything
//! but the programs' mains builds on the host and is tested there with a tree in memory (tests/search_host.rs).
#![no_std]
extern crate alloc;

pub use mind::pattern;

pub mod find;
pub mod grep;
