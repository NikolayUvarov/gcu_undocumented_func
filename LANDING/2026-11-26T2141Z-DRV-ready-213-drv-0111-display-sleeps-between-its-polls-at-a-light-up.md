# Landing request: 213-DRV-0111, `display` sleeps between its polls at a light-up

**From:** DRV · **To:** `LANDING` · **Date:** 2026-11-26

**Ready:** `claude/213-DRV-0111-display-waits-asleep` b33a5fcd4

**What it is.** At each light-up of the second output `display` held its CPU awake for about 1.5 s: stage 9 read the frame count every 100 us for 30 frames, and stage 15's watch read it in a loop with no wait. On the Ryzen that CPU is also `usb_host`'s and `bt_hci`'s, and the maintainer saw the pointer move in jerks while the monitor was detected.
- **Stage 9** now waits awake only for its two frame edges (polled every 20 us, a frame at most each) and sleeps through the frames between. It counts from an edge to an edge, 30 frames or more, so a late wake only makes the count longer.
- **Stage 15's watch** sleeps a millisecond when no new frame has come. Its statuses are kept until cleared, so none is lost.
- **Code:** `display/src/light.rs` and `tests/display_light_host.rs` only. `main` 863bf9e7e is merged in.

**On the Ryzen** (release 202611262015, boot 108: the bench image abe47faa4, which is `main` 9d6feac696 with this change, 213-DRV-0112's read-only reads and a bench-only line of `init`; it holds for that machine, that monitor and that release, MC-12.1):
- the second output lit as before at boot and after two plug changes: `STEP 3'S TIMES (MS): IN ALL 2015.0` and `2017.0`, `WATCHED 60 FRAMES IN 1001 MS CLEAN, LONGEST GAP 3093 US`;
- the maintainer, moving the mouse while the plug went out and in: «рывков нет при перетыкании»;
- the kernel's minute lines for those two minutes: 18 and 8 waits behind another task (the longest 15 ms), against 28 to 67 (the longest 64 ms) in the minutes of plug changes with the old service on boot 107.

**Closes:** 213-DRV-0111 (moved to `issues-done/` on the branch, with the lines above).

**Tested:**
- LEG9, on b33a5fcd4: `bash ./02_build.sh` RESULT: SUCCESS; all of `scripts/host_tests.sh`: exit 0, 282 `test result: ok` lines, none failed. `display_light` 62 of 62, also with the Ryzen's own VBIOS image given (`MIND_AMD_VBIOS`, the vendor's file, not in the repository).
- The new host case fails when the polls are made awake again (checked by hand).
- QEMU, on the bench image abe47faa4 that carries this change: acc-1 "x86: keys, shell, tools" 600 s and "x86: services, storage, audio" 1144 s PASS; x32-128 the update from the release before PASS; LEG9 "x86: keys, shell, tools" 590 s PASS. QEMU has no Radeon, so these show only that nothing else moved.
- In `fast-test` (b0fb0537e).

**A text conflict to expect:** none in code. `issues/README.md` is merged with `main`'s rows on the branch.
