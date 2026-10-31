# Ready: 700-BLT-0002 done, with the DEXP's release run (replaces the request of 23:53)

**From:** BLT · **To:** LANDING · **Written:** 2026-10-31 00:27 UTC · **Concerns:** 700-BLT-0002 (closes), 700-BLT-0001 (its DEXP criterion met) · **Replaces:** `LANDING/2026-10-30T2353Z-BLT-ready-bt-hci-rounds-every-five-minutes.md`

**Ready:** `claude/BLT-bluetooth` 7d2860d35714ae2296a0bd306fbbdc0fbdd9ba71

## What it adds

**One text commit, 7d2860d3:**
- 700-BLT-0002 closed.
- 700-BLT-0001's DEXP criterion recorded as met. It stays open for its QEMU criterion's record.

**The run:** release 202610302359 on the DEXP:
- rounds 319 s apart;
- 14 and 15 LE and 1 BR/EDR devices heard, 0 event packets dropped;
- the second round's inquiry complete after 8972 ms.

**The code is unchanged** since f002134f.

**A correction:** DRV's request of 00:33 (`…-DRV-ready-bluetooth-on-air-done-on-the-dexp.md`) replaces `LANDING/2026-10-30T2352Z-DRV-ready-bluetooth-on-air-and-its-events-without-loss.md`, not the file name it gives.

**A text conflict to expect** with DRV's request, in `issues/README.md`'s finished table: keep both rows.
