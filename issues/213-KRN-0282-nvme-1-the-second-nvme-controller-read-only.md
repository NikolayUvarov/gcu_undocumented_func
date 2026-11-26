# 213-KRN-0282 — `nvme#1`: the second NVMe controller, read-only

**Type:** `init`, `common/abi.rs`'s service list (no system-call change) · **Owner:** `KRN` (the kernel session) · **Priority:** P1 · **Status:** in progress (made, 2026-11-26) · **Blocked by:** DRV's `nvme` by instance (213-DRV-0060, `claude/213-DRV-0060-nvme-by-instance`), which must land in the same round or before: `main`'s `nvme` ignores its argument and writes · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-3.1, MC-4.2, MC-12.1, MC-12.3

## Problem

DRV's ask of 2026-11-26 (`KRN/2026-11-26T0013Z-DRV-213-drv-0060-…`), for the Ryzen's second NVMe controller (04:00.0, 1F40:1202). It holds an installed system that the machine's other users boot, and **it is never written** (213, the maintainer's word of 2026-10-25). DRV's `nvme` takes its instance from its argument. Any instance but the first is read-only: Identify, the queues and Read alone, and every refused write logged.

## Plan (made)

- **`common/abi.rs`:** `"nvme#1"` appended to `SERVICE_INSTANCES`, with its words. The kernel does not read the list; `init`, `svc` and the shell do.
- **`init`'s `nvme` arm:**
  - the instance's controller in PCI order (`find_device(0x01_08_02, …, instance)`);
  - its own BAR0, endpoint and 128 KiB DMA region, and no interrupt;
  - the argument `1` for `nvme#1`. The first keeps no argument.
  - A machine without a second controller gets `NotFound`, as `virtio_blk#1` does.
- **The decision DRV left to KRN:** `nvme#1` is in neither `STORE_DRIVERS` nor `vfs_server`'s driver list.
  - Why: `vfs_server` gets a window of every FAT partition on the disks it is given, and of the GPT partition at the boot partition's first sector. On the Ryzen's second disk that would be the other system's ESP.
  - The driver refuses writes anyway. This keeps the disk out of reach in two places, not one.
  - A read-only use, a later task, gives a read-only client.
- **`tests/qemu_smoke.py`'s `SERVICES`** names `nvme#1`, so that `ps` and `logs` find it.
- **The profile:** `bootstrap.md`'s `nvme` row.

## Acceptance criteria

- `init` and `svc` build. The host tests of the service list pass (`services`, `svc`, `boot_abi`).
- DRV's `nvme2` QEMU suite passes on the merged tree:
  - `nvme#1` read-only;
  - the image's SHA-256 the same before and after;
  - every write refused.
- On the Ryzen, DRV's run for that machine and build only (MC-12.1): `nvme#1` lists the disk and refuses writes, and the disk is never written.

## Related

[213](213-amd-ryzen-pc.md), [213-DRV-0060](213-DRV-0060-the-ryzens-second-nvme-and-its-analog-sound.md).
