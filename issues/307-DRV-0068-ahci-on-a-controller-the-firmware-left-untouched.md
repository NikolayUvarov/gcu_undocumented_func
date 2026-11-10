# 307-DRV-0068 — `ahci` on a controller the firmware left untouched: the port set up before its signature is read

**Type:** driver · **Owner:** `DRV` · **Priority:** P2 · **Status:** in progress (2026-11-10) · **Blocked by:** — · **Main task:** [307](307-the-stores-own-partition.md) (for 307-STO-0044 (STO's, on its branch)) · **Constitution:** MC-4.1, MC-6.1, MC-12.1

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

## Progress (2026-11-10)

**Plan steps 1 to 3 made,** in `ahci/src/checks.rs` (so that a host test drives them) and `ahci/src/main.rs`:
- **Each implemented port with a device present** (`SSTS.DET` 3), before its `SIG` is read:
  - stopped as `start()` stops a port: `ST`, then `FRE`, each awaited 500 ms. A port that does not stop is logged and skipped, its areas left as they were;
  - given the driver's command list and FIS area (`CLB`, `FB`, zeroed), `SERR` cleared, `FRE` set;
  - `SIG` awaited until it is not `0xFFFFFFFF`, or until `TFD` has BSY and DRQ clear.
- **No signature by the deadline:** a COMRESET with `ST` clear and `FRE` kept on (`SCTL.DET` 1, then 0, its other bits kept), `SSTS.DET` 3 awaited, `SERR` cleared, and one more wait.
  - A port that answers only after it says so: `[AHCI] PORT n: SIGNATURE … AFTER A COMRESET`.
  - One that still gives none: `[AHCI] PORT n: NO SIGNATURE, EVEN AFTER A COMRESET (SSTS …, TFD …, SIG …): SKIPPED`.
- **A port that was set up and is not a disk** (another signature, or none) is stopped again, so that it writes nothing more into the FIS area the ports share.
- **`start()` is unchanged.** A port the firmware had started (OVMF) is stopped, given the driver's areas, and its signature, already there, is read at once.
- **The deadlines,** in 000-DRV-0064's form (`checks::wait`):

  | Wait | Deadline |
  |---|---|
  | The signature, after `FRE` and again after the COMRESET | 10 s |
  | The COMRESET's `DET` 1 | 1 ms |
  | The link back after it | 1 s |

- **The profile:** `docs/profile/aarch64/tcb.md` and its README name `ahci` among aarch64's DMA drivers.

**Tested** on this machine (WSL2, QEMU 8.2.2, TCG, 4 CPUs):
- `tests/ata_ahci_waits_host.rs`, 6 more tests against a model of a port's registers that, as QEMU does, posts the device's first D2H FIS only with `FB` set and `FRE` on:
  - `CLB`, `FB` and `SERR` written and `FRE` set before `SIG` is first read;
  - a signature 3 s late awaited, with no reset;
  - none by 10 s: a COMRESET (`DET` 1 at the deadline for at least 1 ms, then 0, the other bits kept, `ST` never set, `FRE` kept on), `SERR` cleared after it, and the signature it brings;
  - a port that never answers: one reset, about 20 s, skipped with `SSTS` 113, `TFD` 7F and `SIG` FFFFFFFF, its FIS receive off; the disk on the next port found;
  - a port OVMF had started: stopped, `ST` before `FRE`, before its areas became the driver's; found at once, with no reset;
  - a port without a device: only `SSTS` read; an ATAPI signature: stopped again; a port that does not stop: skipped, its areas untouched.
- The whole `scripts/host_tests.sh`: 200 suites, none failed.
- **aarch64, `virt` under AAVMF:** the `store` suite with STO's `store_partition_disk_check` (307-STO-0044, from its branch) and its aarch64 skip lifted in a local copy only. Both of its machines pass:
  - the store on an NVMe partition beside an AHCI disk, and on an AHCI partition beside an NVMe disk;
  - `[AHCI] PORT 0: 32768 SECTORS` and `81920 SECTORS`, `[INIT] ahci: MBR, 3 PARTITIONS`, the store's window set on `ahci`;
  - the object and the name found after a reboot, every sector outside the store's partition as written.

  No COMRESET was needed: the signature came once `FRE` was on. The rest of the suite passed as well.
- **x86, `pc` under OVMF:**
  - `--suites vfs,disk --disk ahci`, the boot disk on AHCI: pass;
  - the group "x86: files and block writes" (`vfs`, `edit`, `disk`, `block` with its AHCI write, `store`, `storefaults`): pass;
  - the `ahci` suite, the driver killed and restarted after its device was quiesced: pass;
  - `store` with STO's check (the same local copy): both machines pass, the AHCI one included.

**Not tested** (MC-12.3):
- a real machine, neither one whose firmware leaves its AHCI controller untouched nor one that starts it;
- the COMRESET on a real link: only the model has it, since in QEMU the signature always comes once `FRE` is on;
- a controller with staggered spin-up (`CAP.SSS`): a port whose `SUD` the firmware left clear does not reach `DET` 3 and is still not asked.

**Left:** STO removes the aarch64 skip from `store_partition_disk_check` (307-STO-0044, on its branch). The criteria are met once that check runs its AHCI machine on aarch64 in the suite and this change is in `main`.

## Related

307-STO-0044 (STO's, on its branch), [000-DRV-0064](000-DRV-0064-ata-and-ahci-waits-with-a-deadline.md), [211-DRV-0002](211-DRV-0002-ahci-every-port.md).
