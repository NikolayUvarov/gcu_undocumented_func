# NET ready: `wlan` asks the radio before each tune, so no tune hides a restart (550-NET-0061)

**From:** NET · **To:** LANDING · **Concerns:** 550-NET-0061 (advances; it stays open for DRV's Ryzen runs)

**Ready:** `claude/550-NET-0061-checked-before-a-tune` e09166de

- **What it carries:**
  - **`wlan/src/main.rs`:** `apply` asks `info` before each tune of a radio the station tuned. A restart found there is handled in the next turn: the station starts anew under its country, the try under way ends, and a client's parked scan is scanned again. The case is DRV's Ryzen run of 2026-11-13: the automatic join's timer tuned a fresh `rtw89_wifi` before the 10-s check ran.
  - **`tests/qemu_smoke.py`:** `wifi_join_check` accepts both forms of the restart line. It adds a check: the station leaves, `wlansim` is killed, and a client's scan runs as soon as the fresh one is ready.
  - **The issue's progress** and its row in `issues/README.md`.
- **On `main` d35515e0,** merged in; a conflict in `issues/README.md` only (550-NET-0062's row, closed on `main`), resolved as `main` has it.
- **Tested:**
  - **This session's container** (QEMU 8.2.2, TCG), at a33d4012: the x86 `net` suite passed. With `wlan` as on `main`, the new check failed, as it should.
  - **The maintainer's machines,** at 68d32639 (e09166de adds only the issue's text): "x86: network, TLS and Effector" on acc-1 (1464 s) and "aarch64: network, TLS and Effector" on acc-2 (1232 s) passed. On both, the new check found the restart before the scan's first tune.
