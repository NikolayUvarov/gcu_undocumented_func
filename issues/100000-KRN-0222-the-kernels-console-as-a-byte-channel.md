# 100000-KRN-0222 — The kernel's console as a byte channel (the A64's page of four UARTs)

**Type:** kernel (ABI) · **Owner:** `KRN` (the kernel session) · **Priority:** P2 · **Status:** open · **Blocked by:** [100000-KRN-0171](100000-KRN-0171-platform-devices-from-the-device-tree.md) in `main` · **Main task:** [100000](100000-a-phone-the-pinephone-first.md), stage 0 · **Roadmap:** track H · **Constitution:** MC-1.1, MC-2.3, MC-3.1, MC-12.4

## Problem

- **The A64's four UARTs share one 4 KiB page:** UART0 (the console, the headphone jack's), UART1 (the Bluetooth chip's) and UART3 are enabled on the PinePhone.
- **100000-KRN-0171 lends that page to none,** so neither `uart_serial` (100000-KRN-0221) nor anything else can drive UART0 by its registers.
- **LNK's answer** (2026-11-09): a byte channel from the kernel is enough for `link`. At 115200 the line moves about 11.5 KB/s, and a frame carries at most 1 KiB.

## Plan

- **A capability over the kernel's console line:** `init` asks for it where the console's node cannot be lent by its page; a new ABI version.
- **What it gives:**
  - **reading:** a receive ring of at least 4 KiB in the kernel, filled from UART0's line (SPI 0 on the A64, within the lines the kernel binds), and a read that blocks until bytes come or a timeout passes, so that nothing polls;
  - **writing:** a write that returns once the bytes are queued, sent by the kernel as the line drains.
- **While it is lent,** the kernel's own lines stop going there, as when the PL011 is lent; the panic and stop lines still go.
- **`uart_serial`** serves `serial.wit` over the channel on the A64, and `link` does not change.
- **UART1 and UART3 stay the kernel's** (MC-2.3): no one drives Bluetooth's UART by sharing the console's page.

## Acceptance criteria

- **Host tests** of the ring: its bounds, a full ring, a timeout.
- **In QEMU,** with a tree whose console shares its page with another enabled UART (a test tree, or the forum's A64 fork):
  - the console's node is not lent;
  - the channel carries `link`'s frames at 115200;
  - the kernel's lines resume when the holder ends.
- **docs/api** names the capability, and the ABI's version is raised.

## Related

[100000-KRN-0171](100000-KRN-0171-platform-devices-from-the-device-tree.md), [100000-KRN-0221](100000-KRN-0221-init-starts-uart-serial-for-link.md), [100000-PRT-0010](100000-PRT-0010-a-16550-uart-in-memory.md) (the 16550 the kernel drives).
