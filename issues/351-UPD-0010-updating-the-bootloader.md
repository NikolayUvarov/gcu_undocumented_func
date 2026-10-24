# 351-UPD-0010 — Updating the bootloader itself

**Type:** update (boot) · **Owner:** `UPD` (the kernel session since 2026-10-24), with `PRT` · **Priority:** P1 (the maintainer, 2026-10-24: the MacBook Pro's next stick is written once this is done) · **Status:** open · **Blocked by:** — · **Main task:** [351](351-self-update.md) · **Constitution:** MC-9.1, MC-9.3, MC-12.4

Numbered by the kernel session at the maintainer's request (2026-10-08), before the track had an owner. Taken up by the kernel session with the track on 2026-10-24, after EFF's request of 2026-10-23 (`UPD/2026-10-23T1856Z-EFF-updating-the-bootloader-351-upd-0010-and-the-macbook.md`).

## Problem

Slots A and B cover the kernel, the services and the applications, but there is one `BOOTX64.EFI` (`BOOTAA64.EFI` on aarch64), at `EFI/BOOT/`. A failed write to it leaves a machine that does not boot. Yet the bootloader changes too: signature checks, records, and the format of `BootInfo`.

- **The handoff.** Since 351-KRN-0166 a kernel takes older loaders as long as `BootInfo` keeps its layout, and handoff 20 (173-KRN-0036) added a field old loaders simply leave out. But the next change to the loader's own work, a key, a record or a check, still needs a rewritten stick.
- **The MacBook Pro.** It is updated only over the link (`scripts/link_update.py`). It boots the stick through Apple's firmware after `bless --setBoot --file …/EFI/BOOT/BOOTX64.EFI`, made from macOS Recovery ([write-disk.md](../docs/write-disk.md), section 6).

## Why not `BootNext`

The plan of 2026-10-08 was:
- two loaders, each with a UEFI boot entry;
- `BootNext` set to the new one for a trial boot, then `BootOrder` changed.

It does not fit the machine it is most needed on:
- **Apple's firmware starts what `bless` recorded,** Apple's own variables, not `BootOrder`. Whether it honours `BootNext` for an external disk is not known, and its one-time setting is Apple's own `bless --nextonly`.
- **`bless` works on that Mac only from Recovery:** SIP refuses it in macOS (0xe00002e2). So no running system can move the Mac's boot to another file.
- **Firmware variables are written from the running system.** On every firmware, a mistake there is harder to undo than a file on the stick.

## Plan

**The loader at `EFI/BOOT/` stays where the firmware starts it, and is never rewritten by an update. Each slot carries its own loader, and the loader at `EFI/BOOT/` starts it.**

- **A slot holds its loader:** `MIND/<slot>/BOOTX64.EFI` (`BOOTAA64.EFI`), the build's loader.
  - The slot's signed `MANIFEST` already lists it, as `EFI/BOOT/BOOTX64.EFI`, with its size and SHA-256. No manifest format change.
- **The loader at `EFI/BOOT/`** chooses the slot from the boot records as today. It counts a trial's try on the disk first, as today.
  - **Then the slot's loader.** If the slot holds a loader that differs from its own file, the root loader:
    - checks it against the slot's manifest: the signature, then the size and SHA-256 of `EFI/BOOT/BOOTX64.EFI`;
    - starts it from the bytes it checked (`LoadImage` from the buffer, with the file's device path, so it reads its own volume), and tells it the slot and whether it is on trial (its load options);
    - starts no other loader before that one has returned.
  - **The same loader, or none.** A slot whose loader is the same file as its own, or that holds none (one staged before this task), the root loader boots itself, as today.
- **A started slot loader** reads the slot and the trial from its options. It does not choose a slot, does not write a record, and does not start another loader. It loads its slot's kernel and services against the slot's manifest, as the root loader would.
  - If it cannot, it returns an error. Firmware Secure Boot refusing it is the same case.
  - The root loader then treats the slot as one that did not verify: on a trial it writes the record with no tries left, and boots the fallback slot.
- **A slot loader that hangs or resets** has already had its try counted: at the next boot the trial is over and the fallback slot boots, as for a kernel that never confirms.
- **The updater** (`updater/src/plan.rs`) stages the manifest's `EFI/BOOT/BOOT*.EFI` into the slot it writes, beside the kernel and services. So does `scripts/link_update.py` (`release_files`).
- **The writers:** `make_usb_image.py --slots`, `boot_slots.py layout` and the internal-disk writer put the build's loader in each slot too. A new stick then has the same loader at `EFI/BOOT/` and in its slots.
- **`EFI/BOOT/BOOTX64.EFI` changes only with a new stick.** That is the loader the firmware starts first, and the one `bless` names on the Mac. It needs the chaining from this task to start a newer slot's loader, so the MacBook Pro's stick is written once more, after this.
- **Secure Boot** (351-UPD-0012): `LoadImage` checks the slot's loader against `db` too, so the build signs it as it signs the root's.

## Acceptance criteria

In QEMU (x86 with OVMF, aarch64 with AAVMF), on a disk with both slots:
- **A newer loader boots on trial.** Slot B staged with a loader that differs from the root's, made with a test feature that says so on the serial line, boots on trial. The root's line, the slot loader's line and the kernel's start come in that order. Once confirmed, B boots the same way.
- **A loader that fails falls back.** Slot B's loader that returns an error at once leaves the machine on slot A at once and at the next boot. So does one that does not match its manifest, which is never started.
- **A loader that hangs falls back at the next boot.** The VM is reset while slot B's loader waits: its try was counted, and the next boot starts slot A.
- **Updates carry the loader.** `link_update.py` and the updater stage the loader into the slot. The `linkupdate` check's slot holds the build's loader.
- **Compatibility.** A slot without a loader, and one whose loader is the root's own, boots as before.

## Related

[351](351-self-update.md), [351-UPD-0006](../issues-done/351-UPD-0006-slots-and-boot-records.done), [351-UPD-0012](351-UPD-0012-secure-boot-with-our-own-keys.md), [351-KRN-0166](../issues-done/351-KRN-0166-the-loaders-handoff-versioned-apart-from-the-system-call-abi.done), [173-KRN-0036](173-KRN-0036-safe-start.md), `docs/update/slots.md`, `docs/write-disk.md`.
