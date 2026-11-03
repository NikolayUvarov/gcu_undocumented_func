# 213-KRN-0099 — `init` grants every xHCI controller to `usb_host`

**Type:** `init`, with the drivers track's `usb_host` (213-DRV-0048) · **Owner:** kernel session · **Priority:** P1 · **Status:** in progress (in `main` c7cef477; the Ryzen machine's report is left) · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-2.3

## Problem

`init` grants `usb_host` the first xHCI controller only. The Ryzen 9 5900HX machine has two (05:00.3 and 05:00.4, 1022:1639), and what is plugged into the second's ports is not seen: its bus mastering stays off. The maintainer (2026-11-01): a keyboard and mouse receiver in a blue port does nothing.

## Plan

**One `usb_host` for every controller,** as it drives the EHCI ones: the drivers track's shape ([213-DRV-0048](../issues-done/213-DRV-0048-usb-host-drives-every-xhci.done)), taken instead of this issue's first plan of `usb_host#1` instances, so a Bluetooth adapter, a serial converter or a camera works on either controller too.

1. **`init`:** after the first controller, each further xHCI controller in PCI order gets its BAR0 and its own DMA region in `mind::usb::XHCI`'s slots, as the EHCI controllers get theirs.
2. **A driver's restart:** the second controller is stopped and resumed with the first.
3. **Tests:** the `usb` suite's keyboard on a second `qemu-xhci` (213-DRV-0048) is required, not skipped.

## Acceptance criteria

- In QEMU a keyboard on the second controller types in the shell.
- On the Ryzen machine both controllers are granted to a driver in the hardware report.

## Progress (2026-11-01)

- **First made as instances** (371de749: `usb_host#1`, `usb_storage#1`, `usb_hid#1`). Reverted (8f2c3f97) for the drivers track's shape, which came the same day with `usb_host`'s side done and the grant tested.
- **`init`** (on 213-DRV-0048's 4756d380, merged into this branch):
  - the `usb_host` arm grants each controller of `mind::usb::XHCI` its BAR0 (which turns its bus mastering on) and 512 KiB of DMA, and logs `[INIT] XHCI n: REGISTERS AT … NOT GRANTED` when the BAR cannot be;
  - the second controller is the service's second device, so `init` stops it when `usb_host` ends and resumes it on a restart, as for the first.
- **Tests:** the `usb` suite asserts `XHCI 1:` in `usb_host`'s log, then that a keyboard on the second `qemu-xhci` types, also after `usb_hid` and `usb_host` restart.
- **Docs:** README's `usb_host` row, `docs/profile/bootstrap.md`.

## Progress (2026-11-02)

- **Landed in `main` c7cef477** (round 67), with DRV's 213-DRV-0048. The first criterion is met: in the gate, the `usb` suite's keyboard on a second `qemu-xhci` typed, also after `usb_hid` and `usb_host` restarted, on x86 ("x86: keys, shell, tools", `acc-2`) and on aarch64 ("aarch64: programs, shell and four CPUs", `acc-2`). That is evidence for QEMU only (MC-12.1).
- **Left:** the second criterion. On the Ryzen machine, both controllers (05:00.3 and 05:00.4) must show "(granted to a driver)" in the hardware report of a release with c7cef477.

## Related

[213](213-amd-ryzen-pc.md), [213-DRV-0048](../issues-done/213-DRV-0048-usb-host-drives-every-xhci.done), 164 (`usb_host`), [requests-DRV.md](requests-DRV.md).
