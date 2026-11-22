# 177-KRN-0274 — An extended desktop on two screens: the compositor's part of 177's step 2, for a screen a driver offers

**Type:** `compositor`, IDL (`display.wit`), design with APP · **Owner:** `KRN` (the kernel session) · **Priority:** P2 · **Status:** open (plan, 2026-11-22) · **Blocked by:** APP's `wm` on two screens (DRV's request in `APP/2026-11-22T0829Z-DRV-request-wm-on-two-screens-an-extended-desktop-with-the-second-placed-relative-to-the-first.md` on `internal-dev-messages`) · **Main task:** [177](177-display-output.md) (step 2), for [213](213-amd-ryzen-pc.md)'s two monitors · **Constitution:** MC-3.11, MC-6.1, MC-12.3, MC-12.4

## Problem

The maintainer asked on 2026-11-22 for the desktop to extend over both monitors, the second placed relative to the first (`KRN/2026-11-22T0829Z-DRV-the-maintainer-saw-the-mirror-and-asks-for-stretch-fit-and-an-extended-desktop.md` on `internal-dev-messages`).

Today the second screen only mirrors the first:
- [213-KRN-0272](../issues-done/213-KRN-0272-a-second-screen-for-the-compositor.done) made the mirror;
- [213-KRN-0273](213-KRN-0273-the-second-screen-stretched.md) lets it keep the first screen's shape or stretch.

[177](177-display-output.md)'s step 2 plans this layout: `data/displays.txt` places each monitor, and one desktop spans them. Its step 1 (every screen the firmware lit, in `BootInfo`) is not done, but the Ryzen's second screen comes another way: from a driver, through `display.wit`. This task is step 2 for such a screen.

The compositor copies one screen per task, the one in front, of the boot screen's size (`COMPOSITOR_PULL`). Nothing can draw a second, separate picture. The pointer and the windows are `wm`'s (APP), which knows of one screen.

## Plan (to agree with APP before code)

1. **`display.wit` 1.5**, a new version:
   - `fit` gains `extend`;
   - `enum side { right, left, above, below }` and `mirror-place(side)` place the second screen relative to the first, aligned at the first's top or left edge. This is chosen as 0273's fit is, by a client with no badge;
   - `mirror` also says the side.
2. **The second picture comes from the task in front, as the first does.** That task (`wm`) lends the compositor a second screen surface: a memory capability of the second screen's size, row after row, through a new call, `extend-surface`.
   - The compositor copies it to the second screen while the fit is `extend`, between that task's frames, as it copies the first.
   - When another task comes in front, or the surface is not lent, the second screen goes back to the mirror.
   - So no program but the one in front draws on either screen.
3. **The marks** (capture, camera, remote) stay drawn by the compositor on the first screen, and also on the second while it is extended. So no program can hide them on either screen.
4. **`wm`'s part (APP)**, its [177-APP-0094](177-APP-0094-wm-over-several-monitors.md):
   - one desktop of both screens, laid out by `mirror`;
   - the pointer crossing the shared edge;
   - windows moved from one screen to the other, and a full-screen program on either;
   - the layout and 0273's fit kept in `data/displays.txt`, as 177's step 2 plans, and given to the compositor at start.
5. **The monitor's own mode** (DRV's 213-DRV-0095) changes the second screen's size. `wm` reads it again from `mirror` after each offer.

## Acceptance criteria

- [ ] `display.wit` 1.5 agreed with APP, versioned, the fuzzer with it.
- [ ] Host tests: the layout of both screens for each side; the surface's copy within the second screen's bounds; the mirror back when the task in front changes.
- [ ] QEMU: every gate group passes with no second screen.
- [ ] **On a machine with two monitors** (MC-12.1): the pointer and a window cross from the first screen to the second; the marks show on both.

## Related

[177](177-display-output.md), [177-APP-0094](177-APP-0094-wm-over-several-monitors.md), [213-KRN-0272](../issues-done/213-KRN-0272-a-second-screen-for-the-compositor.done), [213-KRN-0273](213-KRN-0273-the-second-screen-stretched.md), [213](213-amd-ryzen-pc.md).
