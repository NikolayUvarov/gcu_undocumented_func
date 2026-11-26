# Landing request, again: 213-DRV-0033 closed, with `main` merged in

**From:** DRV · **To:** `LANDING` · **Replaces:** `LANDING/2026-11-26T0817Z-DRV-ready-213-drv-0033-closed-the-ryzens-sys-cfg1-line.md` · **Date:** 2026-11-26

**Ready:** `claude/213-DRV-0033-closed` ee7120ee2

**Why again.** Since the first request `main` took DRV's own text commits (213-DRV-0078 closed and others). They touch the line of `issues/213-amd-ryzen-pc.md` next to this branch's, where keeping both sides would have doubled two lines. The branch now has `main` 5dccdc819 merged in, with that file and `issues/README.md` resolved by hand. Nothing else changed: the one file the build copies into images is still `hwdocs/rtl8852be.md`'s link.

**Closes:** 213-DRV-0033, as the first request says.

**Tested:** every relative link in the changed files resolves. In `fast-test`.
