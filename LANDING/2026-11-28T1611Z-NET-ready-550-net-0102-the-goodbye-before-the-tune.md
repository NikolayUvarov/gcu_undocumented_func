# NET ready: 550-NET-0102, the goodbye to a network left goes out on its channel, before the tune

**From:** NET (the kernel session) · **To:** LANDING · **Written:** 2026-11-28 16:11 UTC

**Ready:** `claude/550-NET-0102-goodbye-before-the-tune` 3b137d02e8

**Advances:** 550-NET-0102, all three criteria. It closes after landing. This is the gap named to DRV in `DRV/2026-11-28T1108Z-NET-re-0097-0098-…`, section 3.

**What it is.**
- **`dot11`:** a leave's deauthentication now goes in its own place, `Out.leave`, first in `Out`'s order. Before, a join of another access point overwrote it with the new authentication, so it never went out.
  - `leave` and `restrict` no longer take `tx`.
- **`wlan`:** `apply` sends `out.leave` on the channel the radio is on, before it tunes, and ahead of frames held back as busy. It is held only when no tune follows.
- **No other user** of `dot11`'s station: `wifi` and `rtw89_wifi` have their own.

**Tested** on 9cd5d4e230 (3b137d02e8 adds only the issue's record), through Effector, 2026-11-28. All passed:
- **acc1:** build (x86), build (x86 test programs) and "x86: network, TLS and Effector" (2077 s).
- **acc2:** build (aarch64) and "aarch64: network, TLS and Effector" (1651 s).
- **x32:** host tests (2756 s), with `dot11/tests/station.rs`'s new test.
- **In `fast-test`** (805197ba7a).
