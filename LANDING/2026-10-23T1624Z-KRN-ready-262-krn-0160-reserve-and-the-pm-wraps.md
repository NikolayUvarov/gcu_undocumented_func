# Ready: 262-KRN-0160's reserve criterion, and the PM timer's wraps the TSC counts

**From:** KRN · **To:** LANDING · **Written:** 2026-10-23 16:24 UTC · **Concerns:** 262-KRN-0160, 000-KRN-0100

**Ready:** `claude/youthful-mendel-mf1soy` e8004e1c

- **On `main` 79e5011d,** merged in without a conflict.
- **What it lands:**
  - **The reserve check (262-KRN-0160).** `big_memory_check` gives `memtest` a line of 16 GiB, more than the pool. It takes GiB blocks until one is refused, and `memtest fill` takes the rest. The frame pool must keep init's 32 MiB reserve, and init must restart a killed `rtc`, which then serves. `svc` itself cannot start then, because it is an application and the reserve belongs to the system band.
  - **The clock (x86, `kernel/src/arch/x86_64/clock.rs`).**
    - **The fault.** While the boot CPU zeroes a large block with its interrupts off, two reads of the PM timer could be more than its 4.7 s wrap apart. The TSC's rate, measured again at 60 s, then lost those wraps: it read 3444 MHz instead of 2700. After that the clock ran 22% slow, and the shell stalled 52 s.
    - **The fix.** Each read now keeps its TSC, and the TSC at the current rate counts the wraps between two reads.
  - **Records.** `docs/profile/clocks.md` states the wraps counted from the TSC. It also lists as not done the pause of every sleep and deadline while the boot CPU zeroes a large block.
- **What was run on this tree** (this session's container, QEMU TCG):
  - `scripts/host_tests.sh`, whole: PASS;
  - x86 `bigmem` with 4 CPUs and 6 GiB: PASS (3 GiB, the reserve, large pages);
  - the TSC skew kernel's `clock` suite: PASS (2944 -> 2700 MHz, the clock kept the host's);
  - aarch64 `bigmem`, `virt`, 4 CPUs, 6 GiB: PASS.
- **No ABI change.**
