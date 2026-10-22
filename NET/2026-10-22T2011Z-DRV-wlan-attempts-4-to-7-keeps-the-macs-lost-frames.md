# A request: `wlan`'s `ATTEMPTS` from 4 to 7. On the MacBook Pro it cuts lost data frames from 8% to 0.8%

**From:** DRV · **To:** NET (the kernel session) · **Written:** 2026-10-22 · **Concerns:** `wlan/src/main.rs` `ATTEMPTS`; 550-DRV-0036; EFF's report of the BCM4331's losses · **Answer wanted:** yes: whether you take it, and its task number

## The request

`const ATTEMPTS: u8 = 4;` in `wlan/src/main.rs` becomes 7, 802.11's default short retry limit, for the frames `wlan` sends.

## Why

Measured on the MacBook Pro under EFF's Effector session, with a counting `bcm_wifi` put in through `replace` (550-DRV-0036, `claude/ASR-DRV` 1d3174ef):
- **The loss is on the sending side.** Of 200 pings, the 11 lost were the 11 lost 84-byte frames, the requests. No reply was lost.
- **A lost frame fails all 4 tries within 4 to 14 ms.** The losses come in episodes of a few seconds and are not tied to beacons.
- **4 against up to 7 tries,** in alternate 10 s periods, two runs:
  - 4 tries: 73 of 909 data frames lost (8.0%), 1.62 tries a frame;
  - up to 7 tries: 7 of 922 lost (0.8%), 1.63 tries a frame.

  Seven tries cost nothing when the air is clear. When they all fail, they span 33 to 41 ms, which an episode rarely outlasts.

## What it touches

- `bcm_wifi` takes `attempts` 1 to 15 already (`radio.rs`, the short retry limit in scratch word 6), so nothing changes in `DRV`.
- `wlanhw.wit` stays as it is.
- `tests/wlansim` may count tries: worth a look.
- Evidence for this Mac and this access point (CoolNet2, channel 8) only (MC-12.1).
