# 211-DRV-0053 — A USB disk that missed one transfer comes back, and the boot disk is written less often

**Type:** driver · **Owner:** `DRV` · **Priority:** P1 · **Status:** in progress · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Constitution:** MC-6.1, MC-6.6, MC-12.1

## Problem

**What the maintainer saw** (2026-11-09): the MacBook Pro's boot disk was being accessed all the time. Release 202611011517 (tree 101b4f47), slot A, up 17 hours. The disk is `7825:A2A4`, 1000215216 sectors, on xHCI port 5 at SuperSpeed.

**What the link showed** (`effector485`, the cable moved to the Mac that night):
- **Every read and write of the disk failed:**
  - the Effector agent's audit journal (`collect_logs: audit (JOURNAL WRITE FAILED)`), so the server's operations timed out;
  - the loader (`[UPDATER] PARSER: THE SHARED SERVICE (Loader(Unreadable))` every 90 s), so no program could start;
  - `link`'s `list` and `stat` ("the other end's storage failed", code 3).
- **`usb_storage` was in its claim tries.** Its only sleeps are in `claim` (100 ms) and `capacity` (50 ms): it was `SLEEPING` in 9 of 66 samples, with bursts of 11 to 32 system calls.
- **When it began.** The updater already failed at 55845 s of uptime, about 02:30 UTC. The log ring (2048 records) no longer held the first failure, and the saved boot log is larger than the 2 MiB Effector's policy lets out.
- **The way out.** A reboot over the link brought the disk back at once (`MASS STORAGE: 1000215216 SECTORS`).

**The cause in the code** (checked, not seen at the moment of the failure):
- **`usb_host`, xHCI bulk transfers:**
  - a transfer with no answer returned `ERR_TIMEOUT` and left its TRB on the ring, the endpoint still running, so the next transfer waited behind it;
  - any error but a stall (a transaction error, for one) halts the endpoint on the controller, and it was not reset, so every later transfer failed.

  The EHCI path resets its queue head after both; the control path restarts endpoint 0.
- **`usb_storage`:** a failed BOT cycle claimed the interface again without giving the old claim back. `usb_host` gives only unclaimed interfaces, so a device that had only missed a transfer, still attached, could never be claimed again: `THE DEVICE IS GONE` until the next boot.

**Why the disk is busy while all is well** (the log ring of the failed boot, 1480 s):
- **How often.** `vfs_server` saves the system log to `log:bootNNNN.log` at most every 2 s whenever new records came. Records came in 134 of the 740 two-second windows, so the log was saved every 11 s on average.
- **What a save costs.** After the reboot, one save took `usb_storage` about 110 system calls, roughly 25 SCSI commands. From the code:
  - the file found in the directory;
  - the data and its directory entry;
  - three SYNCHRONIZE CACHE;
  - the FAT's clean bit written twice (dirty, then clean).
- **What brings the records.** `bcm_wifi`'s 10-second traffic tally (550-DRV-0036) is four lines whenever data went out, so all the time, and nearly all of the windows. Without it a record came about every 90 s (the updater, the agent's reconnection).

## Plan

1. **`usb_host`, xHCI bulk:**
   - **no answer:** stop the endpoint and move its dequeue pointer past the TRB left behind (`restart`, as endpoint 0 does);
   - **any error:** reset it on both sides, as after a stall (`recover` and CLEAR_FEATURE ENDPOINT_HALT), and say so in the log for any code but a stall.
2. **`usb_storage`,** before claiming again after a failed cycle: a Bulk-Only Mass Storage Reset to the interface, then the old claim released. The same interface can then be claimed, its capacity checked and the request repeated.
3. **`bcm_wifi`'s tally:**
   - a period is logged only when something was lost: a frame not acknowledged, a DMA error, the ring full, or five beacons or more missed in a row (runs of two to four come every few periods on the Mac with nothing lost);
   - the quiet periods are summed into one line every 10 minutes.
4. **The log's own cost** is `vfs_server`'s (KRN): asked on `internal-dev-messages` (`KRN/`): fewer saves for ordinary records, and fewer flushes a save.

## Acceptance criteria

- Plan steps 1–3 built, and the suites that boot from a USB stick or move it pass:
  - `x86: USB image`: unplugged, back, another medium;
  - `x86: files and block writes`: the block suite through ATA, AHCI and USB;
  - `x86: keys, shell, tools`: the `usb` and `ehci` suites.
- **On the MacBook Pro:**
  - its disk saved to less often, the log showing the tally's lines only for losses;
  - a missed transfer, when one comes, followed by `THE DEVICE IS BACK` rather than `THE DEVICE IS GONE`.

  Nothing can make the stick miss a transfer at will, so the second point waits for the machine to do it again (MC-12.1, MC-12.2).

## Progress (2026-11-09)

**Built** on `claude/ASR-DRV`: plan steps 1 to 3.

**On the MacBook Pro, live.** `bcm_wifi` with step 3 replaced over the link, built on the Mac's own tree, 101b4f47 (gone at the next boot):
- **The first build** logged a period for beacons missed two to four in a row. Those come every few periods there with nothing lost, so it still logged four periods in three minutes.
- **The second build** (`bcm_quiet2`) takes only five or more in a row as a loss. In its first 90 s, `bcm_wifi` logged nothing.
- **What brings records now:** the updater. Its check runs every 20 s, the Mac's `update.txt` `every`, and logs two lines each time. Its source is a directory, as no TLS goes with the checks, so each check reads the stick too. That setting is the owner's; a longer period would leave the disk mostly quiet.

`usb_host` and `usb_storage` reach the Mac only with a slot update: `usb_host` serves the boot disk, so it cannot be replaced while the machine runs.

## The stick lost again, seen whole (2026-11-09, 13:13 UTC)

**The Mac** still ran its old boot image (101b4f47, without this fix), 13 h after the morning's reboot. This time the log ring held the moment:
- **47497 s:** the updater's check, the last one that read the disk (`REFUSED: Older`).
- **47521 to 47581 s:** a minute with nothing in the log. In it `usb_host` waited on a bulk transfer of the stick's that never ended. `usb_host` serves every USB client in turn, so the link's converter went silent too: `effector485` got "no answer".
- **47583 s:** `[USB] STORAGE: NO ANSWER TO READ: THE DEVICE IS GONE OR WAS RESET; CLAIMING IT AGAIN`.
- **47585 s:** `[USB] STORAGE: THE DEVICE IS GONE`, 2 s later. That is `claim`'s 20 tries 100 ms apart, while its old claim was still held.
- **From 47652 s:**
  - `[VFS] THE USB DRIVE DOES NOT ANSWER: REQUESTS FAIL AT ONCE, IT IS ASKED AGAIN EVERY 10 S`;
  - every file request refused (`Loader(Unreadable)`, link's code 3).

This is the sequence plan steps 1 and 2 fix:
- the stuck TRB is stepped past;
- the interface is reset and released before the claim.

Why the stick did not answer is not known. It has happened twice, 13 to 15 h into a boot. Saved: scratchpad `mac-lost2-1309.log`, session 4c527436.

The Mac was rebooted over the link at 18:48 UTC, and its slot is being updated to `main` (ABI 29) with this fix.

## Related

[211-DRV-0019](../issues-done/211-DRV-0019-usb-storage-refusals-and-resets.done), [211-DRV-0021](211-DRV-0021-a-gone-device-fails-at-once.md), [550-DRV-0036](550-DRV-0036-the-radios-traffic-in-the-log.md), [211-KRN-0019](../issues-done/211-KRN-0019-boot-logs-on-the-log-partition.done), [211-KRN-0050](211-KRN-0050-a-program-on-an-unplugged-disk-is-refused-at-once.md).
