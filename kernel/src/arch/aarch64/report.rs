// The processor's part of the hardware report (174-KRN-0038): the ID registers raw and decoded, the caches and the
// timer. ID registers not allocated read as zero at EL1, so every one is read directly.
use alloc::string::String;
use core::fmt::Write;

macro_rules! read { ($name:literal) => {{ let value: u64; unsafe { core::arch::asm!(concat!("mrs {}, ", $name), out(reg) value, options(nomem, nostack)); } value }}; }

/// No probing read can fault on aarch64.
pub fn resume_after_fault(_pc: u64) -> Option<u64> { None }

fn field(value: u64, shift: u32) -> u64 { (value >> shift) & 0xF }

fn implementer(code: u64) -> &'static str {
    match code { 0x41 => "Arm", 0x42 => "Broadcom", 0x43 => "Cavium", 0x46 => "Fujitsu", 0x48 => "HiSilicon", 0x4E => "NVIDIA", 0x50 => "Applied Micro", 0x51 => "Qualcomm",
        0x53 => "Samsung", 0x61 => "Apple", 0x69 => "Intel", 0x6D => "Microsoft", 0xC0 => "Ampere", _ => "?" }
}

/// The CPU section.
pub fn cpu(out: &mut String) {
    let midr = read!("midr_el1");
    let _ = writeln!(out, "CPU\n  implementer       {:#X} ({})\n  part              {:#X} variant {:#X} revision {:#X} (MIDR {:016X})", (midr >> 24) & 0xFF, implementer((midr >> 24) & 0xFF),
        (midr >> 4) & 0xFFF, (midr >> 20) & 0xF, midr & 0xF, midr);
    let count = crate::cpu::COUNT.load(core::sync::atomic::Ordering::Relaxed);
    let _ = write!(out, "  CPUs started      {} of {} listed\n  MPIDR affinities ", count, crate::acpi::CPU_COUNT.load(core::sync::atomic::Ordering::Relaxed));
    for index in 0..count { let _ = write!(out, " {:#X}", crate::cpu::apic_id(index)); }
    out.push('\n');
    let (pfr0, pfr1, isar0, isar1, isar2) = (read!("id_aa64pfr0_el1"), read!("id_aa64pfr1_el1"), read!("id_aa64isar0_el1"), read!("id_aa64isar1_el1"), read!("S3_0_C0_C6_2"));
    let (mmfr0, mmfr1, mmfr2, dfr0, zfr0, smfr0) = (read!("id_aa64mmfr0_el1"), read!("id_aa64mmfr1_el1"), read!("S3_0_C0_C7_2"), read!("id_aa64dfr0_el1"), read!("S3_0_C0_C4_4"), read!("S3_0_C0_C4_5"));
    let level = |v: u64| match v { 0xF => "absent", 0 => "present", 1 => "present with FP16", _ => "present (newer)" };
    out.push_str("Features\n");
    let _ = writeln!(out, "  FP                {}\n  Advanced SIMD     {}\n  SVE               {}\n  SME               {}\n  MTE               level {}\n  BTI               {}\n  pointer auth      APA {} API {} APA3 {}",
        level(field(pfr0, 16)), level(field(pfr0, 20)), if field(pfr0, 32) != 0 { "present" } else { "absent" }, if field(pfr1, 24) != 0 { "present" } else { "absent" },
        field(pfr1, 8), field(pfr1, 0) != 0, field(isar1, 4), field(isar1, 8), field(isar2, 12));
    let _ = writeln!(out, "  AES {} SHA1 {} SHA2 {} SHA3 {} CRC32 {} atomics {} dot product {} RNDR {} FHM {} BF16 {} I8MM {}",
        field(isar0, 4), field(isar0, 8), field(isar0, 12), field(isar0, 32), field(isar0, 16), field(isar0, 20), field(isar0, 44), field(isar0, 60), field(isar0, 48), field(isar1, 44), field(isar1, 52));
    let _ = writeln!(out, "  physical address  {} bits; 4K granule {}, 16K {}, 64K {}; PAN {}; VHE {}; EL2 {}; EL3 {}",
        [32, 36, 40, 42, 44, 48, 52, 56].get(field(mmfr0, 0) as usize).copied().unwrap_or(0), field(mmfr0, 28) != 0xF, field(mmfr0, 20) != 0, field(mmfr0, 24) != 0xF,
        field(mmfr1, 20), field(mmfr1, 8), field(pfr0, 8) != 0, field(pfr0, 12) != 0);
    let (ctr, clidr) = (read!("ctr_el0"), read!("clidr_el1"));
    let _ = writeln!(out, "Caches\n  CTR_EL0 {:016X} (line: data {} B, instruction {} B)\n  CLIDR_EL1 {:016X}", ctr, 4 << ((ctr >> 16) & 0xF), 4 << (ctr & 0xF), clidr);
    for level in 0..7u64 {
        let kind = (clidr >> (level * 3)) & 7;
        if kind == 0 { break; }
        for instruction in [false, true] {
            if (instruction && kind & 1 == 0) || (!instruction && kind == 1) { continue; }
            let ccsidr: u64;
            unsafe { core::arch::asm!("msr csselr_el1, {}", "isb", "mrs {}, ccsidr_el1", in(reg) (level << 1) | instruction as u64, out(reg) ccsidr, options(nostack)); }
            let (line, ways, sets) = (16u64 << (ccsidr & 7), ((ccsidr >> 3) & 0x3FF) + 1, ((ccsidr >> 13) & 0x7FFF) + 1);
            let _ = writeln!(out, "  L{} {:<11} {} KiB, {}-way, line {} B", level + 1, if instruction { "instruction" } else if kind == 4 { "unified" } else { "data" }, line * ways * sets / 1024, ways, line);
        }
    }
    let _ = writeln!(out, "ID registers (raw)\n  PFR0 {:016X} PFR1 {:016X}\n  ISAR0 {:016X} ISAR1 {:016X} ISAR2 {:016X}\n  MMFR0 {:016X} MMFR1 {:016X} MMFR2 {:016X}\n  DFR0 {:016X} ZFR0 {:016X} SMFR0 {:016X}\n  REVIDR {:016X} MPIDR {:016X}",
        pfr0, pfr1, isar0, isar1, isar2, mmfr0, mmfr1, mmfr2, dfr0, zfr0, smfr0, read!("revidr_el1"), read!("mpidr_el1"));
}

/// The platform section: what the kernel chose on this machine.
pub fn kernel(out: &mut String) {
    let _ = writeln!(out, "  architecture      aarch64\n  timer             {} Hz (CNTFRQ_EL0)\n  vector state      FP/SIMD: V0-V31, FPCR and FPSR saved a task (250-KRN-0056)\n  protection        {}", read!("cntfrq_el0"), if super::cpu::pan() { "PXN PAN" } else { "PXN" });
}
