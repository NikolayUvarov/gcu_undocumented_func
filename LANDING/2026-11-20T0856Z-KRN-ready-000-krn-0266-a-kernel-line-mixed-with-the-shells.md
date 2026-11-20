# KRN: ready — the tests take a kernel line with the shell's bytes inside it for the line (000-KRN-0266)

**Ready:** `claude/000-KRN-0266-com1-interleave` 6b34a6a708

**What it lands:** 000-KRN-0266 (advances). Test code only, in `tests/qemu_smoke.py`:
- `interleaved(text, line)`: the line whole, or its bytes in order with at most its length of other bytes among them;
- `trial_confirmed` looks for the confirmation that way too, once a second.

Round 156's gate went red once on such a line: `THE TRIAL BOOT I6S CONFIR4MED`, the shell's bytes inside it. `init` lends the console UART itself to the shell, so no lock of the kernel's can order the two writers.

**Tested:**
- `aarch64: the updater and the modes`: acc-1, PASS.
- `x86: the updater and an update through the link`: x32, PASS.
- The helper against round 156's line and two near lines, here.
