# 100000-KRN-0222 — The kernel's console as a byte channel (the A64's page of four UARTs)

**Type:** kernel (ABI) · **Owner:** `KRN` (the kernel session) · **Priority:** P2 · **Status:** open · **Blocked by:** — (100000-KRN-0171 in `main` since round 99) · **Main task:** [100000](100000-a-phone-the-pinephone-first.md), stage 0 · **Roadmap:** track H · **Constitution:** MC-1.1, MC-2.3, MC-3.1, MC-12.4

## Problem

- **The A64's four UARTs share one 4 KiB page:** UART0 (the console, the headphone jack's), UART1 (the Bluetooth chip's) and UART3 are enabled on the PinePhone.
- **100000-KRN-0171 lends that page to none,** so neither `uart_serial` (100000-KRN-0221) nor anything else can drive UART0 by its registers.
- **LNK's answer** (2026-11-09): a byte channel from the kernel is enough for `link`. At 115200 the line moves about 11.5 KB/s, and a frame carries at most 1 KiB.

## Plan

The kernel has a ring and a blocking read already: a task's text input (`TEXT_QUEUE`, 4 KiB; `TEXT_READ` waits up to `TEXT_WAIT_MS` for bytes and answers `ERR_BUSY`, to be asked again; 000-KRN-0094). The channel feeds that input from the console's line, so the kernel grows by the console's interrupt and one call, not by a second ring and a second wait (MC-1.2). The owner revised the plan's first draft (its own ring and read) so, on 2026-11-09.

1. **The ring to `common/`** (`common/text_ring.rs`): the text input's ring as it is, shared by the kernel and a host test.
2. **`PLATFORM_CONSOLE`** (a platform call; ABI 29): `init` asks for the console as a channel for a task it started, by PID, where the console's node cannot be lent by its page.
   - **What the kernel does:**
     - it gives that task a text input if it has none, and feeds it from the console's receive interrupt;
     - it returns a `CAP_KIND_CONSOLE` capability, the channel's writer, for `init` to grant to that task;
     - it keeps the console's line itself (not lent while the channel stands) and turns the UART's receive interrupt on: 16550 `IER.ERBFI`, PL011 `IMSC.RXIM | RTIM`.
   - **Refused:** where the kernel drives no console it can read (x86's COM1 is lent by its ports), and while another channel stands.
3. **Writing:** `TEXT_WRITE` with the console capability queues the bytes in a second ring, and returns the count queued (0 when it is full). The kernel drains that ring from the transmit interrupt: 16550 `IER.ETBEI`, PL011 `IMSC.TXIM`.
4. **While the channel stands,** the kernel's own lines stop going to the console, as when the PL011 is lent (`MIRROR_LOGS`). The panic and stop lines still go, polled.
5. **When the holder ends,** the interrupts go off, and the line and the console's lines are the kernel's again.
6. **UART1 and UART3 stay the kernel's** (MC-2.3): no one drives Bluetooth's UART by sharing the console's page. Their nodes are lent to none, as now.
7. **Users:** `uart_serial` serves `serial.wit` over the channel on the A64 (100000-KRN-0221, after LNK's `uart_serial` is in `main`), and `link` does not change. Until then a test program echoes through it.

## Acceptance criteria

- **Host tests** of the ring: its bounds, a full ring, a timeout.
- **In QEMU,** with a tree whose console shares its page with another enabled UART (a test tree, or the forum's A64 fork):
  - the console's node is not lent;
  - the channel carries `link`'s frames at 115200;
  - the kernel's lines resume when the holder ends.
- **docs/api** names the capability, and the ABI's version is raised.

## Related

[100000-KRN-0171](../issues-done/100000-KRN-0171-platform-devices-from-the-device-tree.done), [100000-KRN-0221](100000-KRN-0221-init-starts-uart-serial-for-link.md), [100000-PRT-0010](100000-PRT-0010-a-16550-uart-in-memory.md) (the 16550 the kernel drives).
