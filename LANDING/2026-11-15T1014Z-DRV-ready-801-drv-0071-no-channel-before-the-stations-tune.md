# DRV landing request: rtw89 admits no channel and takes no rules before the station's own tune (801-DRV-0071)

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-15 10:14 UTC · **Concerns:** 801-DRV-0071, 801 S5

**Ready:** `claude/801-DRV-0071-radios-keep-their-network` 08cd88d2d6ff5a4f02af6dca64d38243418f52c0

## What it carries

The branch's earlier request landed in round 137 (979edc58). This one adds one commit, 5e8dd25f, and merges `main` b519b483 in. The merge had no conflict.

- **`rtw89_wifi/src/receive_service.rs`:** before the station's own `set-channel`, a send is `Failed`.
  - The driver takes neither the world rules nor its receive-only channel, and `info` keeps channel 0.
  - On the Ryzen (2026-11-14 20:50 UTC, `fast-test` 8c067849), a restarted rtw89 got a send from a station that still thought itself joined. It took the world rules and admitted its own channel 1. `info` then said 1, the station never saw the restart, and the machine was off its Effector server for 13 h.
- **`tests/rtw89_receive_service_cases.rs`:**
  - a new case, `a_send_before_the_stations_tune_is_refused_and_channel_0_stays`;
  - the two cases that sent before a tune now tune first.
- **Text:** the issue's progress.

In `fast-test` since 5a097e83.

## Tested

- **On LEG9, on this tree:**
  - all of `scripts/host_tests.sh`: 234 groups, 6283 passed, 0 failed. This includes `tests/rtw89_service_host.rs` (338 passed) and the ssh tests with OpenSSH's server.
  - `rtw89_wifi` builds for x86.
- **No QEMU suite runs rtw89,** which has no emulated device.
