# 700-BLT-0002 — `bt_hci`: a round every five minutes as meant, and what each round got from the controller

**Type:** driver · **Owner:** `BLT` · **Priority:** P2 · **Status:** in progress (2026-10-31) · **Blocked by:** — · **Main task:** [700](700-bluetooth.md) · **Constitution:** MC-12.1

## Problem

On the DEXP (2026-10-30), `bt_hci` (700-BLT-0001) logged a round about every 77 s, though it means one every 300 s. It heard nothing, and its log could not say why:
- **The interval.** `SYSCALL_WAIT` clamps a wait to 60 s (`kernel/src/scheduler.rs`), and `bt_hci` waited 300 s in one call. So each round came 60 s after the last one ended.
- **What the controller gave was not logged:**
  - how many events came in the LE scan and in the inquiry;
  - whether the inquiry ended, when, and with what status.

## Plan

1. **The wait:** pieces of at most 60 s until the round's time.
2. **The round's line:** the devices heard, the events in the LE scan and in the inquiry, and the event packets dropped.
3. **The inquiry's end:** `INQUIRY COMPLETE AFTER N MS, STATUS XX`, or `NO INQUIRY COMPLETE IN 12000 MS`.

## Acceptance criteria

- [ ] `tests/bt_hci_host.rs` and the x86 build pass.
- [x] On the DEXP: rounds 300 s apart, and the round's lines as above.

## Progress (2026-10-31)

**On the DEXP,** release 202610301115 (ABI 25, tree `ff0da96d`), by `effector485 replace`, unverified. The service was built from that tree with this change, since `main` is at ABI 26. The first build, from `main`, was refused (`ABI MISMATCH: PROGRAM 26 KERNEL 25`) and quarantined.

- **With `rtw88_wifi`'s Wi-Fi-only set-up,** before 700-DRV-0046:
  - `0 EVENTS IN THE LE SCAN, 1 IN THE INQUIRY`;
  - `INQUIRY COMPLETE AFTER 7048 MS, STATUS 00`.

  The controller answered its commands but heard no air.
- **With 700-DRV-0046's free run:**
  - `15 LE AND 1 BR/EDR DEVICES HEARD; 25 EVENTS IN THE LE SCAN, 69 IN THE INQUIRY; 64 EVENT PACKETS DROPPED`, then `NO INQUIRY COMPLETE IN 12000 MS`;
  - a later round: 9 LE and 1 BR/EDR (`"[TV] Samsung AU7100 65 TV"`), 209 events in the inquiry, 195 packets dropped.
- **The drops are packets lost before `bt_hci`, not its own:**
  - `usb_host` keeps 8 reports of an interrupt endpoint and drops the oldest when a ninth comes (`usb_host/src/xhci.rs`);
  - the controller sends a 16-byte packet a millisecond;
  - an extended inquiry result is 17 of them.

  A lost packet breaks the event it belongs to, and the tail then spills into the next. That is DRV's 700-DRV-0047: the Bluetooth's events kept without loss, as a converter's bulk IN already is.
- **A try that took the bytes past an event's end as the next event** (as Linux btusb does with its larger transfers) lost its alignment after the first lost packet and stalled the round (`Timeout(8204)`). It was taken back.
- **The interval:** two rounds ended at 43,641 s and 43,963 s, 322 s apart: the 300 s wait and the round itself. The second heard 15 LE and 1 BR/EDR devices and dropped 404 packets.

## Related

[700-BLT-0001](700-BLT-0001-bt-hci-the-rtl8821c-up.md); DRV's 700-DRV-0046 (`rtw88_wifi`'s free run) and 700-DRV-0047 (the events without loss), on `claude/ASR-DRV`.
