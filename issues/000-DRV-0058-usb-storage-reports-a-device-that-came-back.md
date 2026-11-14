# 000-DRV-0058 — `usb_storage` reports a device that came back, rather than running the command on it

**Type:** driver · **Owner:** `DRV` · **Priority:** P2 · **Status:** in progress (2026-11-12: `usb_storage`'s part made; the criteria wait for `vfs_server`) · **Blocked by:** 000-KRN-0085's plan steps 2 and 3 (`vfs_server` checks the medium after a return, not only after a slow failure) · **Main task:** [000-KRN-0085](000-KRN-0085-a-returned-device-reported-to-its-block-client.md) (the kernel session's half, which waits for this) · **Constitution:** MC-4, MC-6.3, MC-6.4

Numbered from the kernel track's request of 2026-10-11 in `requests-DRV.md`.

## Problem

**What happens now.** Since 211-DRV-0019, a command that finds the device gone claims it again. If a device of the same capacity answers, the command runs on it (`THE DEVICE IS BACK, THE SAME CAPACITY`). Since [211-DRV-0053](211-DRV-0053-a-usb-disk-that-missed-a-transfer-comes-back.md), the interface is first reset and released, so a stick that only missed a transfer comes back too.

**Why that is a risk.** Our images all have the same capacity, so the device that answers may be a stick rewritten with another build.
- `vfs_server` checks the medium before it writes once it saw the drive not answer ([000-KRN-0079](../issues-done/000-KRN-0079-a-returning-drive-is-checked-to-be-the-same-medium.done)).
- It cannot see that when it made no request while the stick was out, for example when everything it needed was cached.
- Then the first request after the swap runs on the new stick, possibly a write of the old system's cached sectors, before anything can be checked.

## Plan

1. **When the device comes back, the command that found it is not run.** It fails, and `usb_storage` reports the return.
2. **The report goes through libmind's block protocol** (000-KRN-0085's part):
   - `Driver::returned()` is read by `block::serve` before each request;
   - the next request is answered `not-found`, and the client's buffer is dropped;
   - the client checks the medium before anything else.
3. **A stick that only missed a transfer** (211-DRV-0053) goes the same way. The check of the medium then finds it the same, and the work goes on.

## Acceptance criteria

`tests/usb_image_smoke.py`, x86:
- the stick is swapped for a copy with another manifest while `vfs_server` makes no request;
- the copy is unchanged afterwards;
- `vfs_server` logs `CAME BACK WITH ANOTHER MEDIUM`;
- the existing unplug and replug of the same stick still goes on as before.

## Progress (2026-11-12): `usb_storage` reports a return; `vfs_server` does not check it yet

**What "came back" means** (`usb_storage/src/recovery.rs`).
- **The key.** `usb_host`'s handle names one enumeration of a device: its generation, device and interface (`handle` in `usb_host/src/main.rs`). After a lost transfer `usb_storage` resets the interface and gives its claim back (211-DRV-0053), then claims every free interface, holding the others while it looks:
  - **its own handle, from the same `usb_host` instance:** the device never left `usb_host`'s table, so it only missed a transfer. No stick can be swapped without being enumerated again. Its capacity is checked and the command repeated, as 0053 made it (`THE DEVICE IS BACK, NOT ENUMERATED AGAIN, THE SAME CAPACITY`).
  - **another Bulk-Only interface that answers with a medium:** the stick plugged in again, or another device. A return.
  - **any interface after `usb_host` started again:** a return as well. A new instance numbers its devices from the start, so it may hand out the old handle's number. `usb_storage` knows from a peer error, or a claim refused for want of its buffer.
  - **its own interface saying UNIT ATTENTION 28h** (the medium may have changed: a card reader's card): a return. Before, the command was repeated after it.
  - **nothing:** gone, as before, after the 2 s of tries. `vfs_server` still sees a drive that does not answer.
- **Why no comparison of vendor, product, serial, capacity or a sector's hash.** None of them can show the same medium: this issue's case is the same stick rewritten with another build, its identity and capacity unchanged. Only the content tells, and checking it is the client's (000-KRN-0079; `init`'s checks of the partitions before it sets a window). So any enumeration again is a return, whatever the device reports.
- **Another capacity is not used,** as before: `ANOTHER MEDIUM (N SECTORS) IS NOT USED`, with no return. Reported as a return, it would be served by a fresh instance with nothing checking it (below), where it was refused before.
- **On a return** the command is not run and fails with `Gone` (not-found). `returned()` then answers true, so `block::serve` ends the instance at the next request; `Device`'s re-attach makes that request at once. `init` starts `usb_storage` again and sets the store's and `vfs_server`'s windows again only after its checks.

**The plan's step 3 changed.** A stick that only missed a transfer is not reported as a return:
- the same enumeration cannot hold another medium;
- on the MacBook Pro the stick is the boot disk: 0053's misses came 13 to 15 h into a boot, and a restart each time would cost the disk for the restart's wait and a place in the restart budget.

**The restart budget.** One restart a return; a missed transfer and a medium of another capacity cost none. A device enumerated again more than three times in 60 s leaves `usb_storage` quarantined until `run usb_storage` (`init`'s budget, MC-6.5), so a flapping device does not loop. Not tried in QEMU.

**Tested** (QEMU 8.2.2 on the WSL2 machine, 4 CPUs):
- **`tests/usb_storage_host.rs`** (new, in `scripts/host_tests.sh`), the production logic against a model of `usb_host`'s claims, 12 tests:
  - its own handle found behind another stick, which is not taken for a return;
  - a device enumerated again, or one still being set up for a few rounds: a return;
  - a gone device gets every try;
  - an empty reader or another class is no return, and is given back;
  - a new `usb_host` instance, or a peer error before the claim: the old handle's number is not the same device;
  - a full table ends without claiming its own interfaces again;
  - the verdicts, another capacity among them.
- **The full `scripts/host_tests.sh`:** passed.
- **x86, `tests/usb_image_smoke.py`:** passed, with these steps added:
  - **the existing unplug and replug** goes on as before, now through a return: `A DEVICE CAME BACK, ENUMERATED AGAIN`, `[BLOCK] THE MEDIUM WENT AWAY AND CAME BACK`, `[INIT] usb_storage RESTARTED`, windows 1, 2 and 3 set again, `THE USB DRIVE ANSWERS AGAIN`;
  - **the same stick out and in at once** (`device_del` and `device_add` together), so that `vfs_server` asks nothing meanwhile. The next request finds it enumerated again, never gone: the instance ends at once and the fresh one serves 30 ms later. `memmap` runs, and `log:note.txt`, written before, reads back;
  - **a copy one MiB longer swapped in the same way:** `ANOTHER MEDIUM (1427456 SECTORS) IS NOT USED`, no restart, the copy unchanged;
  - **the swap 000-KRN-0079 checks** (`vfs_server` saw the drive not answer) goes on, through a return.
- **aarch64, the same suite** (`--arch aarch64`, as CI runs it): passed, every step above with it.
- **x86, "files and block writes"** (the block suite through ATA, AHCI and USB): passed.
- **Builds:** x86 and aarch64.

**Not met: the criterion's swap,** because of `vfs_server`. A run of the suite with the copy of another manifest swapped in with no request while it was out (not committed) showed:
- `usb_storage` found the copy enumerated again, ran nothing on it and ended, 53 ms after the request. The fresh instance served 30 ms later.
- `init` set the windows again over the copy, whose table is the stick's.
- `vfs_server` checked nothing: no `CAME BACK WITH ANOTHER MEDIUM`, and the copy was written (`write log:c58.txt`, `sync`).

**Why.** `vfs_server`'s check (000-KRN-0079) runs only after a request it saw take `SLOW_MS` (1 s) or more (`silent_until` in `vfs_server/src/disk.rs`). `block::Device` re-attaches after not-found or peer and repeats the request without telling it. So the criterion waits for 000-KRN-0085's plan steps 2 and 3: the client told of the return (`Device::check_returns`), and `vfs_server` checking the medium then. A medium of the same capacity swapped while unused is still written, as before this task; the block store's window over it is now set only after `init`'s check.

**Left:**
- **KRN:** 000-KRN-0085's steps 2 and 3, above.
- **DRV, once they are in:**
  - a medium of another capacity becomes a return too;
  - the criterion's swap goes into `tests/usb_image_smoke.py`.
- **Not tried:**
  - the MacBook Pro's real stick (MC-12.3): a missed transfer that is not a return (`NOT ENUMERATED AGAIN`) needs the machine to miss one, as 0053's criterion does;
  - a card reader's medium change (UNIT ATTENTION 28h);
  - `usb_host` restarting under a stick;
  - a flapping device and the quarantine.

## Related

[000-KRN-0085](000-KRN-0085-a-returned-device-reported-to-its-block-client.md), [000-KRN-0079](../issues-done/000-KRN-0079-a-returning-drive-is-checked-to-be-the-same-medium.done), [211-DRV-0019](../issues-done/211-DRV-0019-usb-storage-refusals-and-resets.done), [211-DRV-0053](211-DRV-0053-a-usb-disk-that-missed-a-transfer-comes-back.md).
