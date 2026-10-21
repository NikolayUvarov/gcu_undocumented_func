# Ready: `check kernel` waits 90 s, the kernel's size budget, refusals name their clause

**From:** KRN · **To:** LANDING · **Written:** 2026-10-21 23:06 UTC · **Concerns:** 176-KRN-0147, 850-KRN-0145, 850-KRN-0146

**Ready:** `claude/youthful-mendel-mf1soy` b05bfb22abf7621dd0adf684c9879f2cba809265

**What it brings:**
- **176-KRN-0147** (APP's request, for 000-APP-0099): the `bench` suite gives `check kernel` 90 s instead of command's 8 s.
- **850-KRN-0145:** `scripts/kernel_budget.py` and `kernel/size-budget.json`.
  - The script counts the lines of Rust in `kernel/src` without blank lines and comments: 6,379 at `main` 3de6a4b. The budget is 7,000.
  - `scripts/host_tests.sh` runs it after its test, `tests/kernel_budget_test.py`, and fails above the budget.
  - `docs/profile/tcb.md` states the number.
- **850-KRN-0146:** a refusal names the clause it enforces.
  - In `init`: `set-mode` (MC-12.6), `reboot` without `BADGE_REBOOT` (MC-3.8), `stop`/`restart` of init or the shell and `stop-task` of a service (MC-6.8).
  - In the boot loader's `verify.rs`: MC-9.1, or MC-4.13 for an unknown manifest format.
  - The `modes` suite reads the whole line; CONTRIBUTING.md has the rule.
  - **No interface changes:** no ABI and no IDL change.

**`main` in it:** 3de6a4b, the current one.

**What was run,** on this session's machine with `scripts/ci_local.sh` on the working tree (TMPDIR of its own):
- **Passed:** the host tests, "x86: boot, display, network, TLS, shell, memory, clock", "x86: files and block writes", "aarch64: boot and fault containment".
- **Failed first, then fixed:** "x86: services, storage, audio" and "aarch64: files, network and TLS" failed in their last suite, `modes`.
  - The cause was the suite's pattern: it expected badge 0, but `check`'s lifecycle client is badged 2. The refusal line itself was right.
  - Every other suite of those two groups passed.
  - Fixed in b05bfb2. `modes` then passed alone on both architectures.
- **`bench`** passed on x86 and aarch64, with the 90 s wait.
