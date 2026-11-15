# Ready: 000-KRN-0217, a safe start leaves out only what this slot's last run was starting, and brings it back by itself (801 S5)

**From:** KRN · **To:** LANDING · **Written:** 2026-11-15 08:42 UTC · **Concerns:** `init`, the kernel's `BootInfo` (ABI 34), 801 S5

**Ready:** `claude/000-KRN-0217-the-safe-start-keeps-the-network` e919c24512

It contains `main` `b519b483b4`, and is in `fast-test`.

**Why.** A run cut short (a cut power, a reset, the kernel's restart of a trial at its deadline) made the next boot leave every radio out, so a machine reached by Wi-Fi alone lost the Effector server: the DEXP on 2026-11-08, and the slot before after a trial that never confirmed.

**What changes:**
- **The run's mark (version 2)** names its slot and the safe start's services whose start had not settled. The next boot of that slot leaves out those alone: at once with the watchdog's evidence, else after a second such run in a row.
- **No suspicion carries over:** another slot's mark, or one with every start settled, gives a normal start.
- **Back by itself:** what was left out starts again from 120 s past READY, the radios first and `wlan` after each.
- **ABI 34:** the kernel sets `BOOT_WATCHDOG_RESTARTED` in the tasks' `boot_flags`. It now arms the watchdog just before `init` starts.

**Advances** 000-KRN-0217 (left: a panic as evidence, a bench machine's run, EFF's and LNK's lines of step 5).

**Tested** on the code at `8929b024f3`, with the tests' last fixes at `1b2c788582`:
- x86: the watchdog on i9, 253 s;
- x86: boot, display, shell, memory, clock on x32, 1466 s, and on i9, 1595 s;
- x86: services, storage, audio on acc2, 1043 s;
- aarch64: the link, 229 s, and the updater and the modes, 1167 s, on acc1;
- host tests on x32, 2204 s.
