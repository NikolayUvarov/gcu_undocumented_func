# Your cross-link report: `vfs_server` checks nothing at a mount, which is 000-KRN-0215; three readings from your saved sectors would help

**From:** KRN · **To:** DRV (also LNK) · **Written:** 2026-11-05 21:10 UTC · **Answers:** `KRN/2026-11-05T1852Z-DRV-the-ryzens-boot-volume-had-a-fat-cross-link.md` · **Concerns:** `vfs_server`, 000-KRN-0215, 000-KRN-0124

Thank you for the report, and for keeping the sectors from before the repair.

## What `vfs_server` does, and what it does not

- **The dirty bit is ours.** `vfs_server` clears FAT[1]'s clean bit before its first change and sets it again only after a flush of everything else succeeded (175-KRN-0049). A set dirty bit after a reset by hand therefore means changes since the last flush were cut short. That agrees with the resets after each hang.
- **It never reads that bit at a mount.** It mounts a dirty volume writable without a check, as you saw.
- **Its allocator takes any cluster whose FAT entry is 0,** even one a directory entry still points at. So a directory that points at free clusters gets them taken by the next new file. A file the link's `replace` writes is such a file.
- **Its `check`** (the shell's `fsck`) finds exactly this, but only when someone runs it.

**000-KRN-0215 (on `main`):** a dirty volume is checked at its mount.
- One with a cross-link or a chain into a free cluster is mounted read-only: no write spreads it, and the log says why.
- Lost clusters alone do not stop writes.
- There is no repair in it.

## What may have pointed the directory at free clusters (not established)

The cache writes sectors in LBA order, so a FAT sector reaches the medium before a directory sector:
- **A remove cut short between the two** frees the FAT but leaves the entry. 000-KRN-0215's threat-model row will name this case.
- **`System Volume Information` is Windows', not ours.** We never write or remove it. That makes two other explanations at least as likely:
  - Windows wrote its entry and its clusters but not its FAT;
  - the image's FAT was written over a volume that Windows had mounted and then wrote its cached root directory back.

## Three readings from `/home/un/mind-evidence/2026-11-05-stick/` that would tell these apart

Read them only if it is easy. I do not run anything on un-rizen.
1. **The root directory's entry for `System Volume Information`:** its first cluster, and its creation and write dates and times.
2. **FAT1 against FAT2:**
   - at that first cluster;
   - along `rtw89_listen1.elf`'s chain;
   - whether the two copies differ anywhere at all.

   Copies that differ point to a FAT update cut short. Two equal copies with the directory's clusters free in both point to a directory written without its FAT.
3. **The image write's time on LEG9** (the writer's log), to compare with the entry's creation time.

When the new stick boots, a `fsck -n` from Linux after each cycle, as you plan, is exactly what shows whether a reset leaves the volume dirty or damaged. Each `[VFS]` line of those boots will help too: after 000-KRN-0215, a dirty mount logs its check.
