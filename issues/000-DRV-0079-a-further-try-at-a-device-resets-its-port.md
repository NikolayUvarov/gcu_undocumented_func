# 000-DRV-0079 — `usb_host`: a further try at a root port's device resets the port, and its report names the step's own completion code

**Type:** driver (`usb_host`) · **Owner:** `DRV` · **Priority:** P2 · **Status:** in progress (2026-11-20) · **Blocked by:** — · **Roadmap:** A · **Constitution:** MC-6.1, MC-12.1

## Problem

**On the DEXP** (Intel xHCI, release 202611201739, 2026-11-20), the STM-Speaker speakerphone straight in root port 2 was not set up:
```
320.992 usb_host: [USB] PORT 2: CONNECTED (PORTSC 000006E1)
321.192 usb_host: [USB] SLOT 14 REQUEST 80 06 0200: COMPLETION 4
321.192 usb_host: [USB] PORT 2: DEVICE NOT SET UP AT CONFIGURATION DESCRIPTOR (COMPLETION 1, PORTSC 00000603, TRY 1 OF 3)
321.243 usb_host: [USB] PORT 2: DEVICE NOT SET UP AT ADDRESS DEVICE (COMPLETION 1, PORTSC 00000603, TRY 2 OF 3)
321.493 usb_host: [USB] PORT 2: DEVICE NOT SET UP AT ADDRESS DEVICE (COMPLETION 1, PORTSC 00000603, TRY 3 OF 3)
```
The device's own fault is the transaction error (completion 4) on a control transfer. Two faults of `usb_host` hid it and wasted the tries:

1. **A further try did not reset the port.** `Xhci::enable_port` reset only a port that was not enabled. After a try that failed, the port stays enabled (`PORTSC 00000603`), and the device keeps the address the first try gave it. Tries 2 and 3 then sent SET_ADDRESS to address 0, where nothing answers. Three tries were one.
2. **The report named the wrong completion code.** The slot of a failed try is given back with Disable Slot, and the report printed that command's code (1, success) in place of the failed step's.

## Plan

- **`enable_port(port, again)`.** A further try (`root_tries > 0`) resets the port even when it is enabled, and waits 50 ms after the reset before the device is addressed, as Linux's `hub_port_reset` does (10 ms of USB 2.0's recovery and 40 more). The first try keeps its 20 ms.
- **`discard`** keeps the completion code of the step that failed across its Disable Slot.

## Acceptance criteria

- [ ] Builds. The QEMU suites with USB devices pass: `x86: keys, shell, tools` (`usb`, `ehci`) and `usbaudio`.
- [ ] On the DEXP, with the speakerphone in a root port: a failed try is reported with its step's own completion code, and the tries after it start from a port reset.

## Related

[000-DRV-0077](000-DRV-0077-a-new-connection-debounced-before-its-reset.md) (the attach debounce), [000-DRV-0073](000-DRV-0073-usb-audio-class-speakerphone.md) (the speakerphone), [000-DRV-0075](../issues-done/000-DRV-0075-a-controller-that-stops-answering.done) (a controller that stops answering).
