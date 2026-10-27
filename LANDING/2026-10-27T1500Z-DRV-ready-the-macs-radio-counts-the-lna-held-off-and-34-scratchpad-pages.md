# Ready: the Mac's radio counts, its external LNA held off, and `usb_host` with more than 32 scratchpad pages

**From:** DRV · **To:** LANDING · **Written:** 2026-10-27 15:00 UTC · **Concerns:** 550-DRV-0028, 550-DRV-0036, 550-DRV-0039, 211-DRV-0040 · **Replaces:** nothing pending (DRV's last request, of 2026-10-21, landed)

**Ready:** `claude/ASR-DRV` 76c8525a9a4fb5dbb6bf5bc3feb5f6a1cdd44c2b

## What it brings

- **`bcm_wifi`** (the BCM4331 alone):
  - 550-DRV-0036: every 10 s, the network's beacons heard against those due, and every CCK frame by its level;
  - 550-DRV-0039: the external LNA held off (RF control 0x0400 on each core), logged 2 s after the radio starts serving.
- **`usb_host`, 211-DRV-0040:** an xHCI controller that asks more than 32 scratchpad pages gets the rest from the 64-page pool, up to half of it.
  - The maintainer's DEXP desktop asks 34. Until now it had no USB at all under MIND Core: `usb_host` was quarantined after three restarts, so no keyboard, no mouse and no link.
  - A controller that asks 32 or fewer is served as before.
- **The issues' records:** 550-DRV-0028 and 0036 text.

No ABI change. No IDL change.

## `main` in it

fb821bbd, merged in 417a2f31.

## What was run on the branch

- **`./02_build.sh`:** passed.
- **QEMU x86** (2026-10-27): `usb`, `tablet` and `link` passed. QEMU's xHCI asks no scratchpad pages, so the new path is not exercised there.
- **On the MacBook Pro** (2026-10-23, as 550-DRV-0039 records): `bcm_wifi` as slot B. Four hours on it lost 1 data frame of 33,244.
- **On the DEXP:** not yet. The stick is to be written again from `fast-test` 2886bd72, which holds this.
