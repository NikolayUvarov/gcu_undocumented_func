# 213-KRN-0290 — The mirror blends what it shrinks, so no glyph loses a stroke

**Type:** `compositor` · **Owner:** `KRN` (the kernel session) · **Priority:** P1 · **Status:** in progress (made, 2026-11-28) · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md), with [177-KRN-0274](177-KRN-0274-an-extended-desktop-on-two-screens.md) · **Constitution:** MC-12.1, MC-12.3

## Problem

DRV's report `KRN/2026-11-27T2017Z-DRV-the-maintainer-sees-glyphs-lose-strokes-on-the-mirror-the-nearest-pixel-shrink-drops-rows.md` on `internal-dev-messages`.

**What the maintainer saw.** On the Ryzen, release 202611271851, the characters on the large monitor looked crushed, with parts missing: «на большом мониторе символы как-будто смяты - у них нет некоторых частей».

**Why.**
- The first screen (DP-2, 2560x1600) is mirrored on HDMI-A-1 at 2304x1440, a scale of 0.9, and on HDMI-A-2 at 1228x768, 0.48.
- Each picture pixel took the nearest source pixel (`source_of`). At 0.9 that skips one source row and column in ten; at 0.48, about every other.
- A stroke one pixel thick on a skipped row is not drawn at all.

**What stays.** By 0274's plan the screens copy the desk 1:1, but every driver screen still mirrors the first while another task is in front. So the shrink is seen, and needs mending in itself.

## Plan

1. **`mind::display::taps_of`:**
   - It gives, for each picture column and row, its nearest source pixel and the two after it, each weighted by its share of the span the picture pixel covers.
   - It is packed `first << 16 | w1 << 8 | w2`, so the compositor's tables keep their size.
   - Three pixels are all a span holds whole down to half the size. At the Ryzen's 0.48, a pixel past them is the larger part of the next span's.
2. **`blend` and `sampled`:** up to nine source pixels mixed, each channel apart, with integer weights of 256.
3. **The compositor's second and third screens** use them:
   - in the full draw (`redraw`);
   - in the changed spans (`update`): a picture row now takes the changes of every source row it blends, and the picture columns whose taps reach them;
   - under a mark taken away (`mark`).
4. **A picture of the first screen's own size has no weight past `first`.** It stays a copy pixel for pixel, as 0274's desk will want.
5. **Enlarging** (`stretch` across) takes the same taps: a source pixel's edge is shared between two picture pixels, as its coverage says.

## The cost

Measured on the host (this container's CPU, 5 runs each), a full draw of the picture:

| Picture | Nearest | Blended |
|---|---|---|
| 2304x1440 of 2560x1600 | 5.6 ms | 33 ms (5.9 times) |
| 1228x768 | 2.1 ms | 14.8 ms (7.2 times) |

- On the Ryzen today a full draw of the second screen is 8.6 ms (`DRAWN IN 8568 US`, boot 115). Expect several times that. The offer's line will say it.
- The changed spans of a frame cost the same per pixel.
- If that is too slow on the machine, the next step is a cache of the source rows filtered across, so that each source row is filtered once a frame and not up to three times.

## Acceptance criteria

- [x] Host tests, `tests/display_taps_host.rs`:
  - a picture of its source's own size is a copy;
  - the first tap is the nearest, and no tap is past the source;
  - at 0.9 and 0.48 no source row or column is left out (each is taken with at least a fifth of a pixel's weight), where the nearest pixel left some out;
  - strokes one pixel thick, one row in three, all show at both scales;
  - each channel is blended apart.
- [x] The compositor builds with no warning.
- [ ] The display groups pass ("x86: boot, display, shell, memory, clock" and the window manager's). QEMU has no display driver that offers a second screen, so they show that the first screen is unchanged.
- [ ] **On the Ryzen** (DRV's run, the maintainer's eye; MC-12.1): text on both HDMI screens has whole strokes, and the offers' `DRAWN IN` times are known.

## Related

- [213-KRN-0285](213-KRN-0285-three-screens-a-client-of-the-compositor-per-driver-screen.md) (the second and third screens);
- [213-KRN-0273](213-KRN-0273-the-second-screen-stretched.md) (the fit);
- [177-KRN-0274](177-KRN-0274-an-extended-desktop-on-two-screens.md) (the desk, 1:1).
