# 213-DRV-0061 — The USB keyboard on the Ryzen 9 5900HX's xHCI types nothing

**Type:** driver · **Owner:** `DRV` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-6.1, MC-12.1

Numbered from the kernel track's request of 2026-10-13 in `requests-DRV.md`, from the maintainer's fifth boot of the machine (`log:boot0001.log`–`boot0005.log`).

## Problem

On boot 5 the shell was on the screen and typing did nothing. The logs showed:
- **The device:** 1C4F:000E, low speed, on port 4 of the first xHCI (05:00.3, AMD 1022:1639), slot 2.
- **`usb_hid`:** it took interface 0 as a boot keyboard (endpoint 81), and left interface 1 (consumer and system control, endpoint 82) alone.
- **`usb_host`:** it polls slot 2's endpoint 3 (packet 8, interval 10). On boots 1–3 it logged a first report in the boot format, for example `[00, 00, 3B, 00, 00, 00, 00, 00]` (F2).
- **Not in the logs:** whether later reports came, and whether `usb_hid` made key events of them.

**Two things may hide it:**
- the shell was then stuck reading a COM1 the machine does not have ([213-KRN-0098](213-KRN-0098-black-screen-when-the-shell-takes-it.md), fixed since);
- the same build typed with a low-speed keyboard (046D:C31C) on the Intel i5 PC's xHCI, so the difference is the AMD xHCI or this keyboard.

## Plan

1. **The maintainer's next run with this keyboard** on a build of `main`. If it types, the cause was 213-KRN-0098, and this closes.
2. **Otherwise, count it:**
   - `usb_host` logs, once a second at most, each interrupt endpoint's reports and completion codes after the first;
   - `usb_hid` logs the key events it sends.
3. **Low speed on an xHCI:**
   - check the endpoint context: the interval in 125 µs units for a low-speed device, Max ESIT Payload 8, Max Packet Size 8;
   - check the transfer queued again after each completion.
4. **The maintainer's run after that:** the same keyboard on another port and behind a hub, and another keyboard.

## Acceptance criteria

On the Ryzen this keyboard types in the shell, with the counters of plan step 2 in the log if they were needed. The run needs the maintainer at the machine.

## Related

[213](213-amd-ryzen-pc.md), [213-KRN-0098](213-KRN-0098-black-screen-when-the-shell-takes-it.md), [214](214-intel-core-i5-pc.md).
