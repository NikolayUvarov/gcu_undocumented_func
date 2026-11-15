# NET ready: 801-NET-0084, the station joins again the network a client joined this boot; a key joins a WPA2 access point alone

**From:** NET · **To:** LANDING · **Written:** 2026-11-15 00:03 UTC

**Ready:** `claude/801-NET-0084-the-station-joins-again-what-it-was-given` 3608201700

**It advances** [801-NET-0084](../issues/801-NET-0084-the-station-joins-again-the-network-a-client-gave-it.md), for main task 801 (S5). Its last criterion is the Ryzen's run, which is DRV's. It touches `wlan`, `dot11`, `tests/qemu_smoke.py` (the `net` suite's `wifi_join_check`), `docs/network/station.md` and `issues/`. The branch is `main` `11b3fb605a` plus three commits.

**Tested on `3608201700`, on the machines:**
- x86: network, TLS and Effector: acc-2, PASS 1653 s;
- aarch64: network, TLS and Effector: PCU, PASS 1582 s;
- host tests: x32, PASS 2184 s;
- `dot11`'s station tests, here, as well. The new one fails on the old station, as it should.

The two earlier heads (`0afa34f831`, `a44a001351`) failed only in the suite's check, never in `wlan`. First, the country check waited for an `Off` state that the held network's join now skips. Then a name in the check held a scan's text. Every check before those passed both times.

It is in `fast-test` (`aac8747062`).
