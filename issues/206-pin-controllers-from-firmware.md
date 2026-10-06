# 206 — aarch64: pin controllers from the firmware's tables, for a user-space GPIO service

**Type:** porting (kernel platform layer, with the kernel track for `common/abi.rs`) · **Owner:** porting track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** track H · **Constitution:** MC-1.1, MC-3.1, MC-3.3, MC-8.2, MC-8.3, MC-12.1

## Problem

ARM boards bring their hardware interfaces out on the pins of the SoC's pin controller: GPIO, UART, SPI, I²C, PWM, each pin multiplexed between several functions. A user is to see these pins and drive them (the `pins` tool, tools track; the `gpio` service, [207](207-gpio-service.md)). The kernel today knows only the UART and RTC of a board (`arch/aarch64/board.rs`, issue 205); nothing tells `init` where a pin controller is, and nothing names the board, so a tool cannot find the pin tables that describe it ([hwdocs/](../hwdocs/README.md)).

The driver belongs in ring 3 (MC-1.1): the kernel's share is only to find the controller's registers in the firmware's tables and hand them to `init` as a platform resource it validated, as for the UART.

## Plan

- **Finding controllers:** `acpi.rs` scans the DSDT and SSDTs for devices whose `_HID` is a known pin controller — `BCM2845` (Raspberry Pi 4 EDK2: BCM2711 GPIO), `ARMH0061` (PL061, QEMU `virt`), more by table — and takes the register window from the device's `_CRS` (`Memory32Fixed` / QWord memory descriptors). No AML interpreter: a bounded byte scan of `Device` / `Name(_HID)` / `Name(_CRS)` packages; anything it cannot read is reported and skipped. The windows are checked against the identity map (`mapped()`) and the UEFI memory map (never RAM).
- **ABI:** `PLATFORM_GPIO_BASE` indices in `PLATFORM_MMIO` (`PLATFORM_GPIO_BASE + n` for the n-th controller, up to 4), and a `PLATFORM_INFO` query giving `init` the controller's kind (`BCM2711`, `PL061`, …), its pin count and the board identity (the FADT/DSDT OEM ID and OEM table ID, the SMBIOS board name if the loader passes it), so `gpio` and `pins` can find the board's file in `hwdocs/`. Agreed with the kernel track (ABI changes live in kernel issues); `docs/api` updated with it.
- **Boot line:** `MIND CORE KERNEL: BOARD …` names the pin controllers it found.
- **Pins the firmware uses:** the console UART's pins and the boot SD card's pins are named by the board's hwdocs file, not by the kernel; the kernel only reports which device is the console (already in the SPCR).
- **Profile:** `docs/profile/aarch64` lists the pin controller as a device whose registers `gpio` alone holds; its TCB names the DSDT scan as trusted input.

## Acceptance criteria

On QEMU `virt` the kernel reports the PL061 (where the DSDT lists it; otherwise none, and the test says so), and `init` can obtain its registers by index; a host test feeds the scanner the DSDT of QEMU `virt` and of the Raspberry Pi 4 EDK2 firmware (bytes recorded in `tests/`) and checks the windows and kinds found; an unknown or malformed table yields no controller and no crash. x86 is unchanged.

## Related

[205](205-aarch64-boards.md), [207](207-gpio-service.md), the `pins` tool ([u015](u015-pins.md)), [hwdocs/](../hwdocs/README.md), [docs/profile/aarch64](../docs/profile/aarch64/README.md).
