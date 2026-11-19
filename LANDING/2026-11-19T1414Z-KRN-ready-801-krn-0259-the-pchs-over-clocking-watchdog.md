# Ready: 801-KRN-0259, the PCH's over-clocking watchdog, with 801-KRN-0258

**From:** KRN · **To:** LANDING · **Concerns:** 801-KRN-0258, 801-KRN-0259 · **Contains:** `LANDING/2026-11-19T1028Z-KRN-ready-801-krn-0258-armed-as-the-kernel-starts.md` (404448cc65 is in this branch; landing both is the same as landing this one)

**Ready:** `claude/801-KRN-0259-the-pchs-over-clocking-watchdog` 2094f39e05

**What it adds to 0258:** the Alder Lake-N PCH's over-clocking watchdog (`OC_WDT_CTL`, PM base + 0x54), armed beside the TCO.
- **When:** as the kernel starts, now once the FADT is read.
- **The rest:** fed by the tick, stopped by a chosen halt, given its longest count before a reset the kernel makes without stopping it. The DEXP firmware's mark of an expiry is read as a restart and cleared.

**On the DEXP (2026-11-19, the maintainer at the machine):**
- **Found:** the TCO's SMI stays because the firmware set `TCO_LOCK`.
- **The reset:** two trials stopped before `init` (releases 5 and 9) were each reset by the over-clocking watchdog, and the loader booted the fallback.
- **The boot lines:** after the reset the boot said `IT RESTARTED THE LAST RUN`; after an ordinary reset it did not.

Every criterion of 0258 and 0259 is met; I close both once this lands. The record is in both issues.

**Tested:**
- in this container:
  - `tests/tco_watchdog_host.rs`, 17 passed;
  - `scripts/host_tests.sh` on 0258's head: all but `manifest_test`'s comparison with the `cryptography` library, which this container cannot load (`_cffi_backend`);
  - x86 `boot, display, shell, memory, clock` (1560 s) and `the watchdog` (291 s), both on `1297b1e227`;
  - the x86 and aarch64 kernels build;
  - the kernel's size is 7893 of the 7900 budget.
- The commits after `1297b1e227` change only `issues/` and `docs/`.

In `fast-test` (9d86204d1a).
