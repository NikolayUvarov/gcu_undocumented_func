# Landing request: 000-DRV-0115, `usb_storage` tries a refused READ or WRITE again, as Linux does

**From:** DRV · **To:** `LANDING` · **Date:** 2026-11-27

**Ready:** `claude/000-DRV-0115-usb-storage-retries-like-linux` 6538a81fc

**What it is.** The Ryzen's self-update failed twice on one refused write (`issues-human/README.md` §19). Each time the boot drive's JMicron JM20329 bridge answered sense key 3, ASC 03h (a write fault), and `usb_storage` gave up at once. The SSD behind the bridge shows no failing medium in SMART, its own error log is empty, and the whole drive reads.
- **The change:** a refused READ or WRITE is tried again as Linux v7.0's SCSI layer would (`scsi_check_sense`).
  - Tried again: a MEDIUM ERROR other than ASC 11h, 13h or 14h; an ABORTED COMMAND; NOT READY "becoming ready". Up to 5 more tries, as `SD_MAX_RETRIES`, with pauses of 0 to 100 ms, which are ours.
  - Never tried again: hardware errors, illegal requests, protected media. A unit attention keeps its own path.
  - Every try again is logged with the sectors and the sector the device names, at most 64 lines a boot: `[USB] STORAGE: WRITE OF n SECTORS AT lba: DONE|GIVEN UP AFTER k TRIES AGAIN; …`.
- **Code:** `usb_storage/src/main.rs`, `usb_storage/src/recovery.rs` (the rule), `tests/usb_storage_host.rs`. `main` f32ba90c93 merged in.

**Advances:** 000-DRV-0115. Its last criterion is a release installed by this driver on the Ryzen.

**Tested:**
- **Host cases, LEG9:** `usb_storage_host` 14 of 14. The write-fault case fails if medium errors are not tried again (checked by hand). All of `scripts/host_tests.sh`: 282 `test result: ok`, none failed. `bash ./02_build.sh`: SUCCESS.
- **QEMU, on the branch, LEG9:**
  - "x86: services, storage, audio" passed (1137 s).
  - "x86: keys, shell, tools" failed once at `diag/quick.msh` (332 s). Its log was overwritten by the next group, so the cause is not known. Alone, at once after, it passed (603 s).
- **QEMU, on the bench image that carries it** (bench-48 0256effd5, with 213-DRV-0113 and `main` f32ba90c93):
  - LEG9: both groups passed (604 s, 1139 s).
  - acc-1: both failed early once (287 s and 240 s, `Timeout waiting for 'MIND> ': svc stop tts`), then passed both at normal times (598 s, 1147 s).
  - x32-128: the update from the release before passed.
- **On the Ryzen:** release 202611271307 carries it. That install itself still ran on the old driver, at the first try. No `STORAGE` line on that boot.
- In `fast-test` (b336fb1f0).
