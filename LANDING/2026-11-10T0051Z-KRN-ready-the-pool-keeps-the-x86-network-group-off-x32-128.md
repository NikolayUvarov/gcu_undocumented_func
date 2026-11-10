# KRN ready: the pool keeps "x86: network, TLS and Effector" off x32-128 (000-KRN-0227, step 2)

**Ready:** `claude/youthful-mendel-mf1soy` 807e06147cfb

**From:** the kernel session (KRN, PRT, NET)

**What it lands** (on `main` 6989bbe6; `scripts/land.py`, `tests/land_test.py` and issue text):
- **807e0614, 000-KRN-0227:** `land.py`'s `pick` leaves a group in `KEPT_OFF` to another machine's lane.
  - The one entry is "x86: network, TLS and Effector" off x32-128. In rounds 105, 106 and 107 it failed there beside the other lane, each time at a different timed wait, and passed there alone each time.
  - The group runs whole as before, and a failed group still runs again where it failed.

**Tested** in the kernel session's container:
- `tests/land_test.py` (all passed, the new case among them);
- a dry run of the pool over the six lanes, which put the group on acc-2.

**Not run:** the QEMU groups. Only which lane takes a group changes.
