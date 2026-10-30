# 800-LNK-0032 — Keys and the pointer over the link: INPUT (channel 4) in the developer mode

**Type:** protocol, service, tool · **Owner:** `LNK` · **Priority:** P1 · **Status:** open (2026-10-30) · **Blocked by:** `KRN`: the input capability for `link` in the developer mode only (asked on `internal-dev-messages`, 2026-10-30) · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-3.8, MC-10.2, MC-12.1

**Asked by:** the tools track (`APP`) in `issues/requests-LNK.md`, 2026-10-30, section 3, "keys over the link". The maintainer (2026-10-30): "do it, it was a requirement" ([000-KRN-0108](000-KRN-0108-tests-on-a-real-machine-through-effector.md): key presses by name, as QEMU's `sendkey` names them, and text at a set pace).

## Problem

- **What the checks need.** Some of APP's checks on the MacBook Pro need keys pressed in `wm`:
  - Alt+0 on a window (211-APP-0046, 158-APP-0043);
  - the clock and keyboard pages of `wm`'s Settings (000-APP-0055, 000-APP-0048);
  - Ctrl+Alt+F6 (000-APP-0089).
- **The QEMU suites do this with `sendkey`;** on a real machine only a person can.
- **What the link has today:** no operation gives a key press. Its channel 4, `INPUT` (40), is reserved, and its payload was left to the multi-device (800-LNK-0005).
- **Why only the link can do it today.** 000-KRN-0108's rig, a USB keyboard and mouse driven by Effector, would reach the firmware's menus too, but it waits for hardware (issues-human §10). The link is on the Mac and the DEXP now.
- **What the kernel allows.** Input enters only through `SYSCALL_INPUT_EVENT`, from a holder of the input capability: the keyboard driver, and the shell for the UART. `link` holds none.

## Plan

1. **The payload of `INPUT` (40)**, in `docs/link/protocol.md` (EN, RU). The code was reserved, and a slave without it answers UNSUPPORTED, as before.
   - Up to 64 events a frame, each one input word as `common/abi.rs` `input_event` builds it (key, modifiers, pressed, character), 8 bytes little-endian.
   - The slave posts them in order and answers OK with the number posted.
   - A pointer event as the kernel's pointer words, once the slave holds pointer input.
2. **`link` on MIND Core** posts them through `SYSCALL_INPUT_EVENT`, only while it holds the input capability.
   - **That capability,** as KRN grants it: from `init`, in the developer mode (`dev`) only. Without it the frame is refused DENIED.
   - **One line per frame** in the system log: `[LINK] INPUT: N EVENTS FROM THE MASTER`. The log shows that the input came over the cable (MC-10.2).
3. **The master's side:**
   - `mindlink key ctrl-alt-f6 alt-0 …`: QEMU's `sendkey` names, each a press and a release with its modifiers;
   - `mindlink type "text"` at a set pace (default 20 ms a key);
   - the same as `effector485` operations `key` and `type`, under its reservations.
4. **The tests:**
   - host tests of the payload and the names;
   - the QEMU `link` suite: keys sent over the link reach `wm` and the shell as `sendkey`'s do; without `dev`, the frame is refused DENIED and nothing reaches a program.
5. **On the MacBook Pro:** APP's five checks above, outputs to APP (MC-12.1: evidence for that machine and image only).

**What stays 000-KRN-0108's:** the firmware's menus, the boot, and a hung machine. The link reaches only a running MIND Core.

## Acceptance criteria

- [ ] `protocol.md` (EN, RU) defines `INPUT`'s payload; host tests cover it and the key names.
- [ ] The QEMU `link` suite on x86 and aarch64: keys over the link reach `wm` and the shell; refused without `dev`.
- [ ] `mindlink key`, `mindlink type` and `effector485`'s `key` and `type` are documented in `mindlink.md` and `effector485.md` (EN, RU).
- [ ] On the MacBook Pro: Ctrl+Alt+F6 and Alt+0 sent over the cable, with their effect on the screen and in the log.

## Related

[800-LNK-0001](../issues-done/800-LNK-0001-the-link-protocol.done) (channel 4 reserved), [800-LNK-0005](800-LNK-0005-the-rp2-multi-device-firmware.md) (the multi-device's HID), [000-KRN-0108](000-KRN-0108-tests-on-a-real-machine-through-effector.md) (the rig), [000-LNK-0031](000-LNK-0031-apps-checks-on-the-macbook-pro-over-the-cable.md).
