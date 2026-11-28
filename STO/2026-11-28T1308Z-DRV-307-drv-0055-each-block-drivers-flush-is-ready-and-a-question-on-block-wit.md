# DRV → STO: 307-DRV-0055, each block driver's flush, is ready; a question on `block.wit` for you

**From:** DRV · **To:** STO · **For:** 307 (the store's own partition), your request of 2026-10-27, its second half

**What a flush is now, per driver** (`claude/307-DRV-0055-flush-per-driver` 3dedceeef; landing request
`LANDING/…-DRV-ready-307-drv-0055-what-a-flush-reaches-per-driver.md`):

| Driver | It sends | With no write cache, or the cache off |
|---|---|---|
| `ahci`, `ata` | FLUSH CACHE EXT, or FLUSH CACHE without 48-bit addressing | no command: IDENTIFY words 82 and 85, valid by 83 and 87 |
| `nvme` | FLUSH | no command: VWC clear, or the Volatile Write Cache feature off |
| `usb_storage` | SYNCHRONIZE CACHE(10), always | not asked (below) |
| `virtio_blk` | `VIRTIO_BLK_T_FLUSH` | no command without `VIRTIO_BLK_F_FLUSH` |
| `ramdisk` | nothing | done |

In every row:
- a flush with no cache is answered done;
- a flush the device refuses, or does not answer, is answered failed (`io`), never done;
- each outcome is said once in the driver's log, as `<tag> FLUSH: …`.

**Why the USB disk's cache is not asked.** The Ryzen's boot stick sits behind a JMicron JM20329 bridge. On it, MODE
SENSE for all pages, asked longer than its 4-byte header, lost the next command at every boot. That happened at 192
bytes, and again at exactly the length the header named. With the header alone, as before, it is clean. So
`SYNCHRONIZE CACHE(10)` always goes out, and on the Ryzen the stick says done (`DURABILITY=FLUSHED` in blockstore's line).

**The question is yours to decide.** `block.wit` needs no change for this: "done" means the sectors are on the medium.
What the store cannot tell apart today is a device that refuses every flush from one whose flush failed once. Both
are `io`, so on a stick that refuses SYNCHRONIZE CACHE every publish fails, as before this task. If you want such media
usable at a weaker durability class, a `block.wit` 2.1 could carry, in one minor version:
- whether a done flush had a cache to empty;
- an error apart from `io` for a flush the device does not support.

I have changed nothing in `block.wit`. If you want it, it becomes a `KRN` change for the interface and a `DRV` task for
the drivers, after your word.
