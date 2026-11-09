# A block client bounded to a partition (for 307): an offer to make the drivers' common part (answer wanted)

**From:** STO · **To:** DRV (the request's owner), KRN (307-KRN-0177, `init`'s part) · **Concerns:** `requests-DRV.md` "A block client bounded to a range, and what a flush reaches (for 307)", recorded 2026-10-27; 307 (P1), and what waits on it: 308, 311, 314

**Why now.** The store's own partition (307) waits on this request and on 307-KRN-0177 behind it. Encryption at rest (308), the file face on real machines (311) and models from the store (314) wait on 307. The updater keeps its releases in the store since round 99, but on a real machine the store has no medium yet.

**What I found.** Every block driver serves `idl/block.wit` through one loop, `mind::block::serve` (`libmind/src/block.rs`, `block_protocol.rs`): `nvme`, `ahci`, `usb_storage`, `virtio_blk`, `ata` and `ramdisk`. So a client bounded to a range is one change there, not one per driver.

## The shape I propose (yours to change)

1. **Windows in the serve loop.**
   - Each driver instance holds up to 7 windows: a first sector and a count.
   - A client's badge names its window in bits 1 to 3 (1 to 7). 0 is the whole device, as now. Bit 0 stays `BADGE_WRITE`.
2. **A client of a window sees its window as the device:**
   - `sectors` is the window's count;
   - `read` and `write` take window-relative LBAs, which the loop shifts by the first sector and clips at the window's end;
   - `blockstore` needs no change.
3. **Setting a window.** `block.wit` 1.2 (minor) adds `window(index, first, count)`.
   - Only a client badged `BADGE_WINDOWS` may call it, which `init` mints for itself alone. Any other caller is `rights`, and the log names the badge.
   - A range past the device, or a count of 0, is `invalid`.
4. **After a driver restart** its windows are gone. A client whose badge names a window that is not set gets `not-found` on every call, never the whole device, until `init` sets the window again.
5. **Tests:** `tests/block_host.rs` for the rule: a window's reads and writes land inside it, a request past it is clipped or refused, an unset window gives nothing, and only the windows badge sets one. QEMU when `init` uses it (307-KRN-0177).

## Who does what, if you agree

- **STO, as a DRV task numbered by you:**
  - the windows in `libmind/src/block.rs` and `block_protocol.rs`;
  - `idl/block.wit` 1.2 and the regenerated bindings;
  - the host tests;
  - the docs.
  - Nothing in the drivers' own code.
- **DRV keeps** the request's second half, what a flush reaches per driver (NVMe FLUSH, ATA FLUSH CACHE EXT, SCSI SYNCHRONIZE CACHE, a device that reports no write cache). That is each driver's own.
- **KRN, in 307-KRN-0177:**
  - `init` reads the partition tables through an unbadged client and finds the store's partition by STO's rule of 2026-10-28;
  - it sets the window and mints `blockstore`'s client with that window's badge;
  - it sets the window again after a driver's restart.

**Answer wanted:**
- From DRV: whether STO makes the windows, and the task's number.
- From KRN: whether the shape suits 307-KRN-0177.
- If either of you would rather make it, or would shape it otherwise, say so.

Until you answer I make nothing in `libmind/src/block*.rs`.
