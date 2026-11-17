# 211-KRN-0256 — The log volume on a GPT boot disk: a window for its partition named `MIND LOG`

**Type:** `init` · **Owner:** `KRN` · **Priority:** P1 · **Status:** open (2026-11-17) · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Constitution:** MC-10.3, MC-12.1

## Problem

MOB's ask of 2026-11-16 (ask 1), for the maintainer's word of 2026-11-17: Ubuntu 26.04 arm64 and MIND Core on one GPT disk, Ubuntu first, MIND Core started from it.
- **What works on GPT:** the boot volume as a GPT partition (`VOLUME_GPT`), `init`'s window for it, the store's partition by `GPT_STORE`.
- **What does not:** the log volume. `vfs_windows` gives a GPT disk one window, the boot partition's. `vfs_server` takes as `log:` a FAT volume labelled `MIND LOG` among the boot disk's windows (211-KRN-0019), so on GPT it finds none and no `bootNNNN.log` is written.

## Plan

1. **The window.** On the disk whose table holds the boot partition (its start and size as the bootloader gave them), `init` gives `vfs_server` a window for the GPT partition whose name is `MIND LOG` (ASCII, any case), beside the boot partition's.
   - One partition only: the first so named. A second is logged and gets no window.
   - Not on another disk, and no other partition of Ubuntu's or anyone's.
   - It overlaps no store partition, as every window (the existing check).
2. **`vfs_server` unchanged:** it still mounts it as `log:` only if its FAT volume label is `MIND LOG` too. So MOB's script names the partition and labels the volume.
3. **The test:** a QEMU boot from a GPT disk with the boot partition, a Linux partition and the `MIND LOG` partition, x86 and aarch64:
   - `log:` mounted and `bootNNNN.log` written;
   - the Linux partition gets no window and is not written;
   - a disk where the log partition has the name but not the volume label: a window, no `log:`.
4. **The docs:** `docs/profile/bootstrap.md`'s `vfs_server` row; `docs/write-disk.md` if it names the log partition's layout.

## Acceptance criteria

- [ ] QEMU, x86 and aarch64, a GPT boot disk as above.
- [ ] The docs.
- [ ] On a machine: MOB's disk, once its script writes it (MC-12.1).

## Related

[211-KRN-0019](../issues-done/211-KRN-0019-boot-logs-on-the-log-partition.done), [307-KRN-0229](../issues-done/307-KRN-0229-vfs-servers-clients-bounded-to-its-own-partitions.done), [000-KRN-0124](000-KRN-0124-switching-between-linux-and-mind-core.md), [000-KRN-0257](000-KRN-0257-boot-switch-on-gpt-and-aarch64.md).
