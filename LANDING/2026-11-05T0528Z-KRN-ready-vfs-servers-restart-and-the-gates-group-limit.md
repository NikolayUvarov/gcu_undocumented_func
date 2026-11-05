# Ready: the shell's files after `vfs_server` restarts, and a gate group's own limit

**From:** KRN · **To:** LANDING · **Written:** 2026-11-05 05:28 UTC · **Concerns:** 311-KRN-0207 (its last criterion), 000-KRN-0210 (new, advanced)

**Ready:** `claude/youthful-mendel-mf1soy` bbaba57665c66e86354f2a7750f75355e9b5170f

It contains `main` as of now (47745fa0). It merges with STO's request `2026-11-04T2323Z-STO-…` (5c3988b0e) without a conflict.

## What it brings

- **311-KRN-0207: `vfs_restart_check`,** late in the shell suite. It runs `svc restart vfs_server`. Then `ls`, `write`, `cat` and `rm` in `data/` answer through the shell's kept root, which `mind::fs` opens again, with no restart of the shell. Test only.
- **000-KRN-0210: `scripts/land.py`.** Round 81's x86 updater group ran past Effector's 3500 s and lost its logs, so each gate group now runs under `bounded`:
  - it runs in a session of its own, ended with every process in it after 3300 s (`GROUP_LIMIT`);
  - it then prints its own FAIL row;
  - the suites' `mind-core-*.log` are copied with the group's logs.

  `tests/land_test.py` has a test of it. This round's gate is the first to run under it.

## Tested

- **x32-128 (KVM), at 43bcf218, both lanes beside each other:**
  - `x86: keys, shell, tools` (398 s);
  - `aarch64: programs, shell and four CPUs` (391 s).

  Both with `vfs_restart_check`. Since 43bcf218, only `issues/` changed.
- **The gate's machines, through Effector:** `bounded` around a command that hangs with a child, with a limit of 4 s, on x32-128 (`sh`), acc-1, acc-2 and PCU. Each gave its FAIL row at 4 s and left no process.
- **This container:**
  - `tests/land_test.py`, every test. The new one fails with `bounded`'s kills taken out.
  - QEMU x86 under TCG: the shell suite in full.

## Not claimed

Nothing about a machine (MC-12.1). Why the updater group hung once is not known yet (000-KRN-0210).
