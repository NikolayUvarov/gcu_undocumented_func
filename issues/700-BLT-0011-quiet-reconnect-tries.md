# 700-BLT-0011 — A device asleep all night is looked for quietly: its tries summed up, not three lines each

**Type:** driver (`bt_host`, `bt_hci`) · **Owner:** `BLT` · **Priority:** P2 · **Status:** open (2026-11-28) · **Blocked by:** — · **Main task:** [700](700-bluetooth.md) · **Constitution:** MC-6.1, MC-12.1

## Problem

On the Ryzen, the night of 2026-11-27 to 28 (release 202611271851, boot 118, up 10.4 hours), the MX Vertical mouse
(LE HID, listed) was asleep. `bt_hci` looked for it all night, as it should: an LE connection waited for 30 s, cancelled,
tried again 2 s later. Each try put three lines in the system log:

```
[BT] C4:E7:E1:93:0C:C4: CONNECTING OVER LE (RANDOM ADDRESS)
[BT] C4:E7:E1:93:0C:C4: NOT ADVERTISING: CANCELLED
[BT] C4:E7:E1:93:0C:C4: NOT CONNECTED (STATUS 04), TRIED AGAIN IN 2 S
```

That is about 340 lines an hour. In the ring read at 05:18 UTC they were 801 of 2 048 lines. The ring covered only the
last 2.5 hours, so the night's other lines were gone. A classic device switched off (the speaker) does the same, at
three lines a minute once its wait reaches 60 s.

## Plan

- The tries themselves are unchanged. A device that wakes is caught by the pending connection, so it must still be
  looked for all the time.
- The first `LOUD_TRIES` (3) failures in a row are said one by one, as before. The third says that the next ones are
  summed up.
- After that the tries are silent. Every `QUIET_LINE_MS` (10 min) one line says how many failed since the last such line,
  the last status, the failures in a row and the wait.
- When the device connects after the quiet stretch, a line says after how many failed tries. Its count starts again
  from none.

## Acceptance criteria

- [ ] Host case (`bt_host/tests/le.rs`): an LE mouse asleep for 6.2 hours. Every try is made. Three tries are said one by
      one, then one line each 10 min. The night's lines number under 60. When it wakes, the line says after how many
      tries, and its counts are reset.
- [ ] Host case: a device that fails twice and then connects logs exactly as before.
- [ ] The case fails with the quieting removed (a mutation check).
- [ ] `bt_host`'s whole suite, and `bt_hci`'s host and QEMU suites, pass.
- [ ] On the Ryzen (MC-12.1): a night with the mouse asleep; the ring holds the summing lines only, and the mouse is
      caught when it is clicked.

## Related

- [700-BLT-0009](../issues-done/700-BLT-0009-le-pairing-and-hid-over-gatt.done) (the LE central and its reconnect).
- [213-DRV-0113](213-DRV-0113-display-both-hdmi-sockets-lit-at-once.md): the release whose night showed this.
