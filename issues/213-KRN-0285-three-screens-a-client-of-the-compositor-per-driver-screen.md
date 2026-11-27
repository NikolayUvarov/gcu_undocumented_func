# 213-KRN-0285 — Three screens: a client of the compositor for each driver screen

**Type:** `compositor`, `init`, IDL (`display.wit` 1.5; no ABI change) · **Owner:** `KRN` (the kernel session) · **Priority:** P1 · **Status:** in progress (made on `claude/213-KRN-0285-three-screens`, for round 181) · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-3.1, MC-3.11, MC-12.3, MC-12.4

## Problem

The maintainer wants all three of the Ryzen's outputs to work at once: DP-2 over USB-C, and both HDMI sockets. His words of 2026-11-27: «все три оба hdma и type-c». DRV's `display` is learning to drive two runs at once, each on its own pipe (213-DRV-0113).

Today the compositor holds one driver screen, the second, which `display` offers through its one client. DRV's messages ask KRN for three things (`KRN/2026-11-26T2143Z-DRV-…`, `KRN/2026-11-27T0613Z-DRV-…`):
- a third screen in `display.wit`;
- the compositor mirroring the first screen into each driver screen at its own size (today 3440x1440 and 1366x768);
- a second window of BAR 0 for the third screen's pixels.

DRV's message of 2026-11-26 (`KRN/2026-11-26T1847Z-DRV-…`) also saw the compositor keep another task waiting 64 ms on its CPU while a second screen came and went.

## Plan (made)

The answer to DRV (`DRV/2026-11-27T0732Z-KRN-three-screens-…`) chose **a client per screen**, rather than an index in the calls or a handle returned by the offer:
- No call changes its shape, so neither track waits for the other to land.
- Each screen's lease is a capability of its own, which init grants only with that screen's window (MC-3.11).

The parts:
- **`display.wit` 1.5.** A driver badged `BADGE_THIRD_SCREEN` (4) offers and lets go of a third screen, through its second client, with the same `offer-screen` and `screen-gone` as the second. Each badge names one screen. `mirror` still answers for the second screen alone: the shell's `screen` command calls it, and its code is APP's. Bindings are regenerated, and `docs/idl/README.md` says 1.5.0.
- **`mind::display`.** It holds:
  - `BADGE_THIRD_SCREEN`;
  - `DRIVER_SCREENS` (2);
  - `screen_of(badge)`, the driver screen a badge names, and `screen_name`;
  - display's slots for the third screen: `SLOT_THIRD_PIXELS` (7) and `SLOT_THIRD_SCREEN` (8).

  A host case (`tests/compositor_mirror_host.rs`) checks that each driver's badge names one screen and that no other badge names any.
- **The compositor** holds two driver screens, picked by the caller's badge. It mirrors the first screen into each:
  - each at its own size, with the fit the shell chose and the marks;
  - a fit chosen is applied to both;
  - a badge that names no screen is refused, as before.

  A full redraw writes each pixel once, the black only around the picture; before, the whole screen went black first.
- **The offer's line gives each part's time** (the 64 ms hold): how long the screen before took to let go, how long the new one took to map, and how long it took to draw. The line for a screen gone gives how long it took to let go. A plug change on the Ryzen thus shows which part holds the CPU. QEMU has no driver that offers a screen.
- **init** grants `display` a second window of the Radeon's BAR 0 for the third screen, [0x2400000, 0x3800000), 20 MiB, in slot 7. Only with it, in slot 8, comes the second client of the compositor badged `BADGE_THIRD_SCREEN`. The BAR is minted once for both windows. A machine without the range starts `display` without either slot, and init says so.
- **`docs/profile/bootstrap.md`'s `display` row** names the new slots.

## Acceptance criteria

- [x] The compositor and init build for x86 and aarch64. The compositor's and display's host cases pass (`compositor_mirror`, `compositor_second`, `display_screen`).
- [ ] The gate's groups pass with it (round 181).
- [ ] On the Ryzen, with 213-DRV-0113's two runs, both HDMI monitors and DP-2 show the desktop at once: the second and third screens mirror the first, each at its own size. This is DRV's run, with the maintainer.
- [ ] The offer's times on the Ryzen show which part held the CPU for 64 ms. Whatever takes more than one time slice is mended, here or in a task of its own.

## Related

- 213-KRN-0272 and 0273: the second screen and its fit.
- 213-KRN-0271: write-combining for a range in the boot framebuffer's BAR.
- 213-DRV-0113: `display`'s two runs.
- 213-DRV-0112: the second HDMI socket.
