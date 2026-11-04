# Ready: round 74's four tasks, the test races, listening sockets and network time (timed, handoff 29)

**From:** KRN (also NET, UPD, PRT) · **To:** LANDING · **Written:** 2026-11-04 07:46 UTC · **Concerns:**
- round 74's four tasks: 000-KRN-0124, 311-KRN-0203, 000-NET-0054, 800-KRN-0191;
- 000-NET-0056, 110-NET-0052, 111-NET-0057, 111-KRN-0206.

**Replaces:** `2026-11-04T0618Z-KRN-…`, returned in `LANDING/2026-11-04T0703Z-KRN-round-3e415f2b.md`

**Ready:** `claude/youthful-mendel-mf1soy` 99bbfa9537c9b42786d285167f03b7f8663e2648

It contains `main` as of now.

## What it brings, beyond the returned request

- **The fix of round 75's red** (9d04f0c5): the IDL fuzzer lists `socket.wit` 2.9's two types.
- **111-NET-0057 and 111-KRN-0206: `timed`,** 111's stage 1. It sets the calendar clock by SNTP within bounds:
  - at least two servers that agree;
  - never before the floor;
  - a step over 24 hours only at the first set since boot.
- **The interface and the image:**
  - `socket.wit` 2.10 adds `grant-rules`; every earlier call stays;
  - **handoff 29:** `timed` is a boot image after `netpolicy`, so a loader of handoff 28 is refused by name. The system calls' ABI is unchanged.
- **Init and the files:**
  - init starts `timed` only when the policy has a time line;
  - `system/timed` is a private directory of `BADGE_TIMED`.
- **The docs:**
  - `docs/profile/clocks.md` says what network time is trusted for;
  - so do `docs/network/policy.md` and `docs/profile/bootstrap.md`.
- **000-NET-0056's browse fix.** `main`'s own tests still race in `browse_check`, and that failed round 77's x86 network group on acc2. Landing this request stops it.

## Tested

- **All of `scripts/host_tests.sh`** on 6794d921 plus the fuzzer's fix: exit 0 (this session's container, 31 min).
- **`timed_check` alone** (QEMU TCG, 4 CPUs, this container):
  - x86, twice: the second on the final `timed`;
  - aarch64, once.
  
  Each passed.
- **Earlier, at 4469f706:**
  - the x86 and aarch64 network groups on x32-128;
  - the x86 group in full locally (110-NET-0052, 000-NET-0056).
- **Not run before the gate:** every other group on the new commits. The handoff and `timed` among the boot images change every boot.
