# 177-KRN-0274 — The desk over all the screens: one desktop shown across every monitor, the compositor's part of 177's step 2

**Type:** `compositor`, IDL (`display.wit`), design with APP · **Owner:** `KRN` (the kernel session) · **Priority:** P1 · **Status:** open (plan revised for every screen, 2026-11-27) · **Blocked by:** APP's answer on one desk or one screen each (the first block, [213-DRV-0113](213-DRV-0113-display-both-hdmi-sockets-lit-at-once.md)'s run on the Ryzen with 0285's mirror, lifted on 2026-11-27: all three screens lit, `ALL/2026-11-27T1338Z-DRV-three-screens-ran-on-the-ryzen-…`) · **Main task:** [177](177-display-output.md) (step 2), for [213](213-amd-ryzen-pc.md)'s three monitors · **Constitution:** MC-3.11, MC-6.1, MC-12.3, MC-12.4

## Problem

The maintainer asked for this twice.
- **2026-11-22:** the desktop extended over both monitors, the second placed relative to the first (`KRN/2026-11-22T0829Z-DRV-the-maintainer-saw-the-mirror-and-asks-for-stretch-fit-and-an-extended-desktop.md` on `internal-dev-messages`).
- **2026-11-27, of the three screens:** «и по поводу "зеркало". Это только один и самый простой случай, а нужно разделение рабочего стола на все устройства..». That is, the mirror is only the first and simplest case; the desktop is to be divided over all the screens (`ALL/2026-11-27T1033Z-DRV-the-maintainers-word-the-desktop-spread-over-all-screens-not-only-a-mirror.md`).

**What there is today:**
- Since round 180 ([213-KRN-0285](213-KRN-0285-three-screens-a-client-of-the-compositor-per-driver-screen.md), `display.wit` 1.5), up to two screens offered by drivers, each mirroring the first.
- [213-KRN-0273](213-KRN-0273-the-second-screen-stretched.md)'s fit shapes the mirror.
- The compositor copies one screen per task, the one in front, of the boot screen's size (`COMPOSITOR_PULL`). Nothing can draw a picture larger than the first screen.
- The pointer and the windows are `wm`'s (APP). Its [177-APP-0094](177-APP-0094-wm-over-several-monitors.md) lays out several monitors and waits for this.

**The Ryzen's three screens:** DP-2 2560x1600 (over USB-C, the firmware's, the first), HDMI-A-1 3440x1440 and HDMI-A-2 1366x768.

**This plan replaces the one of 2026-11-22.** That one had `display.wit` 1.5 with `extend` and one side for a second screen. 1.5 went to the three screens (0285), so the desk is 1.6. A side for one screen does not place three.

## Plan (to agree with APP before code)

1. **One desk.** The screens side by side, each at its own size, at a place `wm` sets.
   - Without one, they stand in the order offered, left to right, tops aligned.
   - The layout is kept in `data/displays.txt`, as 177's step 2 plans.
2. **`display.wit` 1.6**, a new version:
   - `screens`: each screen's name, size and place on the desk. The first is the firmware's; the others are named by their driver.
   - `place`: a screen's place, set by a client with no badge, as `fit` is.
   - `name-screen`: a call a driver makes after its offer, naming the connector (`HDMI-A-1`, `HDMI-A-2`). `offer-screen` keeps its shape, so display need not land with the compositor.
   - `desk-surface`: the task in front lends the compositor a memory capability of the desk's size, row after row.
3. **The picture comes from the task in front, as now.**
   - While the task that lent the desk is in front, the compositor copies each screen's own part of it, 1:1, not scaled, between that task's frames.
   - When another task comes in front, or the surface is withdrawn, every driver screen mirrors the first again, as now.
   - So no program but the one in front draws on any screen.
   - **No ABI change.** Every task keeps its screen of the boot screen's size, and a program that knows nothing of the desk runs as it does. KRN's message of 2026-11-27 (`ALL/…T1320Z-KRN-re-the-desktop-over-all-screens-…`) planned a kernel screen as large as the desk for the task in front instead. The lent surface does the same without touching the kernel, and leaves every other program as it is.
4. **The marks** (capture, camera, remote) are drawn by the compositor on every screen. So no program can hide them on any.
5. **`wm`'s part (APP, 177-APP-0094):**
   - the desk as one surface (or one per screen, if APP answers so);
   - the bars and the pointer's area over the whole desk, the pointer crossing each edge;
   - windows moved between screens, and a full-screen program on any of them.
6. **A monitor's own mode** (213-DRV-0095) changes a screen's size. `wm` reads `screens` again after each offer and lends the desk again at its new size.

## Acceptance criteria

- [ ] `display.wit` 1.6 agreed with APP, versioned, the fuzzer with it.
- [ ] Host tests:
  - each screen's part of the desk, for 1, 2 and 3 screens of different sizes and places;
  - the copy within each screen's bounds and the surface's;
  - the mirror back when the task in front changes or the surface goes.
- [ ] **No screen's part of the desk is scaled** (DRV's ask, 2026-11-27): text drawn on the desk is copied 1:1 to each screen. A host test shows each screen's part copied pixel for pixel, and no scaled copy can pass for it. The mirror, while another task is in front, blends what it shrinks ([213-KRN-0290](213-KRN-0290-the-mirror-averages-what-it-shrinks.md)).
- [ ] QEMU: every gate group passes with no driver screen.
- [ ] **On the Ryzen with its three monitors** (MC-12.1): one desk across them; the pointer and a window cross each edge; the marks show on all three.

## Related

- [177](177-display-output.md), [177-APP-0094](177-APP-0094-wm-over-several-monitors.md), [213](213-amd-ryzen-pc.md).
- [213-KRN-0285](213-KRN-0285-three-screens-a-client-of-the-compositor-per-driver-screen.md) (the mirror on three screens), [213-KRN-0273](213-KRN-0273-the-second-screen-stretched.md) (the fit), [213-KRN-0272](../issues-done/213-KRN-0272-a-second-screen-for-the-compositor.done) (the first mirror).
- [213-DRV-0113](213-DRV-0113-display-both-hdmi-sockets-lit-at-once.md) (both HDMI sockets lit), [213-DRV-0095](../issues-done/213-DRV-0095-display-the-second-outputs-mode-from-its-edid.done) (a monitor's own mode).
