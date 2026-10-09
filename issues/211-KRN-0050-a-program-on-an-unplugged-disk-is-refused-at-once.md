# 211-KRN-0050 — A program started while its disk is unplugged is refused at once, with the reason

**Type:** kernel (`vfs_server`, `loader`) · **Owner:** `KRN` · **Priority:** P1 · **Status:** in progress · **Blocked by:** — (the `DRV` part: [requests-DRV.md](requests-DRV.md)) · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Constitution:** MC-6.6, MC-10.2, MC-12.3

## Problem

The maintainer unplugged the boot disk (a USB disk on the MacBook Pro's xHCI) and plugged it in again on 2026-10-09 (fast-test 57242b9c78ff). The system survived that and started programs afterwards. But starting a program while the disk was out hung the system, where it should say why and go on.

What happens, from the code:
- **`usb_host`.** A transfer to a device that has gone waits up to about 30 s for a completion that never comes. Meanwhile `usb_host` does not see the port's disconnection (no `PORT 5: DISCONNECTED` in the log, only the `CONNECTED` of the replug). `usb_storage` then tries to claim the interface again for 2 s. Each request to the disk costs about 32 s.
- **`vfs_server`** serves one request at a time. Its journal saves the system log to `log:`, on the same disk, every 2 s. So the queue fills with 32-second failures.
- **The journal** counted its records as saved before writing them, so the records of the time without the disk were lost. A failed read of the free space was taken for a full volume, which stopped the log for the rest of the boot.
- **`loader`** turned the read error into `invalid`, so the shell would print `NOT A PROGRAM FILE`.

## Plan

- **`vfs_server` (`disk.rs`).** A drive whose request failed after more than 1 s does not answer. For 10 s every request to it fails at once, without asking it. Cached sectors are still read. The log says `[VFS] THE USB DRIVE DOES NOT ANSWER …` once, and `… ANSWERS AGAIN` when it does.
- **The journal.**
  - A save that fails keeps its records for the next save while logd still holds them.
  - It is tried again after 30 s, not 2.
  - A volume that cannot be read is not taken for a full one.
- **`loader.wit` 1.7.** The error `unreadable`: the program's file could not be read.
  - `ERR_IO` in `common/abi.rs` carries it through `mind::sys::Error`. It is for services only; the kernel does not return it, so the ABI version stays.
  - The shell says `CANNOT READ THE PROGRAM: ITS DISK DOES NOT ANSWER (UNPLUGGED?)`. Its match on loader errors is exhaustive, so the interface change takes the shell's mapping with it. `APP` may word it otherwise.
- **`usb_host` and `usb_storage` are `DRV`'s** (requested): a transfer to a device whose port is disconnected ends at once, port changes are seen during waits, and a gone interface is not reclaimed for 2 s on each request. Until then, a request to the gone drive still waits about 30 s once in every 10 s window.
- **The test.** `tests/usb_image_smoke.py` boots from a USB disk on xHCI.
  - It unplugs the disk (`device_del`) and starts a program on it. The refusal must come within the test's limit, and the shell answers meanwhile.
  - It plugs the disk in again (`drive_add`, `device_add`), and the program starts.

## Acceptance criteria

- **In QEMU, with the disk unplugged:**
  - `run app` is refused with `CANNOT READ THE PROGRAM` within 15 s;
  - `ps` answers;
  - `vfs_server` logs that the drive does not answer.
- **After the replug,** `run app` starts the program and `vfs_server` logs that the drive answers again. The boot log holds records from the time the disk was out, as far as logd's ring kept them.
- **The MacBook Pro.** The same: a refusal with the reason, not a hang. How fast depends on `DRV`'s part.

## Related

[211-KRN-0019](211-KRN-0019-boot-logs-on-the-log-partition.md) (the journal), [requests-DRV.md](requests-DRV.md), [requests-APP.md](requests-APP.md).
