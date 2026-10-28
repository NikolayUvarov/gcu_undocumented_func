# 800-LNK-0029 — An Intel Mac's startup disk set by a script on the disk itself

**Type:** tool · **Owner:** `LNK` · **Priority:** P2 · **Status:** in progress (2026-10-28) · **Blocked by:** — · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-10.2, MC-12.3

## Problem

An Intel Mac starts a MIND Core disk at every boot only once `bless` has recorded it in the firmware's settings ([docs/write-disk.md](../docs/write-disk.md), section 6).

- **Where `bless` works:** only in the Terminal of macOS Recovery. In macOS, System Integrity Protection refuses it (0xe00002e2).
- **What is typed there today:** the disk's boot partition is mounted by hand, then three more commands are typed in full, with the loader's path twice. This worked on the MacBook Pro on 2026-10-21.
- **When it is needed again:** a rewrite of the disk may lose the setting, so the commands are typed again.

The maintainer (2026-10-28): put this on the disk as a script, so that only `mkdir` and `mount` are typed and the script does the rest.

## Plan

1. **`scripts/mac/startup-disk.sh`,** in POSIX `sh`, which Recovery has:
   - it takes the volume it lies on and refuses one without `EFI/BOOT/BOOTX64.EFI`;
   - it finds the partition's device with `diskutil info`;
   - it runs `bless --mount <volume> --setBoot --file <volume>/EFI/BOOT/BOOTX64.EFI --shortform`, and names Recovery when `bless` is refused;
   - it checks that `bless --getBoot` now names that device, and fails if not.
2. **`build_and_write.sh`** copies it onto the boot volume as `mac/startup-disk.sh` after the write, with `disk_copy.sh`, which reads it back. It does so unless `--image` is given, as for the wallpapers.
3. **The test:** `tests/mac_startup_disk_test.py` runs it under `sh` and `bash` against stand-ins of `bless` and `diskutil`, in `scripts/host_tests.sh`.
4. **[docs/write-disk.md](../docs/write-disk.md)** (both languages): Recovery's commands become `mkdir`, `mount` and the script. The commands by hand stay for a disk written before.

## Acceptance criteria

- [x] The host test: the script blesses its own volume's loader, run by an absolute or a relative path. When `bless` is refused, `--getBoot` names another device, the loader is missing or `diskutil` does not know the volume, it fails, and in the last two cases without calling `bless` (`tests/mac_startup_disk_test.py`, 2026-10-28).
- [ ] On the MacBook Pro, from Recovery: after `mkdir`, `mount` and `sh /Volumes/MINDCORE/mac/startup-disk.sh`, the Mac starts the disk at the next power-on with no key pressed. Evidence for that Mac only (MC-12.1).

## Related

[800-LNK-0014](800-LNK-0014-build-and-write-in-the-repository.md) (`build_and_write.sh`), [351-UPD-0010](../issues-done/351-UPD-0010-updating-the-bootloader.done) (the loader `bless` names stays at `EFI/BOOT/`), [211-PRT-0004](211-PRT-0004-first-run-on-an-intel-pc.md).
