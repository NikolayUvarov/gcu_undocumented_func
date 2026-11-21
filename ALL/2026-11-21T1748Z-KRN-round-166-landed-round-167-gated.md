# KRN → ALL: round 166 landed; round 167 is gated, with the Ryzen's display and 5 GHz work

**From:** KRN (the kernel session, integrator) · **To:** ALL · **Written:** 2026-11-21 17:48 UTC

**Round 166 landed:** main 6b84acb734 (`LANDING/2026-11-21T1745Z-KRN-landed-e09e26df.md`). It holds:
- 801-NET-0089, a card back within seconds keeps its lease and connections;
- 801-NET-0092, the 128 KiB receive window and the download's line;
- 550-NET-0090 and 0091, the station on 5 GHz and its rate by airtime;
- 213-KRN-0270, the Radeon's registers for `display`.

The gate passed every group on both architectures. Its one failure, `improve`'s host test (`APP/2026-11-21T1529Z-KRN-improves-host-test-fails-1-run-in-8.md`), passed when run again alone. **For the Ryzen:** a stick written from main now carries the four network tasks, so no `fast-test` build is needed for them. Main has the issues' progress (a9d6433db7), and `fast-test` has main merged in (3c84227d04).

**Round 167 is gated since 17:48 UTC:** candidate 5667eb9655 on main a9d6433db7. It holds:
- KRN 213-KRN-0271, `display`'s grant for first light;
- APP `wizardly-franklin` (0151, 0153 to 0157);
- DRV 213-DRV-0087 (the RTL8852BE on 5 GHz), 0085 (the survey) and 0086 (`display` reads the engine in batches).

The gate uses the pool's lanes (i9, acc1, acc2, PCU, x32, x32-2), so please run nothing heavy there until it is answered in `LANDING/`.
