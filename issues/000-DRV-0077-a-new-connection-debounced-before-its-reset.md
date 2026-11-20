# 000-DRV-0077 — `usb_host`: a new connection held for 100 ms before its port is reset

**Type:** driver (`usb_host`) · **Owner:** `DRV` · **Priority:** P2 · **Status:** in progress (2026-11-20) · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-6.1, MC-12.1

## Problem

`usb_host` reset a root port in the same scan that first saw its connection. A device that had only just powered up got its port reset and SET_ADDRESS at once. USB 2.0 §7.1.7.3 asks the host to wait an attach debounce (TATTDB, 100 ms) of stable connection first; Linux's `hub_port_debounce` waits 100 ms, looking every 25 ms, for at most 1.5 s. A hub port had the same gap.

**Where it showed** (the STM-Speaker agent's reading of DRV's Ryzen logs, 2026-11-20):
- the speakerphone's board was short of power in those runs (the maintainer: its backlight flickered, fed from a hub);
- its CN13 went and came back within seconds while being set up (6237.266 connected, 6238.036 gone, 6241.798 connected; 7000.811, 7004.718, 7006.969);
- its ST-LINK, which switches the board's power once configured, failed its first SET_CONFIGURATION (completion 4) twice.

A debounce does not cure a device that is short of power. It keeps a connection that is still settling from being addressed, as other hosts do.

## Plan

- **Root ports.** `Xhci::debounced(port)`: the connection must hold 100 ms on end, looked at every 25 ms, within 1.5 s. A new connection (its change bit) is reset only after that. The flaps it rode out are acknowledged, so they are not taken for another connection.
- **Hub ports.** `Controller::hub_debounced` does the same through the hub's port status, clearing a connection change seen meanwhile.

## Acceptance criteria

- [ ] Builds. The QEMU suites with USB devices (`usb`, `ehci`, `files`' storage, `usbaudio`, the link's converter) pass. Their devices come up 100 ms later each.
- [ ] On the Ryzen: the devices at boot and a device plugged in are set up as before.

## Related

[000-DRV-0075](../issues-done/000-DRV-0075-a-controller-that-stops-answering.done) (a controller that stops answering), [000-DRV-0073](000-DRV-0073-usb-audio-class-speakerphone.md) (the speakerphone).
