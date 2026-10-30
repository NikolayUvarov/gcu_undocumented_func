# KRN landing request: 750-KRN-0192, `COMPOSITOR_PULL` names the PID whose screen is in front (ABI 26)

**From:** KRN · **To:** LANDING · **Written:** 2026-10-30 15:41 UTC

**Ready:** `claude/youthful-mendel-mf1soy` d7635f12c46252594346c0897ebdb9d46a469487

## What it brings (`main` 641aad2a merged)

- **750-KRN-0192** (made; closes once landed), for EFF's 750-KRN-0193:
  - every pull but a release returns in `arg2` the PID of the task whose screen is in front, 0 for none;
  - `mind::dev::compositor_pull` and `compositor_pull_frame` return `(Frame, pid)`;
  - the compositor logs `[COMPOSITOR] SCREEN OF PID <n> IN FRONT` when that changes;
  - ABI 26. The handoff stays 25: an ABI change alone leaves it. `tests/boot_abi_host.rs` now states the rule: a handoff is above every ABI an older loader wrote in its place (18 and older).
- **The `normal` suite** holds app2 in front 300 ms and, at its end, finds the shell's PID, app2's and the shell's again in that order.
- **Docs:** `README.md` and `docs/api/README.md` (the call's table, ABI 26).

## Tested

- **Locally, x86 QEMU under TCG:** the build, host tests, the x86 test programs, and the `normal` suite (07a3eb2a, d05e4669).
- **On d7635f12 itself, through Effector** (`branch.py`, GREEN):
  - `aarch64: programs, shell and four CPUs` on acc1;
  - `x86: boot, display, shell, memory, clock` on acc2;
  - `host tests` on rz.
- **Every program is rebuilt against ABI 26**, so the whole gate is the real test.
