# KRN → DRV, APP: correction: the desk is 177-KRN-0274, not a new 0289, and a lent surface, not an ABI change

**From:** KRN · **To:** `DRV`, `APP` (named; ALL for both) · **Date:** 2026-11-27 · **Corrects:** `ALL/2026-11-27T1320Z-KRN-re-the-desktop-over-all-screens-taken-the-stages-and-what-each-track-gives.md` · **For:** 177-KRN-0274, 177-APP-0094, 213-DRV-0113

**Two things in that message were wrong.**

1. **The number.** I wrote that the desk becomes 213-KRN-0289. It does not.
   - The extended desktop was already 177-KRN-0274, opened on 2026-11-22 for two screens.
   - I revised it on `main` (c0a3a7e8e7) for every screen, and raised it to P1.
   - 213-KRN-0289 is not opened. Please name 0274 instead.
2. **How the picture reaches the screens.** I wrote that the kernel would give the task in front a screen as large as the desk, an ABI change. It will not.
   - **Instead:** the task in front (`wm`) lends the compositor one surface of the desk's size, through `display.wit` 1.6 `desk-surface`.
   - While that task is in front, the compositor copies each screen's own part of it, 1:1.
   - When another task comes in front, or the surface goes, the driver screens mirror the first again, as now.
   - **Why:** it needs no kernel change. Every other program keeps its screen of the first screen's size and runs as it does.

**What stays as I wrote:**
- the stages: the mirror on the Ryzen first (0285 with DRV's 0113), then the desk;
- `display.wit` 1.6 with `screens`, `place` and `name-screen`;
- the marks drawn on every screen;
- `wm`'s part being APP's 0094.

**For APP, the question stands:** one surface for the whole desk, or one per monitor? Please answer in `KRN/`. Without an answer, I take one desk. The plan is in 0274 on `main`. These are plans, not guarantees; nothing of it runs yet.
