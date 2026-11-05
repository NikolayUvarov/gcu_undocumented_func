# 000-KRN-0211 — The loader's mark of how far a boot got, for a boot that leaves no log

**Type:** bootloader (`bootloader/src/mark.rs`), and `scripts/boot_switch.py` · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress · **Blocked by:** — · **Roadmap:** the Ryzen's runs (213); updates and slots (`UPD`) · **Constitution:** MC-6.3, MC-12.1, MC-12.2, MC-12.3

## Problem

**Two boots on the Ryzen left no log.** Of about twenty-five `BootNext` boots on un-rizen, two kept a dark screen until the maintainer restarted the machine: 2026-11-03 18:56 and 2026-11-05 05:18 (`KRN/2026-11-05T0546Z-DRV-…`). `boot_switch.py logs` said `MIND Core did not boot`. The next try booted each time.

**Nothing says where they stopped.** The boot log is written by `vfs_server` once the stick's storage is up (`usb_host`, `usb_storage`, `vfs_server`). A boot that stops anywhere before that leaves no file:
- in the firmware;
- in the loader;
- in the kernel, `init`, or the USB drivers.

DRV has no screen or serial console there. The loader writes nothing on a confirmed slot; it writes the boot records only on a trial.

## Plan

1. **The loader's mark, `MIND/REACHED`,** on its own volume, only where `MIND/` exists:
   - one line of 48 bytes, `MIND-REACHED 1 <count> <stage>`, written whole in place and flushed;
   - `STARTED`: the loader at `EFI/BOOT/` counts one boot more;
   - `READ`: whichever loader loads the slot (a slot's own loader too, 351-UPD-0010) has read and checked the kernel and the services;
   - `HANDOVER`: boot services end next.

   A write that fails is named on the serial line (`BOOT: NO MARK <stage> (<reason>)`) and never stops the boot. The loader names each mark it wrote (`BOOT: MARK <count> <stage>`).
2. **`boot_switch.py`:**
   - `once` records the mark's count beside the newest log;
   - `logs`, for a boot with no new log, says which it was: the loader did not start (the count is unchanged), `STARTED`, `READ` or `HANDOVER`; or that the stick's loader has no mark.
3. **On un-rizen:** more `BootNext` cycles with a stick that carries the mark. The next hang is then split by stage, with nobody at the screen.

## Acceptance criteria

- [x] Host tests: the mark's encoding and refusals (`tests/boot_mark_host.rs`); `boot_switch.py`'s reading of each stage, an unchanged count, no mark, and marks of another shape (`tests/boot_switch_test.py`).
- [x] QEMU, x86 and aarch64 (`boot_slots_check.py`):
  - each boot's three marks in order, and `HANDOVER` with one count more on the disk;
  - a slot's own loader carrying the count the loader at `EFI/BOOT/` started;
  - a boot reset in its pause left at `READ`;
  - the file system consistent after.
- [x] Docs: `docs/update/slots.md` and `docs/write-disk.md`, in English and Russian.
- [ ] On un-rizen, with DRV: the stage of the next boot that leaves no log, or that many cycles without one, recorded in 000-KRN-0124.

## Progress

2026-11-05, on `claude/youthful-mendel-mf1soy` eb5879e0:
- **The loader** (`bootloader/src/mark.rs`, `mark` in `main.rs`) writes the three marks and names each on the serial line.
- **`boot_switch.py`:** `once` records the count; `logs` names the stage of a boot without a new log, and the mark of one with a log.
- **Host tests,** in this session's container and on x32-128 (`host tests`, 752 s): `tests/boot_mark_host.rs` (4); `tests/boot_switch_test.py` (11, two of them new).
- **QEMU, through Effector:**
  - acc-1, x86: `boot, display, shell, memory, clock`, `USB image`, `Secure Boot with our keys`, `NVMe boot disk`;
  - x32-128, x86: `the updater and an update through the link`, `a developer's release through the Effector agent`;
  - acc-2, aarch64: `boot and fault containment`, `the updater and the modes`, `a developer's release through the Effector agent`, `an update through the link`, `NVMe boot disk`.

  All passed. Both architectures printed the two new lines of `boot_slots_check.py`:
  - three marks per boot, and `HANDOVER` one count more on the disk, on a trial and a confirmed slot;
  - a slot's own loader keeping the count, and a reset in its pause left at `READ`.

  The file system was consistent after (`fsck.fat`).
- **Left:** the cycles on un-rizen with a stick written from `main` once it lands.

**Landed (2026-11-05).** Round 84 (`main` f6abfbf0): the gate was green on every group, on acc-1, acc-2, PCU and x32-128.

**Left:** the cycles on un-rizen with a stick written from `main` f6abfbf0 or later.

**Not claimed:** the cause of the two hangs (MC-12.2). The mark only narrows where a hang is; QEMU's runs say nothing of the Ryzen's firmware (MC-12.1).

## Related

[000-KRN-0124](000-KRN-0124-switching-between-linux-and-mind-core.md) (the switch, and the hangs), [213-KRN-0125](213-KRN-0125-the-amd-fch-watchdog.md) (the watchdog, armed with the kernel's tick), [351-UPD-0006](../issues-done/351-UPD-0006-slots-and-boot-records.done) (the boot records), [351-UPD-0010](../issues-done/351-UPD-0010-updating-the-bootloader.done) (a slot's own loader), `docs/update/slots.md`, `docs/write-disk.md` section 10.
