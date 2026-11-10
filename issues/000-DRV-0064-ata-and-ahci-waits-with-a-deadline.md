# 000-DRV-0064 — `ata` and `ahci`: waits with a deadline, and checks that do not lean on the caller

**Type:** driver · **Owner:** `DRV` · **Priority:** P2 · **Status:** in progress (2026-11-10) · **Blocked by:** — · **Main task:** — (the critique of 2026-10-14) · **Constitution:** MC-1.5, MC-2.4, MC-5.6, MC-6.1, MC-6.3

Numbered from the kernel track's request of 2026-10-14 in `requests-DRV.md`, from the critique [issues-critica-done/02](../issues-critica-done/02_code_and_comliance_analysis.md), §2.1–2.2. Checked again on `claude/ASR-DRV` d252e168: the waits below are as the request found them.

## Problem

**`ata/src/main.rs`:**
- **Unbounded waits:** `command` and `flush` wait `while self.io.in8(COMMAND) & BSY != 0 {}`. A drive stuck busy hangs the driver, and every request of `vfs_server` with it (MC-6.1).
- **Waits counted in reads, not time:** `probe`, `wait_ready` and `wait_data` count port reads. Each read is a system call, so how long they wait depends on the machine (MC-5.6).
- **The count:** `count as u8` and `chunks_exact(512).take(count)` would go wrong for a count of 0, above 256, or longer than the buffer. Today `block_protocol::span` holds every request to 1–128 sectors, so nothing reaches it.

**`ahci/src/main.rs`:**
- **The BIOS/OS handoff** ignores what `wait` returns, and does not wait for BOHC.BB (BIOS busy) to clear as the AHCI specification asks (MC-1.5, MC-6.3).
- **The DMA region:** `read` and `write` slice it at `DATA` plus `count × 512` with no check. That holds only because `BLOCK_MAX_SECTORS` and `init`'s `AHCI_DMA_BYTES` fit together (MC-2.4).
- **`wait`** sleeps in 10 ms steps. On x86 the tick is 1 ms since [000-KRN-0083](../issues-done/000-KRN-0083-a-1-ms-tick-for-sleeps-and-deadlines.done).

**`usb_host/src/xhci.rs`:** its comment still says "a sleep is a 10 ms tick".

## Plan

1. **One deadline helper in each driver,** on `monotonic_ns`, for every wait:
   - `command` and `flush` return an error on timeout;
   - `read` and `write` pass it on as `ERR_IO`.
2. **Each `read` and `write` checks `count`** against the DMA region and the buffer itself, and refuses rather than panics.
3. **`ahci`'s handoff:**
   - its result is logged;
   - it waits for BB;
   - if the firmware keeps the controller, the driver leaves it and says so.
4. **`ahci`'s `wait`** sleeps in 1 ms steps to its deadline, as `nvme` does.
5. **The stale comment** in `xhci.rs` goes.

## Acceptance criteria

- The `vfs`, `disk` and `block` suites pass on IDE and AHCI disks.
- A request on a drive that never leaves BSY ends with `ERR_IO` within the stated time. A host test with a stand-in for the ports shows it.
- No path in either driver indexes the DMA region or the buffer without a check of its own.

No real machine is needed.

## Progress (2026-11-10)

**Plan steps 1 to 5 made**, with the failure's code left: see the last point below.
- **`ata`:** `ata/src/ide.rs`, the channel's commands over a `Bus` (the ports and a clock), so that a host test drives them.
  - **Each wait** is polled for 2 ms, then between 1 ms sleeps, to a deadline on `monotonic_ns`:

    | Wait | Deadline |
    |---|---|
    | IDENTIFY's at probe | 5 s |
    | A command's start, each sector's DRQ, a write's end | 10 s |
    | A flush | 30 s |

  - **On a timeout** the request fails: `Busy`, `NoData` or the drive's `Error`. The log names a run of failures once, and "THE DRIVE ANSWERS AGAIN".
  - **Each `read` and `write` is checked by the driver:** 1 to 256 sectors, LBA28, the drive's end and the buffer's length. Anything else is refused before a port is touched, and logged.
- **`ahci`:** `ahci/src/checks.rs`.
  - **The handoff:** OOS set, BOS awaited 25 ms, and while BB is set 2 s more.
    - Taken: logged with its time.
    - Silent (BOS kept, BB clear): taken, as AHCI 10.6.3 allows, and said so.
    - Still busy after 2 s: taken anyway, and said so. Before 000-DRV-0064 the controller was always taken; leaving it to a slow firmware would cost the boot disk, and the i5 PC boots from SATA.
  - **Every wait** is to a deadline, sleeping 1 ms:

    | Wait | Deadline |
    |---|---|
    | CR and FR stopping | 500 ms |
    | The device ready after the start | 10 s |
    | A command | 10 s |
    | A flush | 30 s |

  - **A failed command,** timed out or a task-file error, restarts the port and is logged once a run.
  - **The checks:** the DMA region must hold a data area at probe. Each `read` and `write` is checked against one command (1 to 65535 sectors), the drive, the buffer and the data area before anything is sliced.
- **`usb_host/src/xhci.rs`:** the stale "a sleep is a 10 ms tick" comment is gone.
- **`tests/qemu_smoke.py`:** `--disk ahci` boots the suites from a SATA disk on AHCI, and the `disk` suite's mount check names the boot drive.

**Tested** on this machine (x86, QEMU 8.2.2, TCG):
- `tests/ata_ahci_waits_host.rs`, 6 tests:
  - a drive that never leaves BSY fails at 10 s, its command never written;
  - refusals before the ports;
  - 256 sectors as one command;
  - reads, writes and errors on a stand-in;
  - AHCI's data-area check;
  - its handoff taken, silent, waited for and kept;
  - its waits' deadline.
- `scripts/ci_local.sh --only x86`, all PASS (each suite boots from IDE):
  - `build (x86 test programs)`;
  - `x86: boot, display, shell, memory, clock`;
  - `x86: services, storage, audio`, with the `ahci` suite;
  - `x86: files and block writes`: `vfs`, `edit`, `disk`, `block` (ATA, AHCI and USB), `store`, `storefaults`.
- **`--suites vfs,disk --disk ahci`:** both pass from an AHCI boot disk. `block` is left out there, as it runs ATA, AHCI and USB itself.
- The whole `scripts/host_tests.sh`: 199 suites, none failed.

**Left: the failure's code.** A timed-out request reaches the client as the block protocol's failure, `Peer`, not `ERR_IO` as criterion 2 says:
- `Driver::read` and `write` answer a bool;
- `block_protocol` (libmind) answers `false` as `Peer`, which `block::Device` repeats once.

`ERR_IO` needs the protocol to tell a driver's timeout from a lost driver. That is libmind's core, KRN's.

**The failure's code, answered (2026-11-10)** by [000-KRN-0228](000-KRN-0228-a-block-drivers-failure-reaches-the-client-as-io.md) and DRV's port of the six block drivers to its `Outcome`, in one landing request:
- `ata`'s `Busy`, `NoData` and `Error` and `ahci`'s failed command (timed out or a task-file error) answer `DriverError::Io`, which reaches the client as `io` (`ERR_IO`) after one call, never as `peer`;
- a request the driver's own checks refuse answers `Range`, which is `invalid`.

What shows it: `tests/ata_ahci_waits_host.rs` (the 10 s `Busy`) and `tests/block_host.rs` (`Io` as `io`, never `peer`), each its half. No QEMU suite makes a drive fail. The issue closes once that request is in `main`.

## Related

[issues-critica-done/02](../issues-critica-done/02_code_and_comliance_analysis.md), [000-DRV-0065](../issues-done/000-DRV-0065-virtio-blk-waits-a-whole-tick.done).
