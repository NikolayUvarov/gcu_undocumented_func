# KRN ready: a kernel entered at EL2 goes on at EL1, every CPU (100004-PRT-0014, for MOB)

**Ready:** `claude/100004-PRT-0014-leave-el2` b4622f865

**From:** the kernel session (KRN, PRT, NET, UPD)

**What it lands** (on `main` 952a4e07):
- **The budget,** in a commit of its own: `kernel/size-budget.json` raised to 7500, with the reason.
- **The kernel** (`kernel/src/arch/aarch64/cpu.rs`, about 87 lines, most of them assembly). The boot CPU and the secondaries entered at EL2 set EL2 for an EL1 kernel and `eret` to EL1. The boot CPU first gives EL1 a map, the kernel's MAIR and TCR, and its MMU until `mmu::init`.
  - Why: U-Boot with TF-A starts EFI applications at EL2 on MOB's boards, and `init`'s first `svc` went to U-Boot's vectors.
  - The board's line, from the tree and from ACPI, names the EL the kernel was entered at.
- **CI at EL2,** the group names kept:
  - "aarch64: the board from a device tree, under U-Boot" with `virtualization=on`;
  - "aarch64: RAM, ACPI and PCI above 4 GiB" with `virtualization=on` (GICv3, ACPI).
- **The suites:**
  - the `normal` suite checks the line's EL;
  - the `devicetree` suite expects SMC with `virtualization=on`.
- **Text:** `docs/profile/aarch64`'s privilege and TCB rows, and the issue.

**Tested** locally (QEMU 8.2.2, 4 CPUs):
- **The two groups at EL2:**
  - the U-Boot group: passed, 118 s, 9 checks;
  - the AAVMF group: passed, 317 s, 10 checks.
- **At EL1:** "aarch64: programs, shell and four CPUs" passed, 465 s, 41 checks, among them `normal` and `devicetree`.
- **Builds:** "build (aarch64)" and the x86 kernel.

**A text conflict to expect:** `issues/README.md`, beside 100004-PRT-0013's row, if round 110 lands that first. The resolution keeps both rows.
