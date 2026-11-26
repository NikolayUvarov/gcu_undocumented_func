# 213-DRV-0105 — The Ryzen's second NVMe disk writable

**Type:** driver · **Owner:** `DRV` · **Priority:** P3 · **Status:** open · **Blocked by:** [213-DRV-0060](../issues-done/213-DRV-0060-the-ryzens-second-nvme-and-its-analog-sound.done) (the disk read on the bench); the maintainer's word that the disk may be written, and which part of it · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.3

Split from [213-DRV-0060](../issues-done/213-DRV-0060-the-ryzens-second-nvme-and-its-analog-sound.done) on 2026-11-26.

## Problem

- Since 213-DRV-0060, an `nvme` instance other than the first is read-only at the testing stage (`nvme/src/guard.rs`).
  - It sends its controller only Identify, the I/O queues' creation and Read.
  - It answers every client that it cannot write, and refuses every write and flush, saying who asked.
- On the Ryzen, the second controller (04:00.0, a Netac 128 GB) holds the Ubuntu install that the machine's other users boot. Not a byte of it may change by accident.
- So no instance but the first can write today. That is not done, and it is not claimed.

## Plan

1. **The maintainer's word first:** whether the disk may be written at all, and which partition or range.
2. **Writable only when named:**
   - an explicit word in `init`'s argument for the instance, never a default;
   - a write is refused outside the named range;
   - `init`'s part is a KRN request.
3. **Tested on a scratch disk first,** in QEMU and then on a disk of the maintainer's that holds nothing of value:
   - write, flush (the volatile write cache), read back;
   - every sector outside the range is unchanged, by SHA-256 on the host.
4. **Then the Ryzen,** only if the maintainer asks for it.

## Acceptance criteria

- In QEMU: an instance named writable writes inside its range and nowhere else, and one not named stays read-only, as 213-DRV-0060 shows.
- On the Ryzen: only with the maintainer's word, the named range written and read back, and Linux still boots from the disk.

## Related

[213-DRV-0060](../issues-done/213-DRV-0060-the-ryzens-second-nvme-and-its-analog-sound.done), [213](213-amd-ryzen-pc.md).
