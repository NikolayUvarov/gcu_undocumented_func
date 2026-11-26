# Landing request: 213-DRV-0087, the hardware filter stays open until a send admits the channel

**From:** DRV · **To:** `LANDING` · **Date:** 2026-11-26

**Ready:** `claude/213-DRV-0087-beacons-pass-until-admission` 9241d7b02

**What it is.** One change in `rtw89_wifi/src/receive_service.rs` with its host case (c95eb9d51), the issue's record of the run (text), and `main` 5dccdc819 merged in.
- **The fault, on the Ryzen today:** the first join on channel 100 under `00` ended `NO ANSWER` fourteen times. The station's filter for its network narrowed the hardware's before the first send; narrowed, the hardware checks beacons against a BSSID this mode never sets and passes none, so the beacon a listen-only channel waits for never came.
- **The change:** a scan's filter opens the hardware's at once, as before; a network's narrows it with the send that admits the channel, still before the calibration.

**Advances:** 213-DRV-0087 (open: the join stops next at TSSI's power floor under the unknown country's cap, which waits for the maintainer's word; the issue says how).

**Tested, and where:**
- LEG9: `build (x86)` and all of `scripts/host_tests.sh` on 9241d7b02: PASS (159 s, 1094 s). The driver's service cases: 369 passed, the changed one among them (`a_networks_filter_narrows_the_hardware_when_a_send_admits_the_channel_and_a_scan_opens_it_again`).
- **On the Ryzen** (the driver of d57b1aebf with c95eb9d51, replaced over the cable): on channel 100 `LISTEN-ONLY CHANNEL 100: A0:36:BC:11:F8:8C HEARD 157 MS AFTER THE TUNE, 157 MS AFTER THE REFUSED SEND` and the next send admitted; on channel 8 `DATAPATH ADMITTED ON CHANNEL 8` and `AUTO-JOIN CoolNet2: JOINED` as before. That holds for that machine and that access point (MC-12.1).
- No QEMU suite reaches this driver (there is no such device there).

In `fast-test` (70d5ca55c).
