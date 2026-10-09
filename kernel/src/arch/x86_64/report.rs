// The processor's part of the hardware report (174-KRN-0038): every CPUID leaf raw and decoded, the model-specific
// registers that describe frequencies and power (read so that a missing one is reported, not fatal), and what the
// kernel chose. Run on the boot CPU while init asks for the report.
use alloc::string::String;
use core::arch::x86_64::{__cpuid_count, CpuidResult};
use core::fmt::Write;

core::arch::global_asm!(r#"
    .global msr_read_checked
msr_read_checked:
    mov ecx, edi
    mov qword ptr [rsi], 0
    .global msr_read_instruction
msr_read_instruction:
    rdmsr
    mov qword ptr [rsi], 1
    .global msr_read_resume
msr_read_resume:
    shl rdx, 32
    or rax, rdx
    ret
"#);
unsafe extern "sysv64" {
    fn msr_read_checked(msr: u64, ok: *mut u64) -> u64;
    static msr_read_instruction: u8;
    static msr_read_resume: u8;
}

/// A model-specific register, or None when the CPU has none there (#GP, resumed by `resume_after_fault`).
pub fn msr(number: u32) -> Option<u64> {
    let mut ok = 0u64;
    let value = unsafe { msr_read_checked(number as u64, &mut ok) };
    (ok == 1).then_some(value)
}

/// Where a kernel fault at `pc` resumes, if it is the checked read's (the result reads as absent).
pub fn resume_after_fault(pc: u64) -> Option<u64> {
    (pc == core::ptr::addr_of!(msr_read_instruction) as u64).then(|| core::ptr::addr_of!(msr_read_resume) as u64)
}

fn cpuid(leaf: u32, sub: u32) -> CpuidResult { __cpuid_count(leaf, sub) }

// Feature names by register bit, as Linux's /proc/cpuinfo spells them; "" where the bit is reserved.
const LEAF1_EDX: [&str; 32] = ["fpu", "vme", "de", "pse", "tsc", "msr", "pae", "mce", "cx8", "apic", "", "sep", "mtrr", "pge", "mca", "cmov",
    "pat", "pse36", "pn", "clflush", "", "dts", "acpi", "mmx", "fxsr", "sse", "sse2", "ss", "ht", "tm", "ia64", "pbe"];
const LEAF1_ECX: [&str; 32] = ["sse3", "pclmulqdq", "dtes64", "monitor", "ds_cpl", "vmx", "smx", "est", "tm2", "ssse3", "cid", "sdbg", "fma", "cx16", "xtpr", "pdcm",
    "", "pcid", "dca", "sse4_1", "sse4_2", "x2apic", "movbe", "popcnt", "tsc_deadline_timer", "aes", "xsave", "osxsave", "avx", "f16c", "rdrand", "hypervisor"];
const LEAF7_EBX: [&str; 32] = ["fsgsbase", "tsc_adjust", "sgx", "bmi1", "hle", "avx2", "fdp_excptn_only", "smep", "bmi2", "erms", "invpcid", "rtm", "rdt_m", "zero_fcs_fds", "mpx", "rdt_a",
    "avx512f", "avx512dq", "rdseed", "adx", "smap", "avx512ifma", "pcommit", "clflushopt", "clwb", "intel_pt", "avx512pf", "avx512er", "avx512cd", "sha_ni", "avx512bw", "avx512vl"];
const LEAF7_ECX: [&str; 32] = ["prefetchwt1", "avx512vbmi", "umip", "pku", "ospke", "waitpkg", "avx512_vbmi2", "shstk", "gfni", "vaes", "vpclmulqdq", "avx512_vnni", "avx512_bitalg", "tme", "avx512_vpopcntdq", "",
    "la57", "", "", "", "", "", "rdpid", "kl", "bus_lock_detect", "cldemote", "", "movdiri", "movdir64b", "enqcmd", "sgx_lc", "pks"];
const LEAF7_EDX: [&str; 32] = ["", "sgx_keys", "avx512_4vnniw", "avx512_4fmaps", "fsrm", "uintr", "", "", "avx512_vp2intersect", "srbds_ctrl", "md_clear", "rtm_always_abort", "", "tsx_force_abort", "serialize", "hybrid",
    "tsxldtrk", "", "pconfig", "arch_lbr", "ibt", "", "amx_bf16", "avx512_fp16", "amx_tile", "amx_int8", "spec_ctrl", "intel_stibp", "flush_l1d", "arch_capabilities", "core_capabilities", "ssbd"];
const LEAF7_1_EAX: [&str; 32] = ["sha512", "sm3", "sm4", "rao_int", "avx_vnni", "avx512_bf16", "lass", "cmpccxadd", "", "", "fzrm", "fsrs", "fsrc", "", "", "",
    "", "fred", "lkgs", "wrmsrns", "", "amx_fp16", "hreset", "avx_ifma", "", "", "lam", "msrlist", "", "", "", ""];
const EXT1_EDX: [&str; 32] = ["", "", "", "", "", "", "", "", "", "", "", "syscall", "", "", "", "",
    "", "", "", "mp", "nx", "", "mmxext", "", "", "fxsr_opt", "pdpe1gb", "rdtscp", "", "lm", "3dnowext", "3dnow"];
const EXT1_ECX: [&str; 32] = ["lahf_lm", "cmp_legacy", "svm", "extapic", "cr8_legacy", "abm", "sse4a", "misalignsse", "3dnowprefetch", "osvw", "ibs", "xop", "skinit", "wdt", "", "lwp",
    "fma4", "tce", "", "nodeid_msr", "", "tbm", "topoext", "perfctr_core", "perfctr_nb", "", "bpext", "ptsc", "perfctr_llc", "mwaitx", "", ""];
const LEAF6_EAX: [&str; 32] = ["dtherm", "ida", "arat", "", "pln", "ecmd", "pts", "hwp", "hwp_notify", "hwp_act_window", "hwp_epp", "hwp_pkg_req", "", "hdc", "turbo_max_3", "hwp_highest_perf_change",
    "hwp_peci_override", "hwp_flexible", "hwp_fast_request", "hfi", "hwp_ignore_idle", "", "", "thread_director", "therm_interrupt_bit25", "", "", "", "", "", "", ""];
const XSAVE_1_EAX: [&str; 32] = ["xsaveopt", "xsavec", "xgetbv1", "xsaves", "xfd", "", "", "", "", "", "", "", "", "", "", "",
    "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""];
const XSAVE_COMPONENTS: [&str; 19] = ["x87", "SSE", "AVX", "MPX BNDREGS", "MPX BNDCSR", "AVX-512 opmask", "AVX-512 ZMM_Hi256", "AVX-512 Hi16_ZMM", "PT", "PKRU",
    "PASID", "CET user", "CET supervisor", "HDC", "UINTR", "LBR", "HWP", "AMX TILECFG", "AMX TILEDATA"];

fn names(out: &mut String, label: &str, value: u32, table: &[&str; 32]) { names_or(out, label, value, table, &[""; 32]); }

// AMD repeats leaf 1's EDX bits in extended leaf 1: a bit without its own name takes leaf 1's.
fn names_or(out: &mut String, label: &str, value: u32, table: &[&str; 32], fallback: &[&str; 32]) {
    let _ = write!(out, "  {:<18}", label);
    for bit in 0..32 {
        let name = if table[bit].is_empty() { fallback[bit] } else { table[bit] };
        if value & (1 << bit) != 0 { let _ = if name.is_empty() { write!(out, " bit{}", bit) } else { write!(out, " {}", name) }; }
    }
    out.push('\n');
}

fn text(words: &[u32]) -> String { words.iter().flat_map(|w| w.to_le_bytes()).filter(|&b| b != 0).map(|b| b as char).collect::<String>().trim().into() }

// The subleaves worth reading for `leaf`: those that hold data, as each leaf defines its own end.
fn subleaves(leaf: u32) -> u32 {
    match leaf {
        4 | 0x8000_001D => (0..64).take_while(|&s| cpuid(leaf, s).eax & 0x1F != 0).count() as u32 + 1,
        7 | 0x14 | 0x17 | 0x18 | 0x1D | 0x20 => (cpuid(leaf, 0).eax + 1).min(64),
        0xB | 0x1F | 0x8000_0026 => (0..64).take_while(|&s| (cpuid(leaf, s).ecx >> 8) & 0xFF != 0).count() as u32 + 1,
        0xD => 64,
        0xF | 0x10 | 0x12 | 0x8000_0020 => 8,
        _ => 1,
    }
}

fn raw(out: &mut String, first: u32) {
    let last = cpuid(first, 0).eax;
    if last < first || last > first + 0x80 { return; }
    for leaf in first..=last {
        for sub in 0..subleaves(leaf) {
            let r = cpuid(leaf, sub);
            if leaf == 0xD && sub > 1 && r.eax == 0 { continue; }
            let _ = writeln!(out, "  {:08X}.{:02} EAX={:08X} EBX={:08X} ECX={:08X} EDX={:08X}", leaf, sub, r.eax, r.ebx, r.ecx, r.edx);
        }
    }
}

/// The CPU section.
pub fn cpu(out: &mut String) {
    let zero = cpuid(0, 0);
    let vendor = text(&[zero.ebx, zero.edx, zero.ecx]);
    let intel = vendor == "GenuineIntel";
    let max_ext = cpuid(0x8000_0000, 0).eax;
    let brand = if max_ext >= 0x8000_0004 { text(&(0x8000_0002..=0x8000_0004).flat_map(|l| { let r = cpuid(l, 0); [r.eax, r.ebx, r.ecx, r.edx] }).collect::<alloc::vec::Vec<_>>()) } else { String::new() };
    let one = cpuid(1, 0);
    let (stepping, base_model, base_family) = (one.eax & 0xF, (one.eax >> 4) & 0xF, (one.eax >> 8) & 0xF);
    let family = if base_family == 0xF { base_family + ((one.eax >> 20) & 0xFF) } else { base_family };
    let model = if base_family == 0x6 || base_family == 0xF { base_model | ((one.eax >> 16) & 0xF) << 4 } else { base_model };
    let _ = writeln!(out, "CPU\n  vendor            {}\n  brand             {}\n  family            {:#X} model {:#X} stepping {:#X} (signature {:08X})", vendor, brand, family, model, stepping, one.eax);
    // Intel: the revision is in the high half (the firmware ran CPUID 1 after loading it); AMD: the patch level in the low half.
    match msr(0x8B) { Some(v) => { let _ = writeln!(out, "  microcode         {:#X}", if intel { v >> 32 } else { v & 0xFFFF_FFFF }); } None => out.push_str("  microcode         (no MSR 8B)\n") }
    let _ = writeln!(out, "  CPUs started      {} of {} listed", crate::cpu::COUNT.load(core::sync::atomic::Ordering::Relaxed), crate::acpi::CPU_COUNT.load(core::sync::atomic::Ordering::Relaxed));
    let _ = write!(out, "  APIC IDs         ");
    for index in 0..crate::cpu::COUNT.load(core::sync::atomic::Ordering::Relaxed) { let _ = write!(out, " {}", crate::cpu::apic_id(index)); }
    out.push('\n');
    if max_ext >= 0x8000_0008 { let r = cpuid(0x8000_0008, 0); let _ = writeln!(out, "  address bits      physical {} virtual {}", r.eax & 0xFF, (r.eax >> 8) & 0xFF); }
    out.push_str("Features\n");
    names(out, "leaf 1 EDX", one.edx, &LEAF1_EDX);
    names(out, "leaf 1 ECX", one.ecx, &LEAF1_ECX);
    if zero.eax >= 6 { names(out, "leaf 6 (power)", cpuid(6, 0).eax, &LEAF6_EAX); }
    if zero.eax >= 7 {
        let seven = cpuid(7, 0);
        names(out, "leaf 7 EBX", seven.ebx, &LEAF7_EBX);
        names(out, "leaf 7 ECX", seven.ecx, &LEAF7_ECX);
        names(out, "leaf 7 EDX", seven.edx, &LEAF7_EDX);
        if seven.eax >= 1 { names(out, "leaf 7.1 EAX", cpuid(7, 1).eax, &LEAF7_1_EAX); }
    }
    if max_ext >= 0x8000_0001 { let e = cpuid(0x8000_0001, 0); names_or(out, "ext 1 EDX", e.edx, &EXT1_EDX, &LEAF1_EDX); names(out, "ext 1 ECX", e.ecx, &EXT1_ECX); }
    if max_ext >= 0x8000_0007 { let _ = writeln!(out, "  invariant TSC     {}", cpuid(0x8000_0007, 0).edx & (1 << 8) != 0); }
    caches(out, if intel || max_ext < 0x8000_001D { 4 } else { 0x8000_001D }, zero.eax);
    if !intel && max_ext >= 0x8000_0006 && max_ext < 0x8000_001D { amd_caches(out); }
    topology(out, zero.eax);
    xsave(out, zero.eax);
    frequencies(out, zero.eax, intel);
    out.push_str("CPUID (raw: leaf.subleaf, the boot CPU)\n");
    raw(out, 0);
    raw(out, 0x8000_0000);
    if one.ecx & (1 << 31) != 0 { raw(out, 0x4000_0000); }
}

fn caches(out: &mut String, leaf: u32, max: u32) {
    if leaf == 4 && max < 4 { return; }
    out.push_str("Caches\n");
    for sub in 0..16 {
        let r = cpuid(leaf, sub);
        let kind = match r.eax & 0x1F { 0 => break, 1 => "data", 2 => "instruction", 3 => "unified", _ => "other" };
        let (ways, partitions, line, sets) = ((r.ebx >> 22) + 1, ((r.ebx >> 12) & 0x3FF) + 1, (r.ebx & 0xFFF) + 1, r.ecx + 1);
        let bytes = ways as u64 * partitions as u64 * line as u64 * sets as u64;
        let _ = writeln!(out, "  L{} {:<11} {} KiB, {}-way, line {} B, shared by {} threads{}", (r.eax >> 5) & 7, kind, bytes / 1024, ways, line, ((r.eax >> 14) & 0xFFF) + 1,
            if r.edx & 2 != 0 { ", inclusive" } else { "" });
    }
}

// AMD's older cache leaves (0x8000_0005, 0x8000_0006), where leaf 0x8000_001D is missing.
fn amd_caches(out: &mut String) {
    let (l1, l2) = (cpuid(0x8000_0005, 0), cpuid(0x8000_0006, 0));
    let _ = writeln!(out, "  L1 data        {} KiB, line {} B\n  L1 instruction {} KiB, line {} B\n  L2 unified     {} KiB, line {} B\n  L3 unified     {} KiB",
        l1.ecx >> 24, l1.ecx & 0xFF, l1.edx >> 24, l1.edx & 0xFF, l2.ecx >> 16, l2.ecx & 0xFF, (l2.edx >> 18) * 512);
}

fn topology(out: &mut String, max: u32) {
    let leaf = if max >= 0x1F { 0x1F } else if max >= 0xB { 0xB } else { return };
    out.push_str("Topology (the boot CPU's view)\n");
    for sub in 0..8 {
        let r = cpuid(leaf, sub);
        let kind = match (r.ecx >> 8) & 0xFF { 0 => break, 1 => "SMT", 2 => "core", 3 => "module", 4 => "tile", 5 => "die", _ => "other" };
        let _ = writeln!(out, "  level {} {:<7} {} logical CPUs, ID shift {}", sub, kind, r.ebx & 0xFFFF, r.eax & 0x1F);
    }
    if max >= 0x1A { let r = cpuid(0x1A, 0); if r.eax != 0 { let _ = writeln!(out, "  this core's type  {:#X} ({})", r.eax >> 24, match r.eax >> 24 { 0x20 => "Atom, efficient", 0x40 => "Core, performance", _ => "?" }); } }
}

fn xsave(out: &mut String, max: u32) {
    if max < 0xD { return; }
    let zero = cpuid(0xD, 0);
    let supported = zero.eax as u64 | (zero.edx as u64) << 32;
    let _ = writeln!(out, "Vector state (XSAVE)\n  supported XCR0    {:#X}; enabled by this kernel {:#X} ({}); area {} B for the enabled ones, {} B for all",
        supported, crate::context::saved_state(), if crate::context::saved_state() == 0 { "FXSAVE" } else { "XSAVE" }, zero.ebx, zero.ecx);
    names(out, "leaf D.1 EAX", cpuid(0xD, 1).eax, &XSAVE_1_EAX);
    for (bit, name) in XSAVE_COMPONENTS.iter().enumerate().skip(2) {
        if supported & (1 << bit) != 0 { let r = cpuid(0xD, bit as u32); let _ = writeln!(out, "  component {:<2} {:<20} {} B at offset {}", bit, name, r.eax, r.ebx); }
    }
}

fn frequencies(out: &mut String, max: u32, intel: bool) {
    out.push_str("Frequencies and power\n");
    let _ = writeln!(out, "  TSC measured      {} MHz", crate::clock::tsc_hz() / 1_000_000);
    if max >= 0x15 { let r = cpuid(0x15, 0); if r.eax != 0 && r.ebx != 0 { let _ = writeln!(out, "  TSC/crystal       {}/{}, crystal {} Hz", r.ebx, r.eax, r.ecx); } }
    if max >= 0x16 { let r = cpuid(0x16, 0); if r.eax != 0 { let _ = writeln!(out, "  nominal           base {} MHz, maximum {} MHz, bus {} MHz", r.eax & 0xFFFF, r.ebx & 0xFFFF, r.ecx & 0xFFFF); } }
    let intel_msrs: &[(u32, &str)] = &[(0x17, "IA32_PLATFORM_ID"), (0x1B, "IA32_APIC_BASE"), (0x3A, "IA32_FEATURE_CONTROL"), (0xCE, "MSR_PLATFORM_INFO"), (0xE2, "MSR_PKG_CST_CONFIG_CONTROL"),
        (0x198, "IA32_PERF_STATUS"), (0x199, "IA32_PERF_CTL"), (0x19C, "IA32_THERM_STATUS"), (0x1A0, "IA32_MISC_ENABLE"), (0x1A2, "MSR_TEMPERATURE_TARGET"), (0x1AD, "MSR_TURBO_RATIO_LIMIT"),
        (0x1B0, "IA32_ENERGY_PERF_BIAS"), (0x1B1, "IA32_PACKAGE_THERM_STATUS"), (0x606, "MSR_RAPL_POWER_UNIT"), (0x610, "MSR_PKG_POWER_LIMIT"), (0x614, "MSR_PKG_POWER_INFO"),
        (0x770, "IA32_PM_ENABLE"), (0x771, "IA32_HWP_CAPABILITIES"), (0x774, "IA32_HWP_REQUEST"), (0x10A, "IA32_ARCH_CAPABILITIES")];
    let amd_msrs: &[(u32, &str)] = &[(0x1B, "IA32_APIC_BASE"), (0xC001_0010, "SYS_CFG"), (0xC001_0015, "HWCR"), (0xC001_001A, "TOP_MEM"), (0xC001_001D, "TOP_MEM2"),
        (0xC001_0061, "P-STATE CURRENT LIMIT"), (0xC001_0062, "P-STATE CONTROL"), (0xC001_0063, "P-STATE STATUS"), (0xC001_0064, "P-STATE 0"), (0xC001_0065, "P-STATE 1"),
        (0xC001_0066, "P-STATE 2"), (0xC001_0067, "P-STATE 3"), (0xC001_0068, "P-STATE 4"), (0xC001_0069, "P-STATE 5"), (0xC001_006A, "P-STATE 6"), (0xC001_006B, "P-STATE 7"),
        (0xC001_02B0, "CPPC CAPABILITY 1"), (0xC001_02B1, "CPPC ENABLE"), (0xC001_02B3, "CPPC REQUEST"), (0xC001_0299, "RAPL POWER UNIT")];
    for &(number, name) in if intel { intel_msrs } else { amd_msrs } {
        match msr(number) { Some(v) => { let _ = writeln!(out, "  MSR {:08X} {:<26} {:016X}", number, name, v); } None => { let _ = writeln!(out, "  MSR {:08X} {:<26} (absent)", number, name); } }
    }
    if intel {
        if let Some(v) = msr(0xCE) { let _ = writeln!(out, "  ratios            maximum non-turbo {} (x100 MHz), minimum {}", (v >> 8) & 0xFF, (v >> 40) & 0xFF); }
        if let Some(v) = msr(0x1AD) { let _ = writeln!(out, "  turbo ratios      1 core {}, 2 cores {}, 3 cores {}, 4 cores {} (x100 MHz)", v & 0xFF, (v >> 8) & 0xFF, (v >> 16) & 0xFF, (v >> 24) & 0xFF); }
        if let Some(v) = msr(0x771) { let _ = writeln!(out, "  HWP performance   highest {}, guaranteed {}, most efficient {}, lowest {}", v & 0xFF, (v >> 8) & 0xFF, (v >> 16) & 0xFF, (v >> 24) & 0xFF); }
    }
}

/// The platform section: what the kernel chose on this machine.
pub fn kernel(out: &mut String) {
    let _ = writeln!(out, "  architecture      x86-64\n  interrupt mode    {}\n  vector state      XCR0 {:#X}", if crate::cpu::x2apic() { "x2APIC" } else { "xAPIC" }, crate::context::saved_state());
}
