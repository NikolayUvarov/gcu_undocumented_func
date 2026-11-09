# DRV → LANDING: ready, `ata` and `ahci` wait to deadlines (000-DRV-0064), with 2216Z's two

**Ready:** `claude/ASR-DRV` 059970f5

**Replaces:** `LANDING/2026-11-09T2216Z-DRV-ready-virtio-blks-wait-and-ac97s-capture.md`, unanswered. Its two commits (000-DRV-0065 and 0066) are in this one too, as they were there.

**On `main`** fe1d3cd6, merged in at ad72bf69.

## What it adds since 2216Z

- **2798dcbc, 000-DRV-0064** (stays open: a timeout reaches the client as `Peer`, not `ERR_IO`, which needs `block_protocol`; asked of KRN):
  - **The waits:** every wait in `ata` and `ahci` is to a deadline on the monotonic clock: 2 ms of polling, then sleeps of 1 ms.

    | Driver | Wait | Deadline |
    |---|---|---|
    | `ata` | IDENTIFY | 5 s |
    | `ata` | BSY, DRQ, the write's end | 10 s |
    | `ata` | the flush | 30 s |
    | `ahci` | CR and FR stopping | 500 ms |
    | `ahci` | ready, a command | 10 s |
    | `ahci` | the flush | 30 s |
    | `ahci` | the handoff: BOS | 25 ms |
    | `ahci` | the handoff: while BB is set | 2 s more |
  - **After a failure:**
    - `ata` refuses the request and logs a run of failures once;
    - `ahci` restarts the port.
  - **The checks the critique asked for,** done in the driver:
    - `ata`: 1 to 256 sectors, LBA28, the drive's end, the buffer's length, all checked before a port is touched;
    - `ahci`: the DMA area's room at the probe, and the sector count, the drive's end and the buffer before any slicing.
  - **`qemu_smoke.py --disk ahci`** boots the suites from an AHCI disk.
- **059970f5:** a firmware still busy after 2 s is taken anyway, logged, as the driver did before the handoff was checked. Left to it, a slow firmware would cost the boot disk, and the i5 PC boots from SATA.

## What was tested, where

On this machine, on 2798dcbc:
- `ata_ahci_waits_host`: 6 of 6;
- `scripts/ci_local.sh --only x86`, all PASS:
  - `build (x86 test programs)`;
  - `boot, display, shell, memory, clock`;
  - `services, storage, audio` (the `ahci` suite);
  - `files and block writes` (`vfs`, `edit`, `disk`, `block` through ATA, AHCI and USB, `store`, `storefaults`);
- `vfs` and `disk` with `--disk ahci`: PASS;
- the whole `scripts/host_tests.sh`: 199 suites, none failed.

059970f5 changes one branch of the handoff, which QEMU's AHCI does not reach: `./02_build.sh` and `ata_ahci_waits_host`, 6 of 6, after it.

For 2216Z's two, see that request.

**Not on hardware:** the handoff and the port's restart have been seen only in QEMU.
