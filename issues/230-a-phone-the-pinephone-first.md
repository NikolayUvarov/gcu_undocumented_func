# 230 — MIND Core on a phone: the PinePhone first

**Type:** porting (main task) · **Owner:** `MOB` track (open; the kernel session works stage 0 as `PRT` and `KRN` tasks) · **Priority:** P2 · **Status:** open · **Blocked by:** stage by stage, below; stage 1 needs a PinePhone at hand ([issues-human, section 12](../issues-human/README.md#12-a-pinephone-to-test-on)) · **Roadmap:** track H (boards with UEFI) · **Constitution:** MC-1.3, MC-1.5, MC-5.1, MC-6.3, MC-11.3, MC-11.4, MC-11.11, MC-12.1, MC-12.3

**Recorded by:** the kernel session, 2026-10-25, at the maintainer's request:
- the question: what MIND Core needs to run on mobile phones, and which phone to start with;
- then: "make the track".

## Problem

What decides whether MIND Core can run on a phone is its boot chain, more than its processor.

- **Closed boot chains:** an iPhone and a Samsung phone boot only what their maker signed, as an Apple Silicon Mac does (210 is parked for that reason).
- **An Android phone on Qualcomm:**
  - its loader boots a `boot.img` with a kernel and a device tree, with no UEFI and no ACPI;
  - its modem is inside the SoC, with closed firmware and memory it shares with the application processor.

  It can be done, but it is a long way.
- **The aarch64 port today** (docs/profile/aarch64):
  - it boots through UEFI and reads the board from ACPI;
  - with only a device tree it boots with `virt`'s defaults (210-KRN-0029);
  - its platform devices are the UART and the RTC, by a fixed index.

## Why the PinePhone first

As found on 2026-10-25 (sources at the end). Each point is checked on the phone before a profile states it (MC-12.1, 12.3).

- **The machine:** Allwinner A64, four Cortex-A53, 2 or 3 GiB, a GIC-400.
- **It is on sale.** In August 2025 Pine64 said it would keep it in production for about two more years. The PinePhone Pro was discontinued then.
- **It boots from microSD before its eMMC.** A card that does not boot leaves the phone as it was.
- **Tow-Boot,** a distribution of U-Boot, gives it UEFI and a device tree. MIND Core's aarch64 bootloader is a UEFI application, and the device tree reaches the kernel already (`BootInfo.device_tree`).
- **A serial console through the headphone jack:** a switch inside the phone, 115200 8N1.
- **The modem is a separate computer on USB** (Quectel EG25-G), driven by AT commands.
- **Kill switches** for the modem, Wi-Fi and Bluetooth, the microphone and the cameras.
- **The GIC-400 is a GICv2,** which the kernel drives already (issue 205's `gic-version=2` group). PSCI comes from TF-A inside Tow-Boot.
- **Its limits, stated:**
  - no IOMMU: drivers with DMA are in the TCB, as on x86 (MC-1.5);
  - 2–3 GiB and slow cores, so little room for large models.

## Plan

### Stage 0: without a phone, in QEMU and on the host

- [230-PRT-0009](230-PRT-0009-the-board-from-the-device-tree.md): the board from the device tree.
- [230-KRN-0171](230-KRN-0171-platform-devices-from-the-device-tree.md): platform devices from the device tree (an ABI change).
- [230-PRT-0010](230-PRT-0010-a-16550-uart-in-memory.md): a 16550 UART in memory as the console.
- The phone's device tree from Linux (`sun50i-a64-pinephone-1.2.dts`) for host tests, with its licence checked and recorded in THIRD_PARTY.md.

### Stage 1: the first boot on the phone

- **Tow-Boot** is installed once on the phone, by its own installer (a person, issues-human section 12).
- **Our card** holds only the ESP (the bootloader and the slots) and `MIND LOG`, written by `make_usb_image.py`.
  - The A64's boot ROM reads its loader from 8 KiB into the card. Whether our layout must leave that free when Tow-Boot is on the eMMC is checked first.
- **What runs:**
  - the kernel's lines and the shell on the serial console;
  - every CPU through PSCI, and the timer;
  - RAM from the UEFI map;
  - reset and power off through PSCI.

### Stage 2: the phone as a machine

- **The screen:**
  - the GOP, if Tow-Boot sets the panel up (to be checked);
  - otherwise a display driver (the DE2 engine, the DSI host, the ST7703 panel) and the backlight's PWM.
- **Input:**
  - the touchscreen (Goodix GT917S over I2C) as an absolute pointer, as `virtio-tablet` is;
  - the volume keys (LRADC);
  - the power key (the PMIC).
- **The PMIC** (X-Powers AXP803 over RSB): the power key, the battery, charging, and the regulators the peripherals need.
- **Storage:**
  - the microSD and the eMMC, through the A64's MMC controller;
  - the block store's own partition, as proposed to `STO` on 2026-10-25 (its main task 307, proposed).
- **USB:** the A64's EHCI and OHCI in memory, a `usb_host` task of `DRV`; the OTG port later.
- **Clocks and resets:** the A64's clock controller, as a service the other drivers ask. The kernel hands out no clocks.

### Stage 3: radios

- **The modem:** main task [231](231-telephony-sms-and-calls-through-the-modem.md).
- **Wi-Fi and Bluetooth** (RTL8723CS over SDIO):
  - SDIO in the MMC driver;
  - the Wi-Fi driver is a `DRV` task, and Bluetooth goes to `BLT`;
  - its firmware only in `proprietary/` (AGENTS.md, section 3).

### Stage 4: on a battery

Main task [232](232-power-on-a-battery-idle-frequency-and-sleep.md).

### Stage 5: the interface

- The shell and `wm` at 720 × 1440, scaled.
- An on-screen keyboard (a request to `APP`).
- Rotation by the accelerometer, later.

### Stage 6: the platform profile `aarch64/PinePhone-0`

- **Its TCB:**
  - no IOMMU;
  - Tow-Boot and TF-A as firmware;
  - the modem as an external computer on USB;
  - the kill switches as physical control.
- **Its limits and its evidence,** from this phone only (MC-12.1).

### Later: other phones (intention only)

- **Librem 5** (i.MX8MQ): a GICv3, the modem on an M.2 card, kill switches.
- **Fairphone 5** (Qualcomm QCM6490): an unlockable bootloader and active work in mainline Linux, but Qualcomm's boot chain and closed firmware for the modem and Wi-Fi.
- **Not:**
  - the iPhone and Samsung's phones (closed boot chains);
  - the PinePhone Pro (discontinued);
  - the OnePlus 6 and 6T, whose UEFI port warns that flashing it can wipe the UFS.

## Acceptance criteria

- **Stage 0:**
  - host tests of the device tree reader on the PinePhone's tree and on QEMU's;
  - the `devicetree` suite boots to the shell with the board read from the tree;
  - the 16550 code tested on the host.
- **Stage 1,** on a PinePhone:
  - the boot log and the shell on the serial console;
  - all four CPUs;
  - a reboot;
  - the phone's model, its memory and Tow-Boot's version recorded (MC-12.1).
- **Stage 2:**
  - the screen shows the shell, and touch moves the pointer;
  - a file written to the card is read back after a reboot;
  - the battery's level is shown.
- **Stages 3 to 6:** as 231, 232 and the profile say.

## Related

[205](205-aarch64-boards.md), [210-APL-0002](210-APL-0002-board-from-the-device-tree.md), [231](231-telephony-sms-and-calls-through-the-modem.md), [232](232-power-on-a-battery-idle-frequency-and-sleep.md), [207](207-gpio-service.md) (`gpio`), [700](700-bluetooth.md), [docs/profile/aarch64](../docs/profile/aarch64/README.md).

Sources, read on 2026-10-25:
- [Pine64, August 2025](https://pine64.org/2025/08/14/august_2025_short_update/);
- [Liliputing on the PinePhone Pro's end](https://liliputing.com/pinephone-pro-linux-smartphone-has-been-discontinued-but-the-original-pinephone-is-still-available/);
- [postmarketOS: Tow-Boot for UEFI on the PinePhone](https://gitlab.postmarketos.org/postmarketOS/pmaports/-/merge_requests/3068/commits);
- [the serial console at 115.2 kbps](https://lupyuen.codeberg.page/articles/uboot);
- [edk2-sdm845's warning](https://github.com/wode2016501/edk2-sdm845);
- [FOSDEM 2026 on Fairphones](https://fosdem.org/2026/schedule/event/mainline_kernel_for_fairphones_-_2026_update).
