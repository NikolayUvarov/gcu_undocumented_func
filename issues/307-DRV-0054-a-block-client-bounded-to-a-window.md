# 307-DRV-0054 — A block client bounded to a window of sectors, in the drivers' common serve loop

**Type:** library, interface · **Owner:** `DRV`, made by `STO` at its offer · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [307](307-the-stores-own-partition.md) · **Constitution:** MC-2.3, MC-3.4, MC-12.4

Numbered from STO's request of 2026-10-27 in `requests-DRV.md` ("A block client bounded to a range, and what a flush reaches"), its first half. STO offered to make it (`ALL/2026-11-09T1610Z-STO-…` on `internal-dev-messages`), and DRV agreed. The second half is [307-DRV-0055](307-DRV-0055-what-a-flush-reaches-per-driver.md).

## Problem

The store's own partition (307) needs `blockstore` to reach one partition of a disk and nothing else, through any driver. Every block driver serves `idl/block.wit` through one loop, `mind::block::serve` (`libmind/src/block.rs`, `block_protocol.rs`): `nvme`, `ahci`, `usb_storage`, `virtio_blk`, `ata` and `ramdisk`. So the bound is one change there, not one per driver.

## Plan

STO's shape, agreed:
1. **The windows:** up to 7 per driver instance, each a first sector and a count.
2. **A client's badge names its window** in bits 1 to 3 (1 to 7). 0 is the whole device, as now. Bit 0 stays `BADGE_WRITE`.
3. **A window's client sees the window as the device:**
   - `sectors` answers the window's count;
   - `read` and `write` take window-relative LBAs, shifted by the first sector and clipped at the window's end.
4. **`block.wit` 1.2 (minor)** adds `window(index, first, count)`.
   - Only a client badged `BADGE_WINDOWS` may call it, which `init` mints for itself alone. Others get `rights`, and the log names the badge.
   - A range past the device, or a count of 0, is `invalid`.
5. **After a driver's restart** its windows are gone. A client whose window is not set gets `not-found` on every call, never the whole device, until `init` sets the window again.
6. **Tests:** `tests/block_host.rs`:
   - a window's reads and writes land inside it;
   - a request past it is clipped or refused;
   - an unset window gives nothing;
   - only the windows badge sets one.

   QEMU once `init` uses it (307-KRN-0177).

**What STO touches:**
- `libmind/src/block.rs` and `block_protocol.rs`;
- `idl/block.wit` and its generated bindings;
- the host tests and the docs.

Nothing in the drivers' own code. `init`'s part is 307-KRN-0177.

## Acceptance criteria

- The host tests of plan step 6 pass.
- Every block driver builds unchanged on the loop.
- `blockstore` works through a window in QEMU once 307-KRN-0177 sets one.

## Related

[307](307-the-stores-own-partition.md), [307-KRN-0177](307-KRN-0177-the-stores-own-partition-classified-and-granted.md), [307-DRV-0055](307-DRV-0055-what-a-flush-reaches-per-driver.md).
