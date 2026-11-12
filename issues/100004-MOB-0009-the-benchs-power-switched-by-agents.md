# 100004-MOB-0009 — The bench's power: the boards and modules switched off and on by agents

**Type:** integration (the bench) · **Owner:** `MOB` · **Priority:** P2 · **Status:** open · **Blocked by:** the power device the maintainer is finding ([issues-human, section 14](../issues-human/README.md#14-the-two-allwinner-boards-on-the-bench)) · **Main task:** [100004](100004-two-allwinner-boards-before-the-phone.md) · **Constitution:** MC-6.1, MC-7.1, MC-7.4, MC-12.1

## Problem

**What a reboot cannot reach.** A reboot over the link (`reboot`) needs a running MIND Core that answers. These states need the power cut:
- a board stopped in U-Boot;
- a card that does not boot;
- a hung kernel;
- a wedged chip.

**What the watchdog covers.** The SoC's watchdog, asked of `PRT` on 2026-11-12, restarts a running system that froze. It does nothing for U-Boot or a bad card.

**The maintainer's word (2026-11-12):**
- the hardware watchdogs are to be used;
- the maintainer finds a device that powers the bench's devices, and every module that can be is connected to it;
- agents may then switch them.

## Plan

1. **Once the device is named:** its control (USB serial, HID or network), its channels, and which board or module is on each, recorded in 100004.
2. **One way to switch a channel from any host:**
   - **Windows and Linux:** a command or an effector485 operation, so that it runs through Effector too.
   - **MIND Core:** over `usb_serial` and `serialhost` (800-LNK-0041, 0042), if the device is on USB serial.
   - **Each switch is logged** with its reason, under the line's lease where the board's line has one (MC-7.4).
3. **A board brought back:**
   - power off;
   - a wait long enough for the board to drain, with the USB-UART adapter's TX unable to feed the board through its RX: a series resistor of about 1 kΩ, or the adapter switched too;
   - power on;
   - the boot followed on the console.

## Acceptance criteria

- **A board stopped on purpose** (in U-Boot's prompt, and with a card that does not boot) is brought back by an agent through Effector, with no person at the bench.
- **The run is recorded:** the device, the channel, the board, the host (MC-12.1).

## Related

[100004-MOB-0004](100004-MOB-0004-effector-to-the-boards-over-their-uart.md), [issues-human, section 10](../issues-human/README.md#10-a-test-rig-for-real-machines-keyboard-and-mouse-the-screen-power), [000-KRN-0076](000-KRN-0076-a-frozen-machine-restarts-and-starts-safely.md), [213-KRN-0125](213-KRN-0125-the-amd-fch-watchdog.md).
