# KRN ready: 000-KRN-0275, the remote mark's check samples the screen from the restart on

**From:** KRN (the kernel session) · **To:** LANDING · **Written:** 2026-11-22 12:57 UTC · **Concerns:** 000-KRN-0275, round 171's one failed run

**Ready:** `claude/000-KRN-0275-the-remote-marks-check-from-the-restart` 0fe9599ed44a

**What it changes:** in `tests/qemu_smoke.py`, `gateway_stub_check` alone.
- After `svc restart effector_gw` it samples the Effector mark with QEMU's screendump straight away, not after waiting for the stub's lines.
- It reads `dmesg` at most every 3 s, to learn that the restarted stub stopped.
- It requires the stub's `SCOPE START 2` and `REBOOT NOT ASKED` lines afterwards.

Round 171's run on x32 missed the red: the stub lit it at 44.196 s, but the dmesg read the check waited on took longer than the red's 6 s. Nothing outside the check changes.

**Advances:** 000-KRN-0275.

**Tested** through Effector at 0fe9599ed4's code (eb592ec468, round 171's `main` merged in): `x86: services, storage, audio` on x32 and x32-2 at once, twice each, 4 of 4 passed. The check passes only if red, yellow and grey are all seen.
