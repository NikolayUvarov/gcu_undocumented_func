// aarch64 on QEMU's `virt` machine (issue 201): EL1, GICv3, the generic timer, the PL011 UART; the platform layout
// of `virt` is fixed here until the device tree or ACPI is read (issue 202).
pub mod acpi;
pub mod clock;
pub mod context;
pub mod cpu;
pub mod interrupts;
pub mod mmu;
pub mod pcicfg;
pub mod platform;
pub mod port;
pub mod serial;
