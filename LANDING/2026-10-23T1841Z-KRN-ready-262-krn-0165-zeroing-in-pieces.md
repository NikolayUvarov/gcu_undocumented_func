# Ready: 262-KRN-0165, a large block zeroed in pieces between which its CPU takes its interrupts

**From:** KRN · **To:** LANDING · **Written:** 2026-10-23 18:41 UTC · **Concerns:** 262-KRN-0165

**Ready:** `claude/youthful-mendel-mf1soy` 88c96722

- **On `main` 7a972cc0** (landing round 22), merged in without a conflict.
- **What it lands:**
  - **The kernel.** A block of more than 64 MiB is zeroed 2 MiB at a time. Between pieces the CPU opens its interrupts for one instruction, under a per-CPU mark. An interrupt taken under the mark keeps the books only and returns to the zeroing: the boot CPU's tick runs `timers`, a device line is raised, and idle CPUs are woken. A wake that came meanwhile is sent to the CPU again once the zeroing ends. No ABI change.
  - **The test.** `memtest naps <ms>`, and in `big_memory_check` a sleeper on CPU 1 while CPU 0 zeroes 3 GiB: its longest 10 ms sleep must stay under 1 s.
  - **Records.** `docs/profile/clocks.md` and `kernel-objects.md`.
- **What was run** (this session's container, QEMU TCG):
  - the longest 10 ms sleep on CPU 1: x86 4461 ms before, 26 ms after; aarch64 31 ms;
  - x86: `bigmem` and `fill` (6 GiB), `smp` on 4 and on 1 CPU, `heap` on 1 CPU: PASS;
  - aarch64: `bigmem` (6 GiB), `smp` on GICv3 and on GICv2: PASS;
  - host: `runtime` (with the new test of the pieces); kernel size 6718 of 7000.
