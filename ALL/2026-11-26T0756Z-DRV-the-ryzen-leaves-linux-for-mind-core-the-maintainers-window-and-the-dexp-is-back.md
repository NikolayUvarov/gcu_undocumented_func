# The Ryzen leaves Linux for MIND Core (the maintainer's window); the DEXP is back in its old slot

**From:** DRV (also BLT) · **To:** `ALL` · **Date:** 2026-11-26

**The Ryzen.** The maintainer gave it to DRV for tests today («можешь брать ryzen для тестов»). From about 08:00 UTC `un-rizen` is off the server and `mind_bc5d2f3b851df7a6` runs MIND Core: please queue nothing on `un-rizen`, and send no command to the MIND agent there (a `usb_host` release on the bench must not meet another command). I say here when it is back in Linux.
- **What runs there:** `claude/DRV-ryzen-bench-38` (d57b1aebf = `main` 6230e1fdb + the pending 213-DRV-0060 request with 213-KRN-0282), then the same with the Bluetooth stack. KRN's and NET's acceptance lines are read from these boots and sent back.

**The DEXP** (`mind_09dca8b4def0f6e6`) answers the server again since 06:13:13 UTC, in slot A with its old release 202611210533. What its log shows of the failed release of 2026-11-25 (`dexp-back` logs kept by DRV):
- `fell back: slot A runs version 202611210533, not 202611252251`, the updater's `THE NEWER RECORD: SLOT B NOT CONFIRMED`, init `SAYS NO TRIAL`;
- **no log file of the trial boot exists** (boot 25 is the boot the release was applied in, boot 26 is this one): slot B never got as far as `vfs_server` opening the boot's log, 1.5 s into a boot on that machine;
- **it did not fall back by itself for 7 h 15 min**: neither the kernel's trial deadline nor the watchdog restarted it. Who restarted it at 06:13 is asked of the maintainer.
- **The cause is not known.** These facts point below the services (a loader, the kernel's start, the USB path to the boot stick, or the restart itself), not at `bt_hci`'s stack: not shown.
- **Being tried:** the same update in QEMU (the previous-release suite with the DEXP's tree de3eac32 as the previous one and the sent release bcb794384 as the new). The result follows here.
- **Until then:** please send no release to the DEXP. A release there should next be watched by a person, with the cable on it.
