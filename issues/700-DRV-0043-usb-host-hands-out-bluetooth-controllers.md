# 700-DRV-0043 — `usb_host` hands a Bluetooth controller's HCI interface to `bt_hci`

**Type:** driver · **Owner:** `DRV` · **Priority:** P1 (the maintainer's order for the DEXP, 2026-10-27) · **Status:** in progress (2026-10-28) · **Blocked by:** — · **Main task:** [700](700-bluetooth.md) · **Constitution:** MC-3.3, MC-12.4

## Problem

The DEXP's Bluetooth controller is the RTL8821CE's second half, `0BDA:C821`, at full speed on its xHCI. `usb_host` enumerates it and hands it to no class driver.

`bt_hci` (700-BLT-0001) needs three things for it:
- the HCI interface (class 0xE0, subclass 1, protocol 1);
- its HCI commands. The USB transport sends them to the device as a whole, as class requests (request type 0x20, request 0), and `usb_host` refuses every request to the device but a descriptor's;
- its ACL data, as packets on the bulk IN endpoint.

## Plan

1. **`libmind/src/usb.rs`:** `BADGE_BLUETOOTH` (5) and `is_bluetooth`; `claims` gives a client with that badge the HCI interfaces and nothing else.
2. **`usb_host`, for an HCI interface:**
   - its endpoints are configured with the served classes';
   - its bulk IN ring is polled for `reports`, as a converter's is (800-DRV-0034);
   - `allowed` takes the HCI command, host to device, from the client of that interface alone.
3. **`idl/usb.wit` 1.4** says so; the wire is unchanged.
4. **`KRN`:** init's grant of such a client to `bt_hci` (asked with the boot image, 2026-10-28).

## Progress

- **2026-10-28:** written.
  - `usb_host` builds;
  - the `usb_hub`, `hid` and `serial` host tests pass;
  - `libmind/src/idl/usb.rs` is regenerated.

  EHCI's path is unchanged: the MacBook's controller, behind EHCI, waits for its own step.

## Acceptance criteria

- [ ] On the DEXP, with `bt_hci`:
  - the HCI interface is claimed;
  - HCI Reset and Read Local Version Information are answered;
  - an event spanning several interrupt packets arrives whole.

  Evidence for the DEXP only (MC-12.1).
- [ ] QEMU's suites that start `usb_host` (`usb`, `keys`, `link`) pass as before. QEMU has no Bluetooth controller (700).

## Related

[700](700-bluetooth.md), `700-BLT-0001` (`bt_hci`), [800-DRV-0034](800-DRV-0034-usb-host-serves-serial-converters.md).
