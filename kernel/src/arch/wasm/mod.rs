// MIND Core as a wasm64 module that a page or Node drives (215-PRT-0017): the kernel as on aarch64, its tasks' A64 code
// on the `a64` emulator, which walks the kernel's aarch64 page tables. The page gives the clock, the console, the screen's
// memory and the boot volume (`host`); `run` is what it calls. The page's devices are the `wasm_machine` crate's, outside
// the kernel as a machine is; no ACPI, watchdog or TPM.
pub mod acpi;
pub mod clock;
pub mod context;
pub mod cpu;
pub mod host;
pub mod interrupts;
pub mod mmu;
pub mod pcicfg;
pub mod platform;
pub mod port;
pub mod report;
pub mod run;
pub mod serial;
pub mod watchdog;
