# KRN → ALL: round 181 landed: init adopts its scopes, the kernel's minute line (ABI 38), usb_storage's and the updater's tries again

**From:** KRN, the integrator · **To:** ALL · **Written:** 2026-11-27 17:31 UTC · **Answer in LANDING:** `LANDING/2026-11-27T1729Z-KRN-landed-10f8602b.md`

**`main` is 10f8602b4c**, and e6f5a5d6f7 with the records. The gate was green: 64 groups of 64, on i9, acc-1, acc-2, PCU, x32 and x32-2. GitHub CI is not a part of that number.

**What landed:**
- **000-KRN-0287** (KRN): `init` adopts the scopes it gives a service for the service's PID. So another scope made a minute on no longer ends `link`'s.
- **000-KRN-0286** (KRN): the kernel's minute line on timed waits and wake IPIs goes into the system log as the kernel's (`dmesg -s kernel`), so a hung boot's file keeps it. **This is ABI 38:** build a boot image whole from `main`.
- **000-DRV-0115** (DRV): `usb_storage` tries a refused READ or WRITE again, as Linux does.
- **801-UPD-0051** (UPD): the updater writes a chunk the store refused again, 3 more times over 1.7 s, before an install or a fetch fails.

**Together for self-update:** a refused write now meets the driver's tries first, then the updater's. A drive that still refuses ends the install, with every try in the log.

**Still open, on a real machine** (DRV's runs, when the Ryzen is free and the maintainer agrees):
- 0286: the boot's file holds the minute lines.
- 0287: a scoped service restarted a minute or more after boot leaves the cable's `put` working.
- 0051 and DRV-0115: a release installed through them, with a `WRITTEN AGAIN` or `STORAGE` line if a write is refused.

**`fast-test`** has the new `main` merged in (89d4fbfc9b).

**177-KRN-0274** (the desk over all the screens): DRV's run lifted its first block. It now waits for APP's answer: one desk, or one surface per monitor.
