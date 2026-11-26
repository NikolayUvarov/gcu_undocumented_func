# Landing request, again: 213-DRV-0087, the join on channel 100 under `00`: both changes

**From:** DRV · **To:** `LANDING` · **Replaces:** `LANDING/2026-11-26T1125Z-DRV-ready-213-drv-0087-the-hardware-filter-stays-open-until-admission.md` · **Date:** 2026-11-26

**Ready:** `claude/213-DRV-0087-beacons-pass-until-admission` a40bfde6c

**What is new since the first request:** one more change, at the maintainer's word of today («Да, делай», to the first of three ways put to him).
- **`rtw89_wifi/src/calibration_policy.rs`:** under the unknown country (`00`) TSSI's 12 dBm is compared with the channel's maximum, the value the 75 % cap (801-DRV-0071) is taken from, not with the capped limit. The cap goes on limiting every frame the station sends. Held against the cap, channel 100's 11.5 dBm refused the calibration and so every join there. A channel whose own maximum is below 12 dBm, and a known country, are as before.
- Its host cases (`tests/rtw89_calibration_policy_cases.rs`), `hwdocs/rtl8852be-service.md` and `hwdocs/rtl8852be-calibration-policy.md`, and the issue's record.

**With both, on the Ryzen today** (the driver of d57b1aebf with a40bfde6c's two changes, replaced over the cable; boot 105; it holds for that machine and that access point, MC-12.1):
- `LISTEN-ONLY CHANNEL 100: A0:36:BC:11:F8:8C HEARD 81 MS AFTER THE TUNE`, IQK and DPK `PASS` on both paths, `POWER CAPPED AT 75 % … ON CHANNEL 100: 1 TX AT MOST 23 HALF-DBM OF ITS 26`, `DATAPATH ADMITTED ON CHANNEL 100`, `THE FIRMWARE TOLD: CONNECTED TO A0:36:BC:11:F8:8C ON CHANNEL 100 AS AID 32`, `AUTO-JOIN CoolNet2: JOINED (TRY 3)`, the address kept;
- three minutes later `TRAFFIC ON 100: … 1706 BEACONS; 9964 SENT, 9964 ACKED, 0 LATE; 0 HOLES`, the firmware's error codes 0; `netbench` 4 MiB down at 229 and 306 KiB/s, up at 101 and 103 KiB/s, 500 of 500 UDP round trips.

**Advances:** 213-DRV-0087 (left: a restart of the machine under the kept `00`, which needs a release with these changes, and the way back to channel 8).

**Tested:** LEG9, `build (x86)` and all of `scripts/host_tests.sh` on a40bfde6c: PASS (1093 s). The calibration policy's cases 277 passed, the driver's service cases 369. In `fast-test` (dcc266585).
