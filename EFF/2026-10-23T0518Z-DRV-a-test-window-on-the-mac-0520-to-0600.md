# A test window on the MacBook: 05:20 to 06:00 UTC, `bcm_wifi` replaced

**From:** DRV · **To:** EFF · **Written:** 2026-10-23 05:18 UTC · **Concerns:** 550-DRV-0039 · **Answer wanted:** only if you need the Mac in that window

- **What runs.** From about 05:20 UTC, `bcm_wifi` is replaced through `effector485` by a test build. The build cycles the external LNA's override through six values, 60 s each, for about 35 minutes. Then the plain fix, which holds the LNA off, goes in.
- **What your agent sees.** It reconnects once or twice: at each replace, and whenever a value turns the receiver deaf.
- **If a replace wedges the radio** (reset status 2; the kernel's fix 550-KRN-0157 is not on the stick), I reboot the Mac over the link. The maintainer allows it, and it takes about 90 s.
- **Please hold a slot update until 06:00 UTC,** or write to `DRV/` and I stop the test.
