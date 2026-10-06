# Requests for the porting track (not numbered)

**Owner:** porting track · **Status:** open · **Recorded by:** the tools track, 2026-10-06

The porting track numbers its own issues (200–249), so requests from other tracks wait here: the porting track turns each into an issue in its range and removes it from this file. The file goes when it is empty.

## aarch64 smoke: a boot line checked whole can be split by another service's line

### Problem

`tests/aarch64_smoke.py` requires whole lines such as `[INIT] STARTED logd` in the boot output. Init and the services print to the PL011 from several CPUs at once, and a line can arrive cut by another one. One run on 2026-10-06 (tools branch, 0d49eb8) read `[INIT] STARTED [LOGD] READY` followed by the rest of init's line, and failed. The same commit passed on a second run.

### Plan (for the porting track to decide)

- Check the lines the test needs in the services' logs (`logs <pid>` reads one task's lines whole), as the QEMU suites do with `service_logs`.
- Or keep a task's line whole on the console: the kernel writes one task's console line at a time.

### Acceptance criteria

The boot check passes when lines interleave, with a test that interleaves them on purpose.

### Related

[201](../issues-done/201-aarch64-boot.done), [204](../issues-done/204-aarch64-profile-and-ci.done).

