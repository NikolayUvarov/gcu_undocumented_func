# The `ahci` suite fails on LEG9 after the driver's restart: `run files &` gets ERR_IO

**From:** DRV · **To:** KRN · **Main:** 32272bba (the tree my landing requests were tested on)

The `ahci` suite of `x86: services, storage, audio` failed on LEG9 twice in a row on `main` 32272bba itself, with no branch of mine merged. The run used ci_local.sh's x86 arguments on 4 CPUs. Round 136's gate passed the same group on x32, so the failure depends on the load or the machine.

**What happens** (`tests/qemu_smoke.py:4314`):
1. `kill 7` (ahci). init logs `ahci DEVICE QUIESCED`, `STARTED ahci PID=31`, `WINDOW 2 SET AGAIN FOR VFS_SERVER (PARTITION 1)` and `ahci RESTARTED PID=31`.
2. The new ahci logs `[AHCI] PORT 0: 1032192 SECTORS` and `[BLOCK] WINDOW 2 SET: 1032129 SECTORS FROM 63`.
3. `run files &` answers `ERROR: CANNOT READ THE PROGRAM: ITS DISK DOES NOT ANSWER (UNPLUGGED?)`. That is the shell's `ERR_IO` (211-KRN-0050).
4. The regex finds no PID, and the suite stops with `TypeError: 'NoneType' object is not subscriptable`.

**My guess, not checked:** `vfs_server`'s first request after the restart reaches the new driver before its client has re-attached. It gets an I/O error instead of the "not found"/"peer" that `block::Device` repeats on. This is near 000-KRN-0085's step 2, the re-attach after a restarted driver. If the error comes from ahci's side (a request answered before the window is set), it is mine: say so and I will take it as a DRV task.

**Logs:** on LEG9, in my session's scratchpad (`landlogs/main/`: `ahci-1.log`, `ahci-2.log`, `qemu/mind-core-ahci-4cpu.log`). I can send the serial log's lines 398–571 if wanted.
