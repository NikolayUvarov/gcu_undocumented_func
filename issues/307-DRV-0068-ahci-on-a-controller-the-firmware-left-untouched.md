# 307-DRV-0068 — `ahci` on a controller the firmware left untouched: the port set up before its signature is read

**Type:** driver · **Owner:** `DRV` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [307](307-the-stores-own-partition.md) (for 307-STO-0044 (STO's, on its branch)) · **Constitution:** MC-4.1, MC-6.1, MC-12.1

Numbered from the storage track's request of 2026-11-10 in `requests-DRV.md`.

## Problem

- **What happens:** on QEMU `virt` (aarch64, AAVMF), an AHCI controller (`-device ahci`) with a disk on port 0 gives `[AHCI] NO SATA DISK`.
- **What STO's local build logged:**
  - `CAP 0xc0141f05`, `PI 0x3f`, `GHC 0x80000000`;
  - port 0: `SSTS 0x113` (a device, the link up), `SIG 0xffffffff`;
  - ports 1 to 5: `SSTS 0`.
- **Why** (AHCI 1.3.1, sections 3.3.9 and 10.1.2): `PxSIG` holds the device's signature only once the port has received its first D2H register FIS, and receiving FISes needs `PxCMD.FRE`. `Ahci::probe` reads `SIG` before `start()` sets `FRE`.
  - On x86, OVMF's own AHCI driver has started the port before, so the signature is there.
  - AAVMF leaves the controller as it is, and so may a real machine whose firmware boots from USB or NVMe.

## Plan

1. **Before the signature is read,** for each port with a device present (`SSTS.DET` 3):
   - the command list and the FIS area (`CLB`, `FB`) set;
   - `FRE` set;
   - `SIG` read once it is not `0xFFFFFFFF`, or `TFD` no longer busy, within a deadline in 000-DRV-0064's form.
2. **No signature by the deadline:** a COMRESET (`PxSCTL.DET` 1, then 0, as section 10.4.2 says), then one more wait. A port that still gives none is logged with `SSTS`, `TFD` and `SIG`, and skipped.
3. **Then the disk is found as now,** with `start()` unchanged for a port the firmware had started.
4. **Tests:**
   - a host test of the probe's order against a register model (`tests/ata_ahci_waits_host.rs`): `FRE` before `SIG`, the deadline, the reset, a port that never answers;
   - QEMU aarch64 under AAVMF with `-device ahci`: the disk found, and the `store` suite's AHCI machine no longer skipped on aarch64 (STO removes the skip);
   - x86: the AHCI boot-disk group and the `block` suite as before.

## Acceptance criteria

- On aarch64 `virt` under AAVMF, `ahci` finds the disk on port 0, and `store_partition_disk_check` runs its AHCI machine there.
- The x86 AHCI groups pass as before.

## Related

307-STO-0044 (STO's, on its branch), [000-DRV-0064](000-DRV-0064-ata-and-ahci-waits-with-a-deadline.md), [211-DRV-0002](211-DRV-0002-ahci-every-port.md).
