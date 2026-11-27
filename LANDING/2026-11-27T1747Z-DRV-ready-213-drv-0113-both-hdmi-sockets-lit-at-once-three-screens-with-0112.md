# Landing request: 213-DRV-0113, both HDMI sockets lit at once (three screens), with 213-DRV-0112

**From:** DRV · **To:** `LANDING` · **Date:** 2026-11-27

**Ready:** `claude/213-DRV-0113-display-both-hdmi-sockets` df23af4de

**What it is.** `display` drives both of the Ryzen's HDMI sockets, beside the firmware's DP-2. The maintainer's requirement is three screens at once: «они все должны работать, все три: оба hdmi и type-c».
- **213-DRV-0112:** the second socket, HDMI-A-2 (VBIOS path 1, PHY B, DDC2, HPD 2). Its blocks are read, its EDID read over DDC2, then lit in 16 stages.
- **213-DRV-0113:** two runs at once, each on its own pipe and guarded against the other's blocks and the firmware's. The second socket is offered as the **third screen** through 213-KRN-0285's slots 7 and 8 (`display.wit` 1.5).
  - Each unplug takes back only its own run.
  - A plug lights its socket again beside the other.
- **The release's behaviour changes:** a release now lights every HDMI socket that has a monitor, in the production order. The bring-up switches keep the narrower levels; at their release values no "A BRING-UP BUILD" line is printed.
- **Code:** `display/src/*` and `tests/display_*_host.rs` (`hwdocs/cezanne-display.md` too). `main` 6f60d95a5 merged in. 0285's branch came in through an earlier merge and is in `main` since round 180. No file of another track's changed.

**Closes:** 213-DRV-0112 (moved to `issues-done/` on the branch).

**Advances:** 213-DRV-0113. Left open:
- the colour squares on the third pipe were not looked at apart;
- table 12's long path is not shown on hardware: every PHY was found set up, even after a start with both cables out;
- `main`.

**Tested:**
- **Host, LEG9:**
  - display_light 99, display 16, display_hpd 9, display_picture 6, display_screen 4, display_batches 19, display_atom 11, display_ddc 10, displays 1. All pass without and with the Ryzen's VBIOS image (`MIND_AMD_VBIOS`, the vendor's file, not in the repository).
  - compositor_mirror 5, compositor_second 3, wm 52.
  - All of `scripts/host_tests.sh` on 145b8316c: 282 ok, none failed. Later commits are text only, or merges of `main`.
  - `bash ./02_build.sh` SUCCESS.
- **QEMU,** on the bench image 7898901a1 (this branch at c2f00edea with 000-DRV-0115 and `main`; no bench-only commit):
  - acc-1: "x86: keys, shell, tools" 597 s and "x86: services, storage, audio" 1146 s;
  - x32-128: the update from the release before.
  - QEMU has no Radeon: these show only that nothing else moved.
- **On the Ryzen,** release 202611271414 of that image (boots 116, 117). It holds for that machine, its three monitors and that release (MC-12.1):
  - all three screens lit 8.6 s after `display` started;
  - each HDMI cable out and in, the other screen undisturbed;
  - the two monitors swapped between the sockets and swapped back, each lit each time;
  - both cables out across a restart, then plugged one at a time;
  - an hour of standing checks on three pipes, nothing new;
  - no underflow, no failed stage, the firmware's pipe unchanged throughout.
  - The maintainer's look at each step: «Светятся», «Не гас», «Не гас», «Поменял, показывает», «есть изображение».
- In `fast-test` (ff3d6e1d4).

**For the integrator:** 000-DRV-0115 (`LANDING/2026-11-27T1337Z-DRV-ready-000-drv-0115-…`) is a separate request, and these two do not depend on each other.
