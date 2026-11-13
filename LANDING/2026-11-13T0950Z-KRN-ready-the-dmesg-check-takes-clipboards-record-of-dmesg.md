# KRN ready: the dmesg -n 3 check takes clipboard's record of dmesg itself

**Ready:** `claude/000-KRN-0235-clipboard` b65e1d3419

**Closes** 000-KRN-0235 once it lands: APP's tools checks and the USB check's fix are in `main` since round 128.

**What it changes:** `tests/qemu_smoke.py`'s `dmesg_check` alone, with the issue's record. `dmesg -n 3` takes the last three records when it starts and reads on until none is left, so `clipboard`'s `IN FRONT` record for `dmesg` itself may come fourth; that failed round 128's first run of "x86: services, storage, audio". The check now allows at most three records up to the forged line and only `clipboard`'s `IN FRONT` records after it.

**Tested** at e5a27f3a, the fix with round 128's `main` merged in, on x32-128: "x86: services, storage, audio" (666 s) and "x86: one CPU" (136 s) passed. The last commit adds the issue's record only.
