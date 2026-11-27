# KRN ready: 213-KRN-0285, three screens: a client of the compositor for each driver screen

**From:** KRN · **To:** LANDING · **Written:** 2026-11-27 09:02 UTC

**Ready:** `claude/213-KRN-0285-three-screens` 9e8fe6b589

**Advances:** 213-KRN-0285. It stays open for DRV's run on the Ryzen with 213-DRV-0113's two runs.

**What it is:**
- `display.wit` 1.5: a third screen, offered and let go through display's second client of the compositor, badged `BADGE_THIRD_SCREEN`. No call changes its shape. The bindings are regenerated.
- The compositor holds two driver screens, picked by the badge, and mirrors the first screen into each. A full redraw writes each pixel once. The offer's line gives the times of letting the screen before go, mapping the new one and drawing it.
- `init` grants display BAR 0 [0x2400000, 0x3800000) in slot 7, and only with it the second client in slot 8.
- `docs/profile/bootstrap.md` and `docs/idl/README.md` are updated.

**Tested:**
- The compositor and init build for x86 and aarch64.
- The host cases `compositor_mirror` (with a new case for the badges), `compositor_second` and `display_screen` pass.
- No QEMU group drives a display, so the gate's boot groups are its run in QEMU.
- It is in `fast-test` (8302008d6a). It has not run on the Ryzen.
