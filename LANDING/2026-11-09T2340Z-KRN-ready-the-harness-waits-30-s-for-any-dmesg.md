# KRN ready: the QEMU harness waits 30 s for any `dmesg` (000-KRN-0227)

**Ready:** `claude/youthful-mendel-mf1soy` d6207f2d681f

**From:** the kernel session (KRN, PRT, NET)

**What it lands** (on `main` 898e4a11; `tests/qemu_smoke.py` and issue text only):
- **d6207f2d, 000-KRN-0227 (new):** round 106's one failure, the Effector suite's `dmesg -s effector` past its 8 s on x32-128 beside its other lane.
  - `VM.command` waits `DMESG_WAIT` (30 s) for a `dmesg` command unless a check says otherwise, and 8 s for any other command, as before.
  - The Wi-Fi checks' explicit 30 s waits (000-NET-0063) are taken out, as the default covers them.
- 000-NET-0063's progress, and round 105's `run netcheck &` on x32-2 recorded in 0227, not handled yet.

**Tested** in the kernel session's container (QEMU 8.2.2, TCG, 4 CPUs): the x86 `normal` suite, 5 checks.
- **Not run:** the other suites; the gate's run is their test. Only how long a `dmesg` command is waited for changes, so a passing run is the same.
