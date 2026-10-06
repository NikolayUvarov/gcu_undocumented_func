# 207 — `gpio`: a user-space service for the pins of ARM boards (BCM2711, PL061)

**Type:** porting (driver service) · **Owner:** porting track · **Priority:** P2 · **Status:** open (206 done; the plan's QEMU test replaced: no pin controller there) · **Blocked by:** — ([206](../issues-done/206-pin-controllers-from-firmware.done) done) · **Roadmap:** track H (with track F for physical control) · **Constitution:** MC-2.3, MC-3.1, MC-3.3, MC-3.7, MC-6.1, MC-8.2, MC-8.3, MC-10.2

## Problem

The `pins` tool (tools track) is to list a board's pins, show each pin's functions with the active one marked, and drive the signals. Something must own the pin controller's registers, decode them into functions, and enforce who may change what: driving a pin is a physical action (Article 8), and some pins carry the console, the SD card or the network and must not be changed by accident.

## Plan

- **Service `gpio`** (ring 3, like the other drivers): `init` grants it the registers of each controller found by 206 (`PLATFORM_MMIO` with `PLATFORM_PINS_PL061 + n` or `PLATFORM_PINS_BCM2711 + n`: the index names the kind); it exits when there is none. On x86 it is not started.
- **Controller drivers:**
  - BCM2711 (Raspberry Pi 4): 58 pins; function select (`GPFSELn`, 3 bits: input, output, ALT0–ALT5), levels (`GPLEVn`), set/clear (`GPSETn`/`GPCLRn`), pulls (`GPIO_PUP_PDN_CNTRL_REGn`).
  - PL061 (QEMU `virt`, others): 8 pins, direction and data, no multiplexing (each pin's only functions are input and output).
  A controller driver says per pin: the function numbers it has, which is active, direction, level, pull. It knows numbers only (ALT3), not names.
- **Interface `idl/gpio.wit` 1.0:** `controllers()` (kind, pins, board id); `pins(controller)` → list of {pin, function, functions, direction, level, pull, reserved}; `read(controller, pin)`; and the changing calls `set-function`, `set-direction`, `write`, `set-pull`. Errors: no such pin, reserved, not permitted, unsupported.
- **Authority (MC-3, MC-8.3):** reading is open to any client holding the endpoint; changing needs the control badge, which `init` gives only to the clients its policy names (the shell's `pins` launch path asks the operator first). A badge may cover a pin mask: a program can be granted pins 17–27 and nothing else. Changing the *limits* (which pins are reserved, which masks exist) is separate from driving pins and stays with `init`'s policy.
- **Reserved pins:** pins the board's hwdocs file marks as used by the platform (console UART, SD card, Ethernet PHY) are refused for change unless the policy grants them explicitly. `gpio` reads the board file from `/hwdocs` when the volume has it; without it, no pin is reserved beyond the console UART's, and the tool says so.
- **Log:** every change goes to `logd` (who, pin, old → new), MC-10.2.
- **Failure:** on restart `gpio` reads the hardware state again and does not reset pins (the physics decides the safe state, MC-8.4); it never drives a pin on its own.
- **hwdocs data:** `hwdocs/socs/bcm2711.pins` (the alternate-function table of the BCM2711 datasheet, with its source recorded) and `hwdocs/boards/rpi4b.board` (the 40-pin header, reserved pins), in the format of [hwdocs/README.md](../hwdocs/README.md); `hwdocs/socs/pl061.pins` for QEMU.

## Acceptance criteria

Host tests run the BCM2711 and PL061 drivers against register models (function select of every pin, set/clear, pulls), the badge and pin-mask policy, and the hwdocs parser against the files. QEMU cannot give an end-to-end run: `virt` with ACPI has no pin controller (206), so the aarch64 suite checks that `gpio` does not start there and that `pins` reports no controller. On a Raspberry Pi 4 with EDK2 (with 205): the 58 pins are listed with their functions (UART0 on 14/15 active), and an LED on a header pin follows `write`; the aarch64 profile records what was tested on hardware.

## Related

[206](../issues-done/206-pin-controllers-from-firmware.done), [205](205-aarch64-boards.md), the `pins` tool ([u015](u015-pins.md)), [hwdocs/](../hwdocs/README.md).
