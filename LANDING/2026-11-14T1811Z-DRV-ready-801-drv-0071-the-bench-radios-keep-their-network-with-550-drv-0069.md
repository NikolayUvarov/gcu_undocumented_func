# DRV landing request: the bench's radios keep their network (801-DRV-0071), with 550-DRV-0069

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-14 18:11 UTC · **Concerns:** 801-DRV-0071, 550-DRV-0069, KRN's request 1 in `requests-DRV.md` (801-KRN-0243)

**Ready:** `claude/801-DRV-0071-radios-keep-their-network` 979edc5873766c8fc04116222adb2bd48f57a5df

**This request carries 550-DRV-0069 too.** `claude/550-DRV-0069-rtw89-no-channel-until-tuned` (c5ad772e) is merged into this branch whole (1e4f69a8), so it gets no request of its own.

## What it carries

`main` 32272bba66 merged in (979edc58), with no conflict. In `fast-test` already, but for the merge.
- **a3dc3999, 550-DRV-0069:** `rtw89_wifi`'s receive-only service answers `info.channel` 0 until its client sets a channel (a `set-channel` to the channel it listens on, or a passive tune). So `wlan` sees a restarted radio and gives it the country again (550-NET-0061).
- **f5371668, 801-DRV-0071:**
  - **`rtw89_wifi` under an unknown platform word** (KRN's request 1): `platform=unknown:<why>`, a malformed or repeated word, or none, is held back, not enforced. The radio sends under the country's limits alone, as with no platform limit, and says so once. `platform=limits:…` is still obeyed, and `PlatformInput::Unknown` itself still refuses.
  - **75 % of the maximum under an unknown country** (the maintainer's decision of 2026-11-14), rounded down to the 0.5 dB step, so 3 steps (−1.5 dB):
    - `rtw89_wifi`: under `00`, or no country before the station's first send that is not a probe request; set at admission;
    - `rtw88_wifi`: under `00` or none yet. It now takes `00`, which it answered `invalid` before, against `wlanhw.wit` 1.3;
    - `bcm_wifi`: no power path, so the cap is not applied, and it says so.
  - **The Ryzen's silent stall:** in data mode, 10 s with no Wi-Fi frame while the filter lets a network in stops the data service with a line, and the driver exits for `init` to start a fresh one. 16 unicast sends in a row not acknowledged over 5 s are logged once.
- **d5c4f3b4, c5ad772e:** the issues' progress, the maintainer's decisions, the bench's replace builds, and three other DRV issues' runs of 2026-11-13. d5c4f3b4 also updates `hwdocs/rtl8852be-service.md`, which the build copies into images, so it is not text only.

**Issues:** advances 801-DRV-0071 and 550-DRV-0069, and serves KRN's request 1 in `requests-DRV.md` (for 801-KRN-0243). That file is not changed here. Closes none: the bench runs are left (each issue's "Left" list).

## What was tested, and where

LEG9, WSL2, Ubuntu 24.04, QEMU 8.2.2 (TCG), 4 CPUs, on 979edc58's exact tree:
- **Builds:** `./02_build.sh` (x86) and `scripts/x86_fixtures.sh`.
- **The full `scripts/host_tests.sh`** with `MIND_SSHD`: exit 0, 230 suites, 6253 passed, none failed. The radios' host suites are among them: `rtw89_channel_power_host`, `rtw89_calibration_policy_host`, `rtw89_service_host` (with `rtw89_receive_service_cases.rs`, 550-DRV-0069's `info.channel` 0 first), `rtw88_driver_host`, `rtw88_country_host`.
- **The group "x86: network, TLS and Effector"** (`net`, `tls`, `effector`, the `net` suite with `wlansim`): PASS, 1527 s. Among its checks: `a restarted radio (wlansim started again by init, no channel and no country): the station noticed, gave it RU again and joined MIND-TEST by itself (550-NET-0061)`, and the same for a radio restarted while the station was not joined.

**Recorded in 550-DRV-0069, not run again here:** on the Ryzen, its `rtw89_wifi` put in by `replace` over release 202611131315: `THE RADIO RESTARTED (IT HAS NO CHANNEL SET)`, the country again, and `AUTO-JOIN CoolNet2: JOINED`.

**Not tested** (MC-12.3): none of 801-DRV-0071's changes ran on a machine. QEMU starts none of these three drivers, so the QEMU group shows only that the rest of the network path is unchanged. Left for the bench: the Ryzen's platform and cap lines, a join and its 30-minute run; the DEXP's cap line and a join; the MacBook Pro's join.
