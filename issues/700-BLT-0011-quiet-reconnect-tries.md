# 700-BLT-0011 — A device asleep all night is looked for quietly: its tries summed up, not three lines each

**Type:** driver (`bt_host`, `bt_hci`) · **Owner:** `BLT` · **Priority:** P2 · **Status:** open (2026-11-28: in `main` with round 183, c2318c10b; one night on the Ryzen left) · **Blocked by:** — · **Main task:** [700](700-bluetooth.md) · **Constitution:** MC-6.1, MC-12.1

## Problem

On the Ryzen, the night of 2026-11-27 to 28 (release 202611271851, boot 118, up 10.4 hours), the MX Vertical mouse
(LE HID, listed) was asleep. `bt_hci` looked for it all night, as it should: an LE connection waited for 30 s, cancelled,
tried again 2 s later. Each try put three lines in the system log:

```
[BT] C4:E7:E1:93:0C:C4: CONNECTING OVER LE (RANDOM ADDRESS)
[BT] C4:E7:E1:93:0C:C4: NOT ADVERTISING: CANCELLED
[BT] C4:E7:E1:93:0C:C4: NOT CONNECTED (STATUS 04), TRIED AGAIN IN 2 S
```

That is about 340 lines an hour. In the ring read at 05:18 UTC they were 801 of 2 048 lines, so the ring covered only
the last 2.5 hours. The boot's file on the disk keeps every line. But a reader of the ring (the server's `ring` log, the
cable) saw nothing of the night before that, and the file grows by some 30 KB an hour for nothing. A classic device
switched off (the speaker) does the same, at three lines a minute once its wait reaches 60 s.

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

- [x] Host case (`bt_host/tests/le.rs`): an LE mouse asleep for 6.2 hours. Every try is made. Three tries are said one by
      one, then one line each 10 min. The night's lines number under 60. When it wakes, the line says after how many
      tries, and its counts are reset.
- [x] Host case: a device that fails twice and then connects logs exactly as before.
- [x] The case fails with the quieting removed (a mutation check).
- [x] `bt_host`'s whole suite, and `bt_hci`'s host and QEMU suites, pass.
- [ ] On the Ryzen (MC-12.1): a night with the mouse asleep; the ring holds the summing lines only, and the mouse is
      caught when it is clicked.

## Progress (2026-11-28)

On `claude/700-BLT-0011-quiet-reconnects` 9431db890 (main e1dfbfebf merged), on LEG9:
- `bt_host`'s cargo suites all pass, `le.rs` 6 of 6 with the two new cases;
- with the quieting removed (`loud` always true), the night's case fails;
- `02_build.sh` passes;
- all of `scripts/host_tests.sh` exits 0: 282 `test result: ok`, none failed;
- "build (x86 test programs)" PASS 160 s and "x86: services, storage, audio" PASS 1178 s (`bt_hci` starts there with no
  controller).

## On the Ryzen (2026-11-28, bench-52)

Release 202611280807 = `claude/DRV-ryzen-bench-52-quiet-bt-wifi-refusal` 8b308d9df: `main` bd350b2f5 with this branch
and 213-DRV-0116. The QEMU groups "x86: boot, display, shell, memory, clock" and "x86: keys, shell, tools" passed on that
commit. The MX Vertical was asleep from the start.

- The first three tries were said one by one, at 26.6 s, 58.6 s and 90.7 s. The third, at 120.7 s, ends `…; 3 TRIES
  FAILED IN A ROW, THE NEXT ARE SUMMED UP EACH 10 MIN`.
- Then nothing for ten minutes, and at 728.7 s: `STILL NOT CONNECTED: 19 TRIES IN 10 MIN FAILED (THE LAST STATUS 04), 22
  IN A ROW; TRIED AGAIN EACH 2 S`.
- The maintainer clicked the mouse when called. At 842.4 s:
  - `CONNECTED OVER LE: HANDLE 010, WE PAGED IT`, then `CONNECTED AFTER 25 FAILED TRIES`;
  - `ENCRYPTED`, `HID READY`, the first report at 843.5 s.

  His word: «указатель двигается» (the pointer moves), with `wm` started to see it.

Left: a whole night on this release. The ring should hold the summing lines only.


## Related

- [700-BLT-0009](../issues-done/700-BLT-0009-le-pairing-and-hid-over-gatt.done) (the LE central and its reconnect).
- [213-DRV-0113](../issues-done/213-DRV-0113-display-both-hdmi-sockets-lit-at-once.done): the release whose night showed this.
