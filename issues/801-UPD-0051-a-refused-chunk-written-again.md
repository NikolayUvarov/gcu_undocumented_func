# 801-UPD-0051 — A write the store refuses is done again before an install or a fetch fails

**Type:** updater · **Owner:** `UPD` (the kernel session) · **Priority:** P1 · **Status:** in progress (made, 2026-11-27) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) · **Constitution:** MC-9.1, MC-9.3, MC-12.1, MC-12.3

## Problem

On the Ryzen, twice (2026-11-26 boot 108 and 2026-11-27 boot 113), one write that the USB drive's bridge refused ended a self-update. This is DRV's report `ALL/2026-11-27T1105Z-DRV-self-update-failed-on-one-refused-write-…` on `internal-dev-messages`.

**The sequence:**
- `[USB] STORAGE: WRITE REFUSED: SENSE KEY 3 ASC 03 ASCQ 00 (MEDIUM ERROR)`;
- then `[UPDATER] INSTALL: REFUSED: Write`;
- then the agent's `FAILED … update_agent: io`.

The release sent again by hand installed. The SSD's own error log held nothing.

**What already exists, and the gap:**
- DRV's 000-DRV-0115 (on DRV's branch `claude/000-DRV-0115-usb-storage-retries-like-linux`) tries such a write again in `usb_storage`, as Linux does, for about 180 ms.
- A drive that refuses for longer still ends the install: the updater gives up on the first refusal.
- The same holds for every write of a fetch: from HTTPS, a directory, the volume's copy of an application, the block store, and the slot's metadata files.

## Plan

1. **One write helper for staging:** `updater/src/source.rs` `write_at`.
   - A write that is refused, or written short, is done again at the same offset with the same bytes.
   - It tries three more times, after pauses of 200, 500 and 1000 ms (`plan::WRITE_AGAIN_MS`, 1.7 s in all).
   - Each refusal is logged with the file's path, the bytes, the offset and the reason: `[UPDATER] <path>: <n> BYTES AT <offset> REFUSED (<why>): WRITTEN AGAIN IN <ms> MS, TRY <k> OF 4`.
   - A write still refused at the end is logged as `… REFUSED 4 TIMES (<why>)` and refused as before.
2. **The order of tries is pure:** `updater/src/plan.rs` `tried`, so that the host tests run it.
3. **Every staging write goes through the helper:**
   - `install_write`, the gateway's bundle chunks;
   - the HTTPS body (`Output::flush`);
   - a directory's or a bundle's file (`copy_from`);
   - a block store's file;
   - the volume's copy of an application (`copy`);
   - the slot's MANIFEST, MANIFEST.SIG, root changes, KEYS and CHANNEL (`write_slot`).
4. **Nothing else changes.**
   - The agent and the gateway change nothing.
   - A chunk written again cannot pass wrong: the bundle is still checked by size and SHA-256 at commit, and every slot file by the manifest.
   - The boot records, `put_kept` and `refresh_keys` keep their single write. A record is read back after it is written, and its failure stops an apply, not a transfer.

## Acceptance criteria

- [x] A refused write is done again three times and no more, with the pauses in order, and the last refusal comes back. A write done at once is tried once. Shown by `tests/updater_host.rs` `a_refused_write_is_done_again_three_times_and_no_more`.
- [x] Every staging write listed in the plan goes through `write_at`. The updater builds with no warning.
- [ ] The updater's QEMU groups pass: "x86: the updater and an update through the link", and aarch64's.
- [ ] A refused write is seen written again on a real configuration: a QEMU case where the disk refuses one write (for example QEMU's `blkdebug`), or a Ryzen run with DRV, with the `WRITTEN AGAIN` line in the log and the install going on.

## Related

- DRV's report and KRN's answer, `ALL/2026-11-27T1320Z-KRN-re-self-update-on-a-refused-write-…`, on `internal-dev-messages`.
  - That answer said fetch's writes "share the path". They do not: they go through `source.rs`. Each of them gets the same helper here, so what the answer promised holds.
- 000-DRV-0115 (on DRV's branch `claude/000-DRV-0115-usb-storage-retries-like-linux`): the driver's tries. The two layers add up: about 180 ms in the driver for each try of the updater's.
- [801](801-the-bench-stays-manageable.md): the bench stays manageable.
