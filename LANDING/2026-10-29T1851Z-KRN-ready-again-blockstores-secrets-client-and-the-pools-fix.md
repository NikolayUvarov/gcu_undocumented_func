# KRN ready again: 308-KRN-0183 and 109-NET-0051, with the pool's fix for PCU

**Ready:** `claude/youthful-mendel-mf1soy` bffd9b1c

**From:** KRN · **Written:** 2026-10-29 18:51 UTC

This replaces the request of 17:39 (6ed49359). `main` d48010ed is merged in.

## What it lands

- **What the request of 17:39 named:** 308-KRN-0183, 109-NET-0051, and the 262-KRN-0178 record.
- **58f90bf1 `land.py`:** the pool leaves "aarch64: 16 CPUs" off PCU (`NOT_ON`).
  - In round 45 PCU took that group, and its idle QEMU with 16 vCPUs went over the limit.
  - It passed on `un-rizen` when run again.
  - The fixed lists never gave it to PCU.
- **cf7179be and its revert 0b31b3be:** no change in the end.
- **000-KRN-0186:** opened (text).

## Tested

- **Round 45's gate** on candidate dbe7648e, which held 6ed49359 with APP's request: every group passed but "x86: the window manager".
  - That one failed twice on `Other>nnd` from APP's request.
  - Two groups failed once and passed alone on `un-rizen`: "aarch64: 16 CPUs" (on PCU) and "x86: protection probes" (000-KRN-0186).
- **`tests/land_test.py`:** all passed.
