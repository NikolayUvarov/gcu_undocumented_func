# 000-DRV-0064 — `ata` and `ahci`: waits with a deadline, and checks that do not lean on the caller

**Type:** driver · **Owner:** `DRV` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** — (the critique of 2026-10-14) · **Constitution:** MC-1.5, MC-2.4, MC-5.6, MC-6.1, MC-6.3

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

## Related

[issues-critica-done/02](../issues-critica-done/02_code_and_comliance_analysis.md), [000-DRV-0065](../issues-done/000-DRV-0065-virtio-blk-waits-a-whole-tick.done).
