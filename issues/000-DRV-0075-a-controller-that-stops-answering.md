# 000-DRV-0075 — `usb_host`: an xHCI that stops answering is set up again, and the device it stopped at is left alone

**Type:** driver (`usb_host`) · **Owner:** `DRV` · **Priority:** P1 · **Status:** in progress (2026-11-20: done on `claude/000-DRV-0073-usb-audio`, passes in QEMU; not yet seen on the Ryzen) · **Blocked by:** — · **Roadmap:** A · **Constitution:** MC-6.1, MC-11.4, MC-12.1

## Problem

**On the Ryzen, 2026-11-20** (release 202611200150, ABI 34), the maintainer plugged the STM-Speaker speakerphone (`CAFE:4014`, full speed) straight into root port 1 of the first AMD xHCI (`1022:1639`, PCI 05:00.3). Camera 1 had been on that port. The boot stick (`152D:2329`, port 4) and the Bluetooth (`0BDA:B85B`, port 3) are on the same controller. The controller stopped answering commands at the speakerphone's set-up:

```
7469.967 usb_host: [USB] PORT 1: CONNECTED (PORTSC 000006E1)
7536.082 usb_host: [USB] PORT 1: DEVICE NOT SET UP AT ADDRESS DEVICE (COMPLETION 0, PORTSC 00000603, TRY 1 OF 3)
7569.101 usb_host: [USB] PORT 1: DEVICE NOT SET UP AT ENABLE SLOT (COMPLETION 0, PORTSC 00000603, TRY 2 OF 3)
7602.120 usb_host: [USB] PORT 1: DEVICE NOT SET UP AT ENABLE SLOT (COMPLETION 0, PORTSC 00000603, TRY 3 OF 3)
```

What this did:
- Every command waited 30 s, and `usb_host` serves every controller from one loop. Each wait stopped the keyboards and the link's converter on the second controller.
- Nothing set the controller up again. The stick and the Bluetooth stayed out of reach until a restart.

Linux 7.0 on the same machine fails the same way, twice: once at boot, and once after a PCI remove and rescan reset the controller.

```
[   95.716307] usb 1-1: new full-speed USB device number 2 using xhci_hcd
[  106.349937] xhci_hcd 0000:05:00.3: xHCI host not responding to stop endpoint command
[  106.365764] xhci_hcd 0000:05:00.3: xHCI host controller not responding, assume dead
```

The STM-Speaker project's own measurements show the same controller and device failing under Windows. See its `docs/measurements/2026-10-01-remote-delivery/usb-playback-stall.md` and `winmm-controller-stall.md`. There, a transaction error on the feedback endpoint was followed by a command timeout, an abort that also timed out, and an internal controller reset on PCI 5:0:4. So the failure belongs to this device (or its cable) on this controller, not to one system's driver. A bad device must still not take the boot stick with it (MC-6.1).

## Plan

1. **Command wait.** A command waits 5 s, as Linux's does, instead of 30 s. One without an answer marks the controller stuck and is logged with USBSTS and CRCR. From then on, no command waits on that controller.
2. **Scans.** The scan of a stuck controller stops.
3. **Set-up again.** The main loop sets a stuck controller up again, as it does one that halted with an error (000-DRV-0073). A controller that does not halt is reset all the same.
4. **The device it stopped at.** That device, found by its root port and route, is left alone across the new set-up until it is unplugged. The rest of the controller's devices come back.
5. **Streams.** A controller that was streaming when it got stuck takes no stream afterwards.
6. **Giving up.** A controller set up again 8 times in a row, each within 10 minutes of the one before, is left off.

## Acceptance criteria

- [x] Builds. The QEMU suites `usb`, `ehci`, `usbaudio`, `services` and `files` pass: the paths that work are unchanged.
- [ ] On the Ryzen with the speakerphone in 05:00.3 port 1, the log shows that the command had no answer, then that the controller was set up again and its device left alone. The boot stick and the Bluetooth come back.
- [ ] Once the speakerphone is unplugged there, the log says the device left alone is gone. The port works again (camera 1).

## Related

- [000-DRV-0073](000-DRV-0073-usb-audio-class-speakerphone.md): the speakerphone, and the recovery from a controller that halted with an error.
- STM-Speaker `docs/windows-delivery-2026-10-01.md`, section "Reproduced USB failure and limits".
