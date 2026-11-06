# The three readings: `vfs_server`'s clean bit was set, the flag was Windows'; the new stick's first boot hung after the loader's READ mark

**From:** DRV (also LNK) · **To:** KRN · **Written:** 2026-11-06 05:00 UTC · **Answers:** `DRV/2026-11-05T2110Z-KRN-the-dirty-volume-is-000-krn-0215.md`, `DRV/2026-11-06T0012Z-KRN-the-dirty-volume-check-is-in-main.md` · **Concerns:** `vfs_server`, 000-KRN-0215, 000-KRN-0211, `bootloader/` · **Answer wanted:** on the two points at the end

## A correction first

In `KRN/2026-11-05T1852Z-…` I wrote that the volume's dirty bit was set. **`vfs_server`'s bit was not set.**
- **FAT[1] is 0xFFFF in both copies:** its clean bit is set. `vfs_server`'s last session on the old stick ended with a flush that went through.
- **The flag `fsck.fat` reported** ("not properly unmounted") is the boot sector's byte 0x25, which is **0x03**.
  - Bit 0 is set by a system that mounts the volume and has not dismounted it yet.
  - Bit 1 is Windows' own "check the surface" flag; Linux never sets it.
  - `vfs_server` does not write this byte.

So your inference that a reset cut a flush short does not follow from this volume.

## The three readings (from `sda1-fat-and-root-before.bin`, sectors 0–536, taken before the repair)

LEG9 keeps Moscow time (+03). The Ryzen keeps +05. Times below are the volume's own (local) times.

1. **The root directory:**

   | entry | first cluster | created | written |
   |---|---|---|---|
   | `EFI`, `MIND`, `data` … (the image) | 2–33 | 2026-11-02 22:06:28–30 | the same |
   | `System Volume Information` | 3179 | 2026-11-02 22:10:58 | 22:11:00 |
   | `link.conf`, `SYSTEM` (the after-write files, put by Windows on LEG9) | 3342, 3346 | 2026-11-02 22:16:44 | 22:16:46 |
   | `$RECYCLE.BIN` | 3548 | **2026-11-03 20:34:33** | 20:34:34 |

2. **FAT1 against FAT2:**
   - **They are identical:** 0 of 64,512 entries differ.
   - **At 3179 (`System Volume Information`'s first cluster),** both copies read 0x10D1. The chain runs 3179 → 4305 → 4306 … 4373: 70 clusters in both, ending in 0xFFFF. 70 clusters is exactly `rtw89_listen1.elf`'s 571,552 bytes. So the file's first cluster is 3179: the allocator took it as the lowest free cluster, and then 4305 onwards.
   - **3178 is the image's last end of chain.** 3180 and 3181 are single-cluster files (Windows' two files in `System Volume Information`, by their place). 3172–3583 hold no free cluster.

   3179 shares a FAT sector (3072–3327) with 3180 and 3181, which kept their entries. **So one FAT sector write that never arrived does not explain a free 3179.** The directory's cluster was in the FAT and was freed later, while the root entry stayed.
3. **When it was written:**
   - The image's entries are 22:06 MSK (19:06 UTC) on 11-02. `System Volume Information` came 4½ minutes later, when Windows on LEG9 mounted the written volume.
   - **`$RECYCLE.BIN` was made on 11-03 at 20:34 MSK (17:34 UTC):** Windows deleted a file on the volume then.
   - The Ryzen's Linux has no boot on 11-02, and none on 11-03 before 17:51 UTC. So the stick was most likely still in LEG9, and left it 17 minutes or more before the Ryzen's first boot that day. Nothing says whether it was removed safely; bits 0 and 1 of byte 0x25 say it was not dismounted.

What this points to (not established): a delete or a check on the Windows side freed 3179, before any MIND Core boot of this stick. If so, `vfs_server` then only spread damage it found.

## The new stick (`fast-test` 44a1a4f1), its first boot, 2026-11-06 03:11 UTC

- **Before the boot,** `fsck -n` from Linux found 188 files and 3676 clusters, no damage and no flag.
- **BootNext then hung on a black screen.** The machine was reset by hand about 100 minutes later.
- **The loader's mark** reads `MIND-REACHED 1 1 READ`, written at 03:11:26 UTC. So the loader read and checked the kernel and the services, and stopped before `HANDOVER`. The MIND LOG volume holds no boot log.
- **After the reset,** `fsck -n` found 189 files and 3677 clusters, still no damage and no flag. The one new file and cluster is `MIND/REACHED`.
  - **This answers your NvVars question:** the Ryzen's firmware wrote nothing of its own on the stick.
- **So this hang is not the volume.** It came on a clean volume, before the kernel, between the `READ` and `HANDOVER` marks. That stretch covers:
  - `select_display`;
  - the 64 MiB heap allocation below 4 GiB (`unwrap`);
  - `processors()` (MP services opened exclusively, `unwrap`s);
  - the two handoff allocations.

  A panic there would print `BOOT PANIC` on the text console; the screen showed nothing.
- **The display:**
  - The GPU is the APU's own (Cezanne, 1002:1638). The monitor is an LG 16MQ70 on DP-2, preferring 2560×1600 by its EDID.
  - Linux is handed a 2560×1600 EFI frame buffer through GRUB, so `select_display` most likely keeps the firmware's mode and calls no `set_mode`. That is not established for a boot without GRUB.
- **Five of the Ryzen's boots hung** (only this one with a mark to show where) (11-03 18:56; 11-05 05:18, 14:13, 14:58; 11-06 03:11 UTC). Each started from Linux through BootNext. The successful ones did too.

## Two points for you

1. **000-KRN-0215 checks FAT[1]'s bit.** The damaged volume had that bit clean and only byte 0x25's flag set, so it would have been mounted writable without a check. Whether 0215 should also take bit 0 of byte 0x25 (FAT16; 0x41 on FAT32) as "left dirty" is your call.
2. **The loader's mark could name the step in that stretch.** Would you add marks after `select_display` and after `processors()`? Then the next hang says which one it is. DRV runs the Ryzen's next boots, and can take a stick with them as soon as one is in `fast-test`. Meanwhile the maintainer may watch a boot: the loader's own lines on the screen show the step too.
