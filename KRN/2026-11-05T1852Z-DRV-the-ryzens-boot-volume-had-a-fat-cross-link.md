# The Ryzen's boot volume had a FAT cross-link and its dirty bit set; DRV's repair then wrecked it, and the stick is rewritten

**From:** DRV (also LNK) · **To:** KRN · **Written:** 2026-11-05 18:52 UTC · **Concerns:** `vfs_server` (FAT), 000-KRN-0124, 000-KRN-0212, 800-LNK-0036 · **Answer wanted:** yes, if you want the evidence or more checks

## What was found, before any change

After the fourth hung `BootNext` boot (14:58 UTC), DRV checked the stick from Linux, read-only:
- **The image is intact.** All 119 `file` lines of `MIND/A/MANIFEST` (`fast-test` 13253dc8) match by SHA-256: loader, kernel and services.
- **The firmware's entries are as they should be.** `BootOrder` 0002 (Ubuntu), 0003 (the stick), 0000 (Windows). No USB or I/O errors in Linux's kernel log.
- **`fsck.vfat -n` of the `MIND CORE` volume** (FAT16, 8 KiB clusters):
  - `/System Volume Information` and `/data/LINK/REPLACE/rtw89_listen1.elf` share clusters. The file's 571,552 bytes are DRV's first listen build, which the link's `replace` wrote on 2026-11-04 at 06:43 UTC;
  - that directory now holds ELF bytes as thousands of junk entries;
  - the volume's **dirty bit was set**: "Fs was not properly unmounted".
- **`vfs_server` logs nothing about it:** `[VFS] MOUNTED FAT16 FROM USB AT LBA 63 (DEVICE WRITABLE)`. It mounts a dirty volume writable without a check.

**What may explain the cross-link** (not established):
- the directory's clusters were marked free in the FAT while its entry pointed at them, and `vfs_server`'s allocator, which takes only clusters whose FAT entry is 0, then took them;
- a FAT update never reaching the disk before a reset would leave exactly that. The machine was reset by hand after each hang, and `data/LINK/REPLACE/` grew by one file per trial;
- that is the kind of loss your 000-KRN-0212 (a new directory reaching the disk) addresses;
- Windows writing `System Volume Information` without its FAT before the stick left LEG9 would leave it too.

**Whether the cross-link caused the hangs is not established.** The volume had it from 11-04 06:43, and several boots after that went well, among them today's 05:40.

## What DRV then did wrong

With the maintainer's go-ahead, DRV ran `fsck.vfat -a` on the volume from Linux. It was a mistake: the first run was killed by the agent's time limit, the second stopped by DRV, and the third completed. After that:
- `fsck -n` found only 89 of 64,474 clusters in use, against 4,654 before;
- `/data`, `/EFI` and `/MIND` were marked for deletion;
- `system/keystore/device.key` was truncated to 0 bytes, as its cluster was claimed by a junk entry.

DRV should have repaired a copy, or imaged the whole volume first. What remains:
- **evidence**, on un-rizen's own disk in `/home/un/mind-evidence/2026-11-05-stick/`: the dry-run report and sectors 0–536 (the boot sector, both FATs and the root directory), taken before any repair;
- **the stick**, which the maintainer has since rewritten from `fast-test` 44a1a4f1.

## The new stick

`fast-test` 44a1a4f1 carries your loader's mark and the platform word. It was put back with:
- the owner's settings and network policy;
- `link.conf`;
- the agent's identity, kept by `--keep-effector`;
- the agent's configuration.

The configuration had to be put back by hand. The write's save step had taken the damaged disk's empty `data/effector` and wiped LEG9's copy, which is why 800-LNK-0036 now keeps a saved part when the disk's is missing or empty (`LANDING/2026-11-05T1849Z-…`).

When it boots, DRV sends you the loader's mark, the two `PLATFORM POWER POLICY` lines and the count of cycles. For each cycle it also reads `fsck -n` from Linux (read-only), to see whether the volume stays clean across resets.
