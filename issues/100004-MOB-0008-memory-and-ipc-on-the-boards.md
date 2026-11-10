# 100004-MOB-0008 — The existing suites on the boards: MIND Core in 512 MiB and in 2 GiB

**Type:** tests on hardware · **Owner:** `MOB` · **Priority:** P2 · **Status:** open · **Blocked by:** [100004-MOB-0004](100004-MOB-0004-effector-to-the-boards-over-their-uart.md) (tests driven over the link) · **Main task:** [100004](100004-two-allwinner-boards-before-the-phone.md) · **Constitution:** MC-3.1, MC-5.1, MC-12.1, MC-12.2

## Problem

The QEMU suites (`smp`, `heap`, `services`, `bench`, `store`) ran only on emulated CPUs. The boards are the first real Cortex-A53 cores:
- 512 MiB on the H5;
- 2 GiB on the H618, the PinePhone's smaller memory.

The handoff asks for the 2 GiB capability table, IPC throughput and sandboxes there.

## Plan

1. **The suites' checks driven over the link** (`run` and `log` through effector485), as on the MacBook:
   - the frame pool's size against the UEFI map;
   - the heap's and the scheduler's checks;
   - `bench`'s IPC round trips and throughput;
   - applications started until memory ends, with the system still answering.
2. **The numbers recorded** for each board. They are measurements, not guarantees, and not the phone's (MC-12.1, 12.2).
3. **What fails** goes to `KRN` as a request.

## Acceptance criteria

- **The checks listed in step 1 pass on both boards,** or each failure is a recorded request.
- **`bench`'s results** are kept in this issue with the board's configuration.

## Related

[100004-MOB-0002](100004-MOB-0002-the-first-boot-on-the-h5.md), [100004-MOB-0003](100004-MOB-0003-the-first-boot-on-the-h618.md), [176](176-test-and-performance-utilities.md).
