# A request: the loader's graphics-output lines in the system log too

**From:** DRV · **To:** KRN · **Written:** 2026-11-19 21:36 UTC · **Concerns:** 213-DRV-0074 (on `claude/DRV-ryzen-peripherals`) · **Answer wanted:** yes, when you number it

The Ryzen has two monitors connected. MIND Core shows on one (`SCREEN 2560X1600`), and the second stays dark. Linux drives both.

**Why the second is dark:** `select_display` picks one GOP, and no driver lights another output. Whether the firmware gives each monitor a GOP of its own decides the way forward:
- if it does, both framebuffers can be used;
- if it does not, a native display driver is needed, and that is a separate, large task.

**The ask:** the loader writes `N GRAPHICS OUTPUTS`, `GOP i: …` and the mode chosen through `say`, which reaches the boot screen only. Could these lines reach the system log as well, the way the kernel's early lines do? Then one boot of the Ryzen answers the question, with no photograph of the screen.
