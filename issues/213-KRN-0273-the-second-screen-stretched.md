# 213-KRN-0273 — The second screen stretched or kept in shape: `display.wit` 1.4, chosen by the shell

**Type:** `compositor`, IDL (`display.wit` 1.4) · **Owner:** `KRN` (the kernel session), for the maintainer's ask through DRV · **Priority:** P1 · **Status:** in progress (made, 2026-11-22) · **Blocked by:** APP's shell command, for the choice at the console (`issues/requests-APP.md`) · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-3.11, MC-6.1, MC-12.1, MC-12.3, MC-12.4

## Problem

[213-KRN-0272](213-KRN-0272-a-second-screen-for-the-compositor.md)'s mirror keeps the first screen's shape. On the Ryzen that is 2560x1600 on 1920x1080: 1728x1080 from x 96, with black columns each side. The maintainer saw it on 2026-11-22 (`KRN/2026-11-22T0829Z-DRV-the-maintainer-saw-the-mirror-and-asks-for-stretch-fit-and-an-extended-desktop.md` on `internal-dev-messages`) and asked for three ways to use a second screen:
- stretched;
- 1 to 1, as now;
- the desktop extended over both monitors, the second placed relative to the first.

This task is the first two. The extended desktop needs `wm` and is [177-KRN-0274](177-KRN-0274-an-extended-desktop-on-two-screens.md).

## Plan

1. **`display.wit` 1.4**, served by the compositor:
   - `enum fit { keep, stretch }`:
     - `keep` is 1.3's way and the compositor's choice until told otherwise;
     - `stretch` fills the second screen, each axis scaled on its own.

     Either way the second screen shows the whole first screen, its marks among it.
   - `mirror-fit(fit)` chooses, for the second screen now and those offered after it, until the next choice or the compositor's restart. Only a client with no badge may choose: the shell's, and the copies it lends the programs it starts. A remote path's client (`BADGE_REMOTE_LINK`, `BADGE_REMOTE_EFFECTOR`) and the driver's (`BADGE_SCREEN`) get `denied`.
   - `mirror()` gives the second screen's size, the fit and the picture's size, or `no-screen`. Any client may ask.
2. **The compositor** keeps the fit. A choice that changes it places the picture again: `mind::display::placed`, with the column and row tables as large as the screen. It draws the whole second screen again and logs `[COMPOSITOR] MIRROR FIT STRETCH (PID n): THE FIRST AT 1920X1080 FROM 0,0 ON THE SECOND SCREEN'S 1920X1080`. An offer made under `stretch` says `, STRETCHED` at the end of its line.
3. **The shell's command** is APP's: `issues/requests-APP.md`. It should:
   - choose and show the fit;
   - keep the choice, so it holds after a restart: in `data/displays.txt`, the file [177](177-display-output.md)'s step 2 plans for the layout ([177-APP-0094](177-APP-0094-wm-over-several-monitors.md)).
4. **Not in this task:**
   - the extended desktop (0274);
   - the monitor's own mode, which is DRV's 213-DRV-0095. `offer-screen` again with the new shape is all it needs, and the fit holds across it.

## Acceptance criteria

- [x] `tests/compositor_mirror_host.rs`: a stretched picture fills the second screen on the Ryzen's shapes and others, each axis on its own; the kept one is 1.3's.
- [x] The IDL fuzzer with `display` 1.4 (`Fit`, `Mirror`).
- [ ] Builds for x86 and aarch64; every gate group passes on both architectures (no second screen in any).
- [ ] **On the Ryzen** (MC-12.1), after APP's command:
  - the shell's `stretch` logs `MIRROR FIT STRETCH … AT 1920X1080 FROM 0,0`;
  - the maintainer sees the first screen fill the second monitor, the marks on it;
  - `keep` brings back 1728x1080 from 96,0.
- [x] docs/idl and docs/api say what changed.

**Not covered:** how a stretched picture looks (the first screen's 16:10 on 16:9 is squeezed); that is the owner's choice.

## Related

[213-KRN-0272](213-KRN-0272-a-second-screen-for-the-compositor.md), [177-KRN-0274](177-KRN-0274-an-extended-desktop-on-two-screens.md), [213](213-amd-ryzen-pc.md).
