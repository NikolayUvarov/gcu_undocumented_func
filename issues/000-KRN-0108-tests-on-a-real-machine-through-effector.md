# 000-KRN-0108 — Tests on a real machine through Effector: a keyboard and mouse emulator, video capture, power

**Type:** test harness · **Owner:** kernel session · **Priority:** P1 · **Status:** open (waiting for the rig: [issues-human](../issues-human/README.md) §10) · **Blocked by:** issues-human §10 · **Main task:** — · **Constitution:** MC-12.1, MC-12.2, MC-12.9

## Problem

Every run on real hardware needs the maintainer: to write the stick, boot the machine, type, and photograph the screen (211, 213, 214, the MacBook Pro's runs). One run takes hours to come back, and an agent cannot try the next step on its own.

The maintainer's offer (2026-10-14):
- Effector drives a device that emulates a keyboard and a mouse;
- a video capture module shows it the machine's screen.

With both, an agent can run a test on a real machine as the QEMU suites do: type, point, and read the screen.

## Plan

1. **What the rig gives, through Effector operations** (the maintainer's side; the names are a proposal):
   - **Input:**
     - key presses by name, with modifiers, as QEMU's `sendkey` names them (`ctrl-alt-f5`, `alt-w`), and text typed at a set pace;
     - the mouse: relative moves, an absolute position if the device offers a tablet mode, buttons and the wheel.
     - The device should be a USB HID boot-protocol keyboard and mouse, so the firmware's menus (setup, the boot menu) take it too.
   - **The screen:** one frame on request, lossless (PNG), at the capture's native size, with its time. A short clip for a moving check.
   - **Power** (needed for a run with no one there): power on, a hard reset and a forced off. Each goes through a relay on the front-panel header or a switched socket, and the operation says which it did. Without it, a hung machine waits for a person.
   - **The boot medium** (wanted): a USB disk the rig writes with the image under test, then hands to the machine. Examples are an SD/USB switch, or the stick written on the rig's host and switched across.
   - **The serial line**, where the machine has COM1 or a USB-UART: its lines, which carry the kernel's log.
2. **A machine of the rig is a test machine, not an Effector work machine.** Rebooting and powering it off are its purpose. The rules of [docs/effector.md](../docs/effector.md) (no reboots, no network changes) stay with the work machines. docs/effector.md gets a section for the rig:
   - who may use it, one test at a time;
   - what each operation does;
   - the proprietary firmware stays on the test disk only (AGENTS.md, section 3).
3. **The harness** (this track), `tests/hw_rig.py`, reusing `qemu_smoke.py`'s parts:
   - `keys()` and typing through the rig's input operations.
   - Reading the screen with `screen_text`. It already reads text by matching MIND Mono 16, the font every MIND screen draws with (`fonts/mind-mono-16.bdf`), so no OCR model is needed. A capture that scales or compresses gets a tolerance per cell, measured on the rig.
   - Waiting for a text on screen, or on the serial line, with a deadline. Then power off or reset the machine and say so.
   - Every frame it read is kept with the run's record.
4. **The first suite on the rig:**
   - boot the image under test;
   - wait for the shell's prompt on screen;
   - run `check` and read its table;
   - run the quick hardware script (`diag/quick.msh`, 176-KRN-0087) and keep what it collects;
   - power off.
5. **Then the machine-specific checks** that now wait for the maintainer:
   - the trackpad and the keyboard (211-DRV-0018);
   - the screen (213-KRN-0098);
   - sound by the rig's microphone, if it has one;
   - a watchdog reset (000-KRN-0076): the machine must restart by itself.
6. **Evidence.** A run on the rig is evidence for that machine and that image, named with both (MC-12.1, MC-12.9), never for another.

## Acceptance criteria

- [ ] With no person present, an agent boots the test machine from the image under test, reads the shell's prompt from a captured frame, types `check`, and reads every row of its table.
- [ ] A hung machine is reset by the rig, and the next run starts clean.
- [ ] The first suite's record (frames, table, serial log) is kept with the run, and docs/profile names the machine where it cites it.

## Related

[issues-human](../issues-human/README.md) §10 (the rig), §8 (the RP2040 indicator); `screen_text` in [tests/qemu_smoke.py](../tests/qemu_smoke.py); [docs/effector.md](../docs/effector.md); 211, 213, 214 (runs on real machines).
