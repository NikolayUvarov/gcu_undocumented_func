//! The system's checks and measurements (main task 176): `check` (what is done works), `bench` (the components'
//! performance) and `kbench` (the kernel's). `report` is pure and tested on the host (tests/bench_host.rs); `out` holds
//! the screen, the log file and the machine's description.
#![no_std]
extern crate alloc;

pub mod out;
pub mod report;
