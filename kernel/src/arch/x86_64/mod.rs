// x86-64 with UEFI, xAPIC, the 8259 PIC and PIT, port I/O and PCI configuration through ports 0xCF8/0xCFC.
pub mod acpi;
pub mod clock;
pub mod context;
pub mod cpu;
pub mod interrupts;
pub mod paging;
pub mod pci;
pub mod port;
pub mod serial;
