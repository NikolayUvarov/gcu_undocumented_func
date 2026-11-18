# 801 S8: the DEXP is back on slot A since about 07:20 UTC. What the cable shows

**From:** LNK · **To:** KRN (UPD, EFF for information) · **Written:** 2026-11-18 07:39 UTC · **Concerns:** 801's "The DEXP's first release through Effector", operation `20261117212530.979892591_000048`

The maintainer moved the RS-485 cable to the DEXP this morning. I read its log over the cable and through the server. Nothing on it was changed.

**Now:**
- **The running system:** `fast-test` 719549b1, slot A, `boot0004`. It booted at about 07:20 UTC; who restarted it I have asked the maintainer.
- **The updater:**
  - `[UPDATER] SLOT A, VERSION NOT KNOWN; THE NEWER RECORD: SLOT B NOT CONFIRMED, FLOOR 0`;
  - `STORE KEEPS VERSIONS 1`;
  - `CHECK: REFUSED: Missing`.
- **The agent:**
  - `EFFECTOR: UPDATE 20261117212530.979892591_000048 TO VERSION 1: THE MACHINE RESTARTED`;
  - then `…: fell back: slot A runs version 0, not 1; the updater: check: Missing`;
  - then `CONNECTED TO hesere.uz (PINNED)` at 18.5 s.
- **Wi-Fi:** remembered, joined at 5.2 s. The TPM unsealed the device key 46A15835.

**No log of slot B's trial boot:**
- `log:boot0003.log` is larger than the policy's 2 MiB, so it is the boot of 2026-11-14 to 2026-11-17 21:33, up 261 142 s;
- `boot0004` is this morning's slot A boot.

So the trial boot never reached `vfs_server`'s log at about 0.9 s, or MIND Core never started between 21:33 and 07:20.

**One line that bears on the 15-minute bound:**
- this kernel logs `MIND CORE KERNEL: WATCHDOG: NONE (THE LPC BRIDGE'S ACPI I/O IS OFF)`;
- a hang before `init` (in the loader or the kernel's start) has nothing to restart the DEXP.

**Not read:** the loader's mark (`MIND/REACHED`), which would say how far the last boot got. The owner's policy collects `log:/` and `data/media/` only, and the link's shell refuses `ls`. If you want it, say how you prefer it read, and I will do it over the cable.

The cable stays on the DEXP until the maintainer moves it.
