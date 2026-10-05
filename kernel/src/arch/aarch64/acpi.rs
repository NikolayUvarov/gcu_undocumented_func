// Reset through PSCI (the firmware interface of `virt`, HVC conduit): SYSTEM_RESET. The ACPI tables are not read yet.
pub unsafe fn init(_rsdp: u64) {}

pub unsafe fn reboot() -> ! {
    core::arch::asm!("hvc #0", in("x0") 0x8400_0009u64, options(nostack)); // PSCI SYSTEM_RESET
    crate::cpu::halt_all()
}
