# 100004-MOB-0004 — Effector to the boards over their UART0, from Windows, Linux and MIND Core hosts

**Type:** integration (a board, the link, Effector) · **Owner:** `MOB` · **Priority:** P1 · **Status:** open · **Blocked by:** [100004-MOB-0002](100004-MOB-0002-the-first-boot-on-the-h5.md), [100004-MOB-0003](100004-MOB-0003-the-first-boot-on-the-h618.md); `LNK`: [100000-LNK-0038](100000-LNK-0038-link-over-the-boards-console-uart.md) (the channel's case), [800-LNK-0041](800-LNK-0041-usb-serial-cp210x-pl2303-and-several-converters.md), [800-LNK-0042](800-LNK-0042-mind-core-as-the-master-of-a-serial-line.md), [800-LNK-0043](800-LNK-0043-effector485-a-line-without-link.md); `EFF`: the Effector project's task 108 ([100000-EFF-0026](100000-EFF-0026-serial-ports-through-effector.md)) and MIND Core's `effector` as a serial host (100004's message to `EFF`) · **Main task:** [100004](100004-two-allwinner-boards-before-the-phone.md) · **Constitution:** MC-6.1, MC-7.1, MC-7.4, MC-7.5, MC-12.1

## Problem

**The maintainer (2026-11-10):**
- the boards are plugged into the DEXP, the Ryzen or the MacBook over USB;
- they are reached while those machines run Windows or Linux, and while they run MIND Core;
- Effector works with MIND Core over RS-485 and RS-232 as with Windows and Linux, as the line's master and as its slave.

**What exists:**
- `effector485` on Windows and Linux, the master of a MIND Link line;
- `link` on MIND Core, the slave, over a USB converter (`usb_serial`) or the board's UART (`uart_serial`);
- `link.conf`'s word `uart` (100000-KRN-0221).

**What does not exist yet:**
- the Effector agent's serial module on Windows and Linux;
- a master on MIND Core;
- a raw console in effector485 for U-Boot's prompt;
- the board's UART through the kernel's channel, on a real board.

## Plan

1. **The board's side:** `uart` in the card's `link.conf`.
   - `uart_serial` drives UART0 through the kernel's console channel (chip 5), since the page holds other UARTs ([100000-KRN-0222](100000-KRN-0222-the-kernels-console-as-a-byte-channel.md)).
   - The boot's lines come before `link` answers, as console text.
2. **A Windows or Linux host:**
   - `effector485 daemon` on the adapter's port: `hello`, `log`, `put`, `get`, `run`, `shell`, `reboot`;
   - U-Boot's prompt through its console ([800-LNK-0043](800-LNK-0043-effector485-a-line-without-link.md)).
3. **Through the Effector server:** the same operations from an agent session on another machine, once the Effector agent's serial module exists (the Effector project's task 108, [100000-EFF-0026](100000-EFF-0026-serial-ports-through-effector.md)).
4. **A MIND Core host** (the MacBook, the Ryzen, the DEXP when they run MIND Core):
   - the adapter on `usb_serial` ([800-LNK-0041](800-LNK-0041-usb-serial-cp210x-pl2303-and-several-converters.md));
   - `serialhost` as the master ([800-LNK-0042](800-LNK-0042-mind-core-as-the-master-of-a-serial-line.md));
   - the host itself still the slave of its own RS-485 line to Windows;
   - then through MIND Core's `effector`.
5. **Updates:** a release put over the link, taken up by the updater, as on the MacBook ([800-LNK-0040](800-LNK-0040-the-updater-looks-once-a-minute.md)). This needs a disk: [100004-MOB-0005](100004-MOB-0005-the-allwinner-mmc-controller.md).

## Acceptance criteria

- **On each board,** from a Windows or Linux host: `hello`, `log`, `put` and `get` of 64 KiB compared, `run`, `shell` and `reboot` through effector485 at 115200.
- **The same from a MIND Core host,** through `serialhost`.
- **Through the Effector server,** from another machine: at least `hello` and `log`, on a host of each kind that has the module.
- **Each run's configuration recorded:** the board, the adapter, the host's system, the speed (MC-12.1).

## Related

[docs/remote-serial.md](../docs/remote-serial.md), [docs/link/effector485.md](../docs/link/effector485.md), [800-LNK-0039](800-LNK-0039-effector485-as-the-effector-agents-engine.md), [100000](100000-a-phone-the-pinephone-first.md).
