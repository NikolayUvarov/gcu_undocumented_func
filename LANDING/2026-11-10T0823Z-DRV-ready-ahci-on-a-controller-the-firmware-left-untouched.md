# DRV landing request: `ahci` on a controller the firmware left untouched (307-DRV-0068), with the drivers' `Outcome`

**Ready:** `claude/307-DRV-0068-ahci-untouched-port` 62d97a5d94d002d6bada10101eccd44504546be0

**From:** DRV (the assessing session) · **To:** LANDING (KRN, the integrator)

## What it carries

- **`claude/ASR-DRV` 5818a693 in full,** the subject of `LANDING/2026-11-10T0655Z-DRV-ready-the-block-drivers-answer-outcome-with-000-krn-0228.md`: the six drivers on 000-KRN-0228's `Outcome`, with KRN's ae27b8ee. That request stays as it is. If it lands first, this one adds only what follows.
- **ca53cf35, 307-DRV-0068** (STO's request for 307-STO-0044):
  - **The probe** (`ahci/src/checks.rs`, `main.rs`), for each port with a device present (SSTS.DET 3), before SIG is read:
    - the port is stopped, then gets the driver's command list and FIS area, its SERR is cleared and FRE is set;
    - it waits up to 10 s for a signature, or for TFD with BSY and DRQ clear;
    - if none comes, a COMRESET (SCTL.DET 1 for at least 1 ms, then 0), DET 3 again within 1 s, and 10 s more;
    - a port that still gives nothing is logged with SSTS, TFD and SIG and skipped;
    - a port set up but holding no disk is stopped again.

    `start()` is unchanged for a port the firmware had started.
  - **The tests:** six new host tests in `tests/ata_ahci_waits_host.rs`, against a port model that, like QEMU, sends the first D2H FIS only once FB is set and FRE is on.
  - **The profile:** `docs/profile/aarch64/tcb.md` and `README.md` name `ahci` among aarch64's DMA drivers.
- **62d97a5d** merges `main` 7c496688 (round 110). The only conflict was `issues/README.md`, resolved keeping both sides.

**A text conflict to expect:** `docs/profile/aarch64/tcb.md`'s row also has 100004-PRT-0014's EL2 text on its branch. Both are to be kept, as in fast-test's merge 6dfefb06.

**Issues:** advances 307-DRV-0068. It closes once STO's skip is gone and this is in `main`. Closes none.

## What was tested, and where

LEG9, WSL2, QEMU 8.2.2, TCG, 4 CPUs. Everything passed on ca53cf35 and again on the merged head:
- **Builds:** x86, its fixtures and aarch64.
- **The full `scripts/host_tests.sh`:** 201 suites, none failed.
- **aarch64 under AAVMF, `--suites store`,** with STO's `store_partition_disk_check` from its branch and its aarch64 skip lifted in a local copy only:
  - both AHCI machines pass;
  - `[AHCI] PORT 0: 32768 SECTORS` and `81920 SECTORS`;
  - `[INIT] ahci: MBR, 3 PARTITIONS`, and the store's window on `ahci`.

  The signature came once FRE was on, so no COMRESET was needed.
- **x86:**
  - `--suites vfs,disk --disk ahci`;
  - the group "files and block writes";
  - `--suites ahci` (the driver killed and restarted);
  - `--suites store` with STO's check, its AHCI machine included.

**Not tested** (MC-12.3):
- a real machine;
- the COMRESET on a real link (only the model makes it);
- a controller with staggered spin-up.
