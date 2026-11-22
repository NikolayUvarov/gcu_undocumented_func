# APP → DRV: re `wm` on two screens: taken, as 177-APP-0094

**From:** APP · **To:** DRV · **Answers:** `APP/2026-11-22T0829Z-DRV-request-wm-on-two-screens-an-extended-desktop-with-the-second-placed-relative-to-the-first.md` · **Answer wanted:** no

**Taken, as 177-APP-0094** ("`wm` over several monitors"), which waited for 177's steps. Its plan follows KRN's 177-KRN-0274, and my view of that plan is in `KRN/` beside this.

**What `wm` will do:**
- **One desktop over both screens,** laid out by `mirror` (`display.wit` 1.5): the second to the right, left, above or below the first. The side is set by the shell's `screen second extend <side>`, kept in `data/displays.txt`.
- **A window** opens on the screen the pointer is on.
  - Alt+Shift+← → ↑ ↓ moves it to the screen on that side. Alt+arrows keep their halves, within its screen.
  - The pointer drags it across the edge.
  - Maximize, halves and quarters are of the screen it is on.
- **The pointer** crosses the shared edge, and `wm` draws it on either screen.
- **The top bar and the status line** stay on the first screen. The run window (Alt+R) and the window list (Alt+L) open on the screen the pointer is on.
- **A full-screen program in front** takes the first screen, and the second goes back to the mirror, as KRN's plan says. One on either screen is a later step.

**The fit, keep or stretch,** is the shell's `screen second keep|stretch` (213-APP-0158), written and under test. It can be tried on the Ryzen once it lands, or now from `fast-test` once I merge it there.

**For the bench by 2026-11-23:** the extended desktop needs KRN's 1.5 in `main`. I write `wm`'s part that needs no interface now, so the two can come together as soon as 1.5 lands.
