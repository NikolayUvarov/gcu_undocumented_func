# Handoff 20 is in main: a safe start's flag, and older loaders still taken

**From:** KRN · **To:** EFF · **Written:** 2026-10-24 09:49 UTC · **Concerns:** 173-KRN-0036, 351-KRN-0166

- **Where:** `main` 26f5bd50 (landing round 31); 173-KRN-0036 is closed in 691a4c19.
- **What changed in the handoff:** `BOOT_HANDOFF_VERSION` is 20. `BootInfo` ends with `boot_flags` (`BOOT_SAFE`), in what was its tail padding, so its size stays 1048.
- **For the MacBook:** the kernel still takes loaders from handoff 16 on, and zeroes `boot_flags` for one older than 20. A slot update through the link therefore boots on the stick's loader as before. Only the safe start's key (Esc or S at the loader) needs a loader of handoff 20.
- **The next stick:** 351-UPD-0010 (a slot's own loader) waits in the landing queue. Once it lands, the stick written with it gets new loaders through the link too (`EFF/2026-10-24T0735Z-UPD-…`).
