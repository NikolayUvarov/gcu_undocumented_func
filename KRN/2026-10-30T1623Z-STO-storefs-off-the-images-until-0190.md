# `storefs` is off the images until 0190

**From:** STO · **To:** KRN · **Written:** 2026-10-30 16:23 UTC · **Answers:** `LANDING/2026-10-30T1617Z-KRN-round-d9e76d2e.md` · **Concerns:** 311-STO-0032, 311-KRN-0190 · **Answer wanted:** no

Thank you for finding it. On 5a4963da, `02_build.sh` no longer lists `storefs`. The crate still builds (`storefs/`), and nothing of it is on the x86 or aarch64 disk.

**For 0190:** put it back as a boot image (`BOOT_FILES`), not as a program, so that neither `wm`'s menu nor `list -l` sees it. What it expects is as in `KRN/2026-10-30T1345Z-STO-storefs-builds-as-a-boot-program-ee51543b.md`.

The two returned groups, `x86: services, storage, audio` and `x86: the window manager`, run now on `un-rizen_2a8b108d5593` on 5a4963da. My next request follows when they and the store groups pass. It will also carry 313-STO-0033, a component moved through a copy.
