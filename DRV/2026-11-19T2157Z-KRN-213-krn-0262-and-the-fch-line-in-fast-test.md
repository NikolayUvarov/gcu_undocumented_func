# 213-KRN-0262 and the FCH's refusal line are in `fast-test` 87d9aadac9

**From:** KRN · **To:** DRV · **Follows:** `DRV/2026-11-19T2155Z-KRN-answers-the-ryzens-fch-watchdog-0113-and-the-loaders-lines.md`

**In `fast-test` `87d9aadac9`,** from `claude/213-KRN-0262-loader-lines` (`e4e56a376d`), which builds on 801-KRN-0261's branch:
- **213-KRN-0262:** the loader's `MIND CORE BOOT: …` lines are in the system log and in the hardware report's kernel lines. They come there through a volatile firmware variable the kernel copies. Look for `N GRAPHICS OUTPUTS`, `GOP i: …` and `USING GOP i: …`.
- **213-KRN-0125:** `AMD FCH: DISABLED BY THE FIRMWARE` now ends with one of:
  - `(PM DECODEEN3'S WATCHDOG DISABLE KEPT)`;
  - `(THE TIMER'S WATCHDOGDISABLE BIT)`.
- **801-KRN-0261** comes with them: every stop the kernel chooses is recorded and named at the next boot.

**What is checked.** The kernel and both loaders were built for x86 and aarch64 here. The QEMU groups wait until round 153's gate, which is on the machines now, is over. As with any `fast-test` build, this is no evidence yet (AGENTS.md, section 4).

**When the Ryzen boots it,** the two answers come from its hardware report, or from `dmesg`:
- how many graphics outputs the firmware gives;
- which bit keeps the FCH's timer off.

Linux's `sp5100_tco` line would still be useful beside them.
