# 351-KRN-0022 — The updater's authorities: request flags and init's grants

**Type:** kernel (ABI) · **Owner:** `KRN` · **Priority:** P1 (the update track waits for it) · **Status:** in progress (all but the update zone) · **Blocked by:** [351-UPD-0008](351-UPD-0008-update-zone-in-vfs.md) (the update zone) · **Main task:** [351](351-self-update.md) · **Constitution:** MC-3.11, MC-9.3

## Problem

[351-KRN-0014](../issues-done/351-KRN-0014-trial-boot-and-confirmation.done) gave the system its slot, the trial and init's confirmation. It left out the grants the updater needs, because no updater exists yet to ask for them:

- TLS, for the release download;
- the update zone of `vfs_server` ([351-UPD-0008](351-UPD-0008-update-zone-in-vfs.md));
- a network grant;
- restarting the machine through `init`;
- the firmware variable privilege (`SLOT_FIRMWARE`, [351-KRN-0027](../issues-done/351-KRN-0027-uefi-variables.done)), for `BootNext` and `BootOrder` ([351-UPD-0010](351-UPD-0010-updating-the-bootloader.md)).

## Plan

- **Request flags** in `common/abi.rs` and `libmind::process` for each authority. They are additive, so the ABI version does not change: an older kernel refuses an unknown flag.
- **`init`** grants them to the `updater` service only, from its own capabilities.

## Acceptance criteria

The updater gets exactly these five authorities and no other, and a program that asks for any of them is refused (an `isolation` case).

## Progress

**2026-10-09: four of the five authorities, from init, tested with a stand-in.**

The updater is a boot service, not a program a launcher starts, so init grants its authorities when it starts it and no request flag is needed. A program has no way to ask for them.

- **Boot image.** `updater` is boot image 28 of 30 (`common/abi.rs`, after `sysmon`, before `shell`). The bootloader now leaves out a boot service missing from the volume if the signed manifest does not list it, so images without `updater.elf` boot as before. A service the manifest lists must be there.
- **What init grants** (`init/src/main.rs`):
  - its server endpoint;
  - an `rtc` client;
  - a read-only `vfs_server` client;
  - the log, as every service has;
  - `tls` (slot 20), when it runs;
  - the flow grant `netpolicy` makes for the program `updater` (slot 18), only if `netpolicy.txt` names it;
  - the firmware variable privilege (slot 27);
  - a client of `init` badged `BADGE_REBOOT` (slot 11).
- **Restart.** `init.wit` 1.3 adds `reboot`. `init` serves it only to the `BADGE_REBOOT` client, and refuses every other with `denied`. It then:
  - flushes the boot volume, `ram:` and `log:`;
  - stops the services in reverse start order, quiescing their devices;
  - resets the machine through its restart privilege, which `REBOOT` now accepts besides process control.
- **No copy can be badged for restart.** A badge is set once, but an unbadged client can still be badged by whoever holds it. The shell's client of `init`, which it lends for `REQUEST_LIFECYCLE`, is therefore badged `BADGE_LIFECYCLE` now.
- **Tests.**
  - `updater` suite (x86 and aarch64), with `tests/updater_stub` standing in for the service. The stand-in reports exactly slots 1, 2, 3, 11, 12, 18, 20 and 27; firmware variables read; the file system read and a write denied. When the shell makes `data/reboot`, the stand-in asks init to restart, and QEMU exits on the reset with the volume clean (`fsck.fat`). vfs_server writes directories through today, so the test shows the volume whole after the restart, not the flush itself.
  - `isolation` case `k`: a program with the lifecycle client cannot badge it for restart (`ERR_INVALID`), and `init` answers its `reboot` with `denied`.

Left: the update zone of `vfs_server` ([351-UPD-0008](351-UPD-0008-update-zone-in-vfs.md)), granted in place of the read-only client once it exists.

## Related

[351-UPD-0007](351-UPD-0007-updater-service.md), [351-UPD-0008](351-UPD-0008-update-zone-in-vfs.md).
