# 000-DRV-0058 — `usb_storage` reports a device that came back, rather than running the command on it

**Type:** driver · **Owner:** `DRV` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [000-KRN-0085](000-KRN-0085-a-returned-device-reported-to-its-block-client.md) (the kernel session's half, which waits for this) · **Constitution:** MC-4, MC-6.3, MC-6.4

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

## Related

[000-KRN-0085](000-KRN-0085-a-returned-device-reported-to-its-block-client.md), [000-KRN-0079](../issues-done/000-KRN-0079-a-returning-drive-is-checked-to-be-the-same-medium.done), [211-DRV-0019](../issues-done/211-DRV-0019-usb-storage-refusals-and-resets.done), [211-DRV-0053](211-DRV-0053-a-usb-disk-that-missed-a-transfer-comes-back.md).
