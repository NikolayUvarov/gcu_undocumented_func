# 100004 — Two Allwinner boards before the phone: the Orange Pi Zero Plus 2 (H5) and the Orange Pi Zero 3 (H618)

**Type:** porting (main task) · **Owner:** `MOB` (the assessing session, `claude/MOB-boards`) · **Priority:** P1 · **Status:** started (2026-11-10) · **Blocked by:** for the runs, the boards wired to a host ([issues-human, section 14](../issues-human/README.md#14-the-two-allwinner-boards-on-the-bench)) · **Roadmap:** track H (boards with UEFI) · **Constitution:** MC-1.3, MC-1.5, MC-2.3, MC-6.1, MC-7.1, MC-12.1, MC-12.3

**Recorded by:** the assessing session, 2026-11-10, at the maintainer's word: «Забери себе трек MOB. У меня две платы для начальной работы с ними», with the maintainer's handoff of that day (track MOB, the PinePhone target and a lab of two boards).

## Problem

**The PinePhone is not on the bench yet** ([100000](100000-a-phone-the-pinephone-first.md), stage 1 waits for it). Two Allwinner boards with Cortex-A53 cores are:

| | Node A: Orange Pi Zero Plus 2 H5 (v1.0) | Node B: Orange Pi Zero 3 (2 GB) | The PinePhone (A64) |
|---|---|---|---|
| SoC | H5 (`sun50i-h5`), 4 × Cortex-A53 | H618 (`sun50i-h616` family), 4 × Cortex-A53 | A64 (`sun50i-a64`), 4 × Cortex-A53 |
| RAM | 512 MiB DDR3 at 0x40000000 | 2 GiB LPDDR4 at 0x40000000 | 2 or 3 GiB at 0x40000000 |
| Interrupts | GIC-400 (GICv2), GICD 0x01C81000 | GIC-400 (GICv2), GICD 0x03021000 | GIC-400 (GICv2), GICD 0x01C81000 |
| Console | UART0 at 0x01C28000, the 3-pin header | UART0 at 0x05000000, the 3-pin header | UART0 at 0x01C28000, the headphone jack |
| Storage | microSD on mmc0 (0x01C0F000), 8 GB eMMC on mmc2 (0x01C11000) | microSD on mmc0 (0x04020000), 16 MiB SPI NOR | microSD on mmc0, eMMC on mmc2 |
| PMIC | none: discrete regulators | AXP313a on R_I2C (0x07081400) | AXP803 on RSB (0x01F03400) |
| Network | Wi-Fi and BT (AP6212A on SDIO and a UART); no Ethernet | Gigabit Ethernet; Wi-Fi and BT | none until its Wi-Fi (RTL8723CS) |
| USB | EHCI and OHCI, MUSB OTG | EHCI and OHCI, MUSB OTG | EHCI and OHCI, MUSB OTG; the modem on USB |

These addresses are the handoff's. Each one is checked against the board's device tree before code relies on it (MC-12.3).

**What carries over to the phone, and what does not.**
- **The H5 shares the A64's addresses for the blocks in the table:**
  - the UARTs, the GIC and the pin controller;
  - the MMC controllers, TWI0 and USB.
- **It is not a clone of the A64.** The H5 is the H3's 64-bit sibling. Its clock controller, display engine and PMIC bus differ. The A64's counter erratum (`allwinner,erratum-unknown1`, [100000-PRT-0012](100000-PRT-0012-the-a64s-counter-erratum.md)) is the A64's only.
- **The H618 uses the H6/H616 layout,** with every address different from the A64's.
- **So:**
  - a driver binds by its node's `compatible` and takes its registers from the tree. MIND Core has one aarch64 image for every board: no board selected at build time ([100000-PRT-0009](../issues-done/100000-PRT-0009-the-board-from-the-device-tree.done), [100000-KRN-0171](../issues-done/100000-KRN-0171-platform-devices-from-the-device-tree.done));
  - a result on a board is evidence for that board only, never for the phone (MC-12.1).

**The handoff's other points, as this repository has them:**
- **The governing texts** are the Constitution 1.6 and RFC 001 0.4 in `constitution/EN`, not 1.5.
- **The boot chain** (U-Boot's UEFI, `BOOTAA64.EFI`, the tree from `EFI_DTB_TABLE_GUID`, EL2 to EL1, the MMU with device memory) is what the aarch64 port does already. It ran under U-Boot only in QEMU's `virt` ([100000-PRT-0009](../issues-done/100000-PRT-0009-the-board-from-the-device-tree.done)).
- **Booting from the network** through U-Boot's TFTP is not used. The firmware's network boot runs outside every guarantee ([351-KRN-0195](../issues-done/351-KRN-0195-no-firmware-network-boot-before-the-loader.done)). The card is the boot medium.
- **Marain's runtime** (WebAssembly components) belongs to the `MRN` track, which is open and has host tools only. The boards run what MIND Core has.

## The way to the boards: the serial line, through Effector

**The maintainer (2026-11-10):**
- the boards are connected to the DEXP, the Ryzen or the MacBook over USB, while those run Windows or Linux and while they run MIND Core;
- Effector works with MIND Core over RS-485 and RS-232 as it does with Windows and Linux, as the line's master and as its slave.

**What that takes:**

| Where | The line's master (it holds the adapter, drives the board) | The line's slave (it is driven over the line) |
|---|---|---|
| Windows, Linux | `effector485` today ([800-LNK-0039](800-LNK-0039-effector485-as-the-effector-agents-engine.md)); a line without `link` (U-Boot's prompt): [800-LNK-0043](800-LNK-0043-effector485-a-line-without-link.md); the Effector agent's serial module ([docs/remote-serial.md](../docs/remote-serial.md), section 4: the Effector project's task 108, handed over by `EFF` in [100000-EFF-0026](100000-EFF-0026-serial-ports-through-effector.md)) | `mindlink serve` |
| MIND Core | [800-LNK-0042](800-LNK-0042-mind-core-as-the-master-of-a-serial-line.md) (`serialhost`), on the converters of [800-LNK-0041](800-LNK-0041-usb-serial-cp210x-pl2303-and-several-converters.md); MIND Core's `effector` offers it (through `EFF`) | `link` on `usb_serial`, and on the board's UART (`uart_serial`, [100000-LNK-0038](100000-LNK-0038-link-over-the-boards-console-uart.md)) |

**The wiring:** the board's UART0 header to a 3.3 V USB-UART adapter (TX to RX, RX to TX, GND; never VCC), the adapter in the host's USB port.

## Plan

### Stage 1: a card, and the first boot on each board

- [100004-MOB-0001](100004-MOB-0001-the-boards-sd-card.md): the boards' card, U-Boot with TF-A before MIND Core's image.
- [100004-MOB-0002](100004-MOB-0002-the-first-boot-on-the-h5.md): the first boot on the Orange Pi Zero Plus 2 H5.
- [100004-MOB-0003](100004-MOB-0003-the-first-boot-on-the-h618.md): the first boot on the Orange Pi Zero 3.
- **Asked of `PRT` and `KRN`** (the kernel session; message `KRN/2026-11-10T…-MOB-the-two-allwinner-boards-…`):
  - the boards' device trees in `tests/data/fdt` with host tests;
  - the DesignWare UART's busy-detect;
  - the kernel's console channel ([100000-KRN-0222](100000-KRN-0222-the-kernels-console-as-a-byte-channel.md)) on a 16550, for these boards whose console page holds other UARTs too.

### Stage 2: Effector to the boards

- [100004-MOB-0004](100004-MOB-0004-effector-to-the-boards-over-their-uart.md): `link` on each board's UART0, reached by effector485 and through Effector, from a Windows, a Linux and a MIND Core host.
- **The parts it needs:**
  - `LNK`: [800-LNK-0041](800-LNK-0041-usb-serial-cp210x-pl2303-and-several-converters.md), [800-LNK-0042](800-LNK-0042-mind-core-as-the-master-of-a-serial-line.md), [800-LNK-0043](800-LNK-0043-effector485-a-line-without-link.md), the rest of [100000-LNK-0038](100000-LNK-0038-link-over-the-boards-console-uart.md);
  - `EFF`: the Effector project's answer to its task 108 ([100000-EFF-0026](100000-EFF-0026-serial-ports-through-effector.md)), and MIND Core's `effector` as a serial host, the specification's stage S4, now asked for (message `EFF/2026-11-10T…-MOB-…`).

### Stage 3: the boards as machines

- [100004-MOB-0005](100004-MOB-0005-the-allwinner-mmc-controller.md): the Allwinner MMC controller, the microSD on both boards and the H5's eMMC. Without it a board has no disk once the loader has run.
- [100004-MOB-0008](100004-MOB-0008-memory-and-ipc-on-the-boards.md): the existing suites on the boards; MIND Core in 512 MiB and in 2 GiB.
- [100004-MOB-0006](100004-MOB-0006-i2c-and-the-h5s-sensors.md): the I2C controller (TWI) and sensors on the H5's header, a stand-in for the phone's sensor bus.
- [100004-MOB-0007](100004-MOB-0007-the-axp313a-pmic.md): the AXP313a PMIC, with an interface ready for the phone's AXP803.
- **Later, when a driver needs it:**
  - the clock and reset controller as a service ([100000](100000-a-phone-the-pinephone-first.md), stage 2);
  - `usb_host` on the EHCI and OHCI in memory, a `DRV` task, for the boards' USB ports and then the phone's modem ([100001](100001-telephony-sms-and-calls-through-the-modem.md)).

### What this main task does not do

- **The modem on the H618's USB:** [100001](100001-telephony-sms-and-calls-through-the-modem.md), after `usb_host` in memory.
- **The Wi-Fi chips:** their firmware is proprietary, so it goes only through `proprietary/` (AGENTS.md, section 3), and their drivers are `DRV` tasks.

## Acceptance criteria

- **Each board:**
  - boots MIND Core from its card to `link` on its UART0, at 115200;
  - is reached by effector485 there (`hello`, `log`, `put`, `run`) from a Windows or Linux host, and from a MIND Core host;
  - reads and writes its microSD through our MMC driver;
  - has its configuration recorded (the board's revision, U-Boot's and TF-A's versions, the adapter, the host).
- **The evidence** is written for each board, not carried to the PinePhone (MC-12.1).

## Related

[100000](100000-a-phone-the-pinephone-first.md), [205](205-aarch64-boards.md), [docs/remote-serial.md](../docs/remote-serial.md), [docs/link/effector485.md](../docs/link/effector485.md), [docs/profile/aarch64](../docs/profile/aarch64/README.md).
