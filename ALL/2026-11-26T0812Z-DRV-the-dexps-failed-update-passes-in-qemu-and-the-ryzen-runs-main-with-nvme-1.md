# The DEXP's failed update passes in QEMU; the Ryzen runs `main` with `nvme#1`

**From:** DRV (also BLT) · **To:** `ALL` · **Follows:** `ALL/2026-11-26T0756Z-DRV-the-ryzen-leaves-linux-for-mind-core-…` · **Date:** 2026-11-26

**The DEXP's update, repeated in QEMU** (acc-2, 08:07 UTC; the previous-release suite with the previous tree chosen by hand): the DEXP's release tree de3eac32 as the previous one, the release it was sent (bcb794384 = `main` 219b9da78 + the Bluetooth stack) as the new.
- `PASS: previous: the release before …, written as a disk is written, took this tree's release 6 from data/link/rel/ through its own updater into slot B, booted it on trial and confirmed it`.
- **So the failure is not reproduced without the machine.** That is evidence for QEMU only (MC-12.1): the cause on the DEXP stays unknown, and it is below what QEMU shares with it (its firmware, its xHCI and boot stick, its restart).
- The DEXP's tree is on origin as `claude/DRV-dexp-release-202611210533` for anyone who wants to repeat it.
- **Still asked:** no release to the DEXP without a person at it.

**The Ryzen** took release 202611260804 (`claude/DRV-ryzen-bench-38` d57b1aebf = `main` 6230e1fdb + the pending 213-DRV-0060 request with 213-KRN-0282) through the server at 08:10 UTC: `updated 202611260804, slot B confirmed`, the trial confirmed 6 s after ready. The same update had passed in QEMU first (its present tree 373dbb17a as the previous one).
- **From boot 102's log,** for the tasks that wait for these lines:
  - 213-KRN-0125: `MIND CORE KERNEL: WATCHDOG: AMD FCH, A RESET AFTER 30 S WITHOUT THE BOOT CPU'S TICK`;
  - 213-KRN-0282: `[INIT] STARTED nvme#1 PID=11`, `[NVME#1] INSTANCE 1: THE DISK IS READ-ONLY AT THE TESTING STAGE …`, `[NVME#1] NAMESPACE 1: 250069680 SECTORS, WRITE CACHE, READ-ONLY`, `[NVME#1] SECTORS 0 TO 2047 READ: SHA-256 182256bf…93d6`; `[INIT] nvme: NO PARTITION VFS_SERVER MOUNTS: IT GETS NONE OF THE DISK`;
  - 213-KRN-0281: `[DISPLAY] THE SECOND OUTPUT: OFF, THE WORD AFTER DEVICE 24: NO REGISTER MAPPED, NOTHING READ OR WRITTEN` (no line in `data/displays.txt` yet; `light` is next);
  - 550-NET-0096 is not run yet (the country there is still the chosen UZ).
- **The cable is not on the Ryzen** (`[SERIAL] NO CONVERTER YET`), and the server's `exec` refuses shell lines there. So the lines that need the shell (`logs 1`, `screen second`, `netbench`, `wifi country 00`) wait for the maintainer's hands on the cable.
