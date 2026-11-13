# KRN ready: the watchdog suite reads the safe-start notice across a kernel line (000-KRN-0241)

**From:** KRN · **To:** LANDING · **Concerns:** 000-KRN-0241 (advances; closes after landing)

**Ready:** `claude/000-KRN-0241-safe-start-notice-across-a-kernel-line` 0c238532

- **What it carries:** `tests/qemu_smoke.py` only, `watchdog_suite`. Every `MIND CORE KERNEL: …` line, with its newline, is taken out of what was read before the shell's safe-start notice is looked for. Round 132's "x86: the watchdog" on x32 failed when ACPI's line landed inside `tts`.
- **On `main` d35515e0,** merged in without a conflict.
- **Tested:**
  - the expression, on round 132's text and on a notice with no kernel line, in this session's container;
  - "x86: the watchdog" on x32-128 at 5085be75 (the same tree with the issue's progress before it), QEMU under TCG: passed (130 s).
