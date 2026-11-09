# 100000-KRN-0221 — `init` starts `uart_serial` on the console UART for `link` (the word `uart`)

**Type:** `init` · **Owner:** `KRN` (the kernel session) · **Priority:** P2 · **Status:** open · **Blocked by:** LNK's `uart_serial` and `serial.wit` 1.1 on `main` (100000-LNK-0038, on `claude/LNK-link`); on the A64, [100000-KRN-0222](100000-KRN-0222-the-kernels-console-as-a-byte-channel.md) · **Main task:** [100000](100000-a-phone-the-pinephone-first.md), stage 0 · **Roadmap:** track H, the link · **Constitution:** MC-2.3, MC-3.1, MC-3.3, MC-7.1

**Agreed with** LNK, 2026-11-09 (`KRN/2026-11-09T0445Z-LNK-your-two-requests-are-100000-lnk-0038-and-800-lnk-0039.md` on `internal-dev-messages`), on the kernel session's proposal of 2026-11-08.

## Problem

- **Until Wi-Fi exists, the PinePhone's way to Effector is the link over its console UART,** the headphone jack's (100000, stage 0).
- **`link` takes one kind of line today:** `idl/serial.wit` in `SLOT_DEV0`, from `usb_serial`. The phone has no USB serial converter on that path.
- **LNK's answer:** `link` stays as it is, and a small service of LNK's, `uart_serial`, serves `serial.wit` over the UART. Starting it and lending it are `init`'s.

## Plan

- **The word `uart` in `link.conf`** (`BootInfo.link_words`). When it is there, from `init`'s first start:
  - `init` starts `uart_serial` with the console UART:
    - on aarch64, the console's platform node (100000-KRN-0171): its registers by `PLATFORM_MMIO` in `SLOT_DEV0` and its line by `PLATFORM_NODE_IRQ` in `SLOT_IRQ`;
    - on x86, COM1's ports and IRQ 4;
    - on the A64, whose console page holds three other UARTs and is lent to none, the kernel's channel instead (100000-KRN-0222).
  - `link` gets `uart_serial`'s client in `SLOT_DEV0`, in place of `usb_serial`'s.
  - The shell gets no serial line, and says so.
- **The rate:** 115200, as `link` takes without `baud=`.
- **Without the word,** nothing changes.

## Acceptance criteria

- **QEMU `virt` (aarch64, the PL011) and x86 (COM1 on a pty),** with `uart` in `link.conf`:
  - `link` runs over the console UART;
  - `mindlink` (or `effector485`) on the host reaches it through that pty;
  - the shell has no serial line.
- **Without the word,** the shell has its serial line, as now.
- **docs/profile/bootstrap.md** names the grants.

## Related

[100000-KRN-0171](100000-KRN-0171-platform-devices-from-the-device-tree.md) (the console's node), [100000-KRN-0222](100000-KRN-0222-the-kernels-console-as-a-byte-channel.md) (the A64), [100000-PRT-0010](100000-PRT-0010-a-16550-uart-in-memory.md), [docs/remote-serial.md](../docs/remote-serial.md).
