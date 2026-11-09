# KRN ready: the revocation check's walks, and a restarted shell's console channel

**Ready:** `claude/youthful-mendel-mf1soy` f6718613acf9

**From:** the kernel session (KRN, PRT, NET)

**What it lands** (on `main` 73804c22; tests and issues only, no program's code):
- **125a87a6, 000-KRN-0225:** round 103's one failure, `revocation_refusal_check` with no refused mint, on x32-2.
  - The kernel refuses a mint made during the walk, but the walk lasts about 2 ms. Nothing promises that the child's vCPU runs in those 2 ms on a loaded host.
  - With more than one CPU, the check now makes up to three walks until one has refused a mint. The refusal, the count removed, the steps and no fault are still required.
- **4fc145de, f6718613, 100000-KRN-0222:** the `devicetree` suite's channel check kills the shell.
  - The shell init restarts gets the same channel, which init keeps for restarts, and answers through it.
  - No line of init's reaches the console meanwhile.
  - The issue records that the channel stands for the boot, and the owner's choice that follows from it, for 100000-KRN-0221.

**Tested:**
- **The `isolation` suite:**
  - in the kernel session's container (QEMU 8.2.2, TCG) on 4 CPUs and on 1 CPU;
  - on x32-128 through Effector, with the x2APIC group's kernel and CPU model, beside 24 busy threads: 20 times on 4 CPUs, each passing at its first walk, and 5 times on 1 CPU.
- **The aarch64 `devicetree` suite** in the container, after a fresh aarch64 build of this tree: passed, the channel check with the restart among it.
- **Not run:** the other groups.
