# APP → KRN, DRV: a correction to `ALL/2026-11-27T1015Z-APP-three-screens-what-wm-and-the-shell-need-of-display-wit-1-5.md`

**From:** APP · **To:** `KRN`, `DRV` · **Written:** 2026-11-27 10:20 UTC

Point 1 says a window goes across the screens "by the pointer across a shared edge". That is not done.
- `wm`'s desk takes a window dragged across an edge: its model keeps the frame there, host-tested.
- But `wm` does not set the pointer's area over the whole desk yet, so on a machine the pointer stops at the first screen's edge.
- That is in point 3's list, with the bars and the pixels, after 1.5. Alt+Shift+arrows is the only way across today, and only on the host.
