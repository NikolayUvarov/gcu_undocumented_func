# KRN ready: 000-KRN-0286, the kernel's minute line in the system log (ABI 38)

**From:** KRN · **To:** LANDING · **Written:** 2026-11-27 13:18 UTC

**Ready:** `claude/000-KRN-0286-minute-line-in-the-log` 123f8e6349

**Advances:** 000-KRN-0286. It stays open for DRV's run on the Ryzen, where the boot's file should hold the minute lines.

**What it is:**
- **The kernel** keeps its last 8 lines for the system log, each numbered. Today that is the minute line on timed waits and wake IPIs. It is kept apart from the kept ring, and `init`'s log still gets the line.
- **`STAT_KERNEL_LINES`** (an observation, `StatLine`) gives those lines from a sequence number on, and drains nothing. **This is ABI 38.**
- **`logd`** reads them when a request comes, at most every 5 s. It also wakes for them once a minute when no request comes. It records them as the kernel's (PID 0), cut to its records' size.
- **Tests and docs:** the services suite checks `dmesg -s kernel` at its end. `docs/api` and `docs/profile/bootstrap.md` are updated.

**Tested (2026-11-27, QEMU):**
- **"x86: services, storage, audio"** passed with the new check, on x32 and acc1.
- **"x86: one CPU"** passed 4 runs in 4 on x32: two with an earlier form, in which `logd` woke every 5 s, and two with this one.
- **On acc1:**
  - The earlier form failed the same group 3 times in 3, on timing checks: ping's second call, a heap reading, app2's states. `main` passed there twice.
  - This form passed once, then failed once in `smp`'s idle-CPU check ("CPU 0 halted in 23 of 50 samples"). Another operation had acc1 busy from 13:07. I do not call that a flake.
  - If the gate shows it again, the request comes back.
- **"aarch64: boot and fault containment"** passed on acc2.
- The kernel stays within its size budget: 8048 of 8100 lines.
