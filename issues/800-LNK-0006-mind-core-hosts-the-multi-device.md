# 800-LNK-0006 — MIND Core with the multi-device: its CDC-ACM port and fast channel, and MIND Core driving another machine

**Type:** drivers and service · **Owner:** `LNK` (with `DRV` for `usb_host`) · **Priority:** P2 · **Status:** open · **Blocked by:** [800-LNK-0005](800-LNK-0005-the-rp2-multi-device-firmware.md); `KRN`: `init`'s grants · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-2.3, MC-2.4, MC-6.1

## Problem

On a MIND Core machine the multi-device appears as a CDC-ACM port, a vendor bulk interface and a HID keyboard and mouse. The HID part already works through `usb_hid`. The other two have no driver, and MIND Core has no way to drive the far machine's keyboard and mouse.

## Plan

- **A CDC-ACM class driver.** It serves the same `serial.wit` as `usb_serial` (800-LNK-0002), so `link` takes either. It also serves the indicator board of issues-human section 8.
- **The fast channel:** the vendor bulk interface as a `link` transport, at about 1 MB/s.
- **MIND Core as master.** `link` and a shell command, `link drive`, send keyboard and mouse events (channel 4) to the far machine, and files and console bytes as `mindlink` does.

## Acceptance criteria

- **MIND Core to Linux, through the multi-device:**
  - a file each way at about 1 MB/s;
  - the Linux machine's pointer moved and keys typed from MIND Core.
- **Linux to MIND Core:** the same, MIND Core's pointer driven from Linux.

## Related

[800](800-mind-link.md), [800-LNK-0003](800-LNK-0003-the-link-service-on-mind-core.md), `usb_hid`.
