# 213-DRV-0116 — `rtw89_wifi`: a channel the calibration's policy refuses is refused to the station, the radio kept up

**Type:** driver (`rtw89_wifi`) · **Owner:** `DRV` · **Priority:** P2 · **Status:** open (2026-11-28) · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-6.1, MC-12.1

## Problem

On the Ryzen on 2026-11-27 ([213-DRV-0087](213-DRV-0087-rtl8852be-on-5-ghz.md), "The access point moved to channel 40"), the
access point's 5 GHz network was on channel 40 and the kept country was `00`. The station's first send there asked for
the channel's admission. The calibration's policy refused it before any RF write: TSSI's bursts are 12 dBm, above that
channel's maximum under the world's rules (`Admission(TssiPower)`). Then:

- the driver's service stopped (`WLANHW SERVICE STOPPED 1: Admission(…)`);
- the driver exited so that init would start a fresh one (`RESTART_AFTER_FAILURE`, a debug behaviour);
- `wlan` joined the same network again, and the same refusal followed;
- after three starts in a minute, init quarantined the driver for 240 s.

So a refusal that depends only on the country, the channel and the board cost a driver start each time, and the
quarantine. No answer to the station said why.

## Plan

- The policy's part of the admission, the part with no RF write, is a function of its own:
  `receive_service::admission(board, limits, channel, heard, platform)`. `calibrate` uses it, as before.
- When the station asks to admit a channel, the serving loop asks that function first. If it refuses:
  - the receive-only service marks the channel refused under the limits now;
  - it opens the filter again, as before the send;
  - it logs `CHANNEL n NOT ADMITTED: <reason>; …` once.
- Every later send on that channel is refused at once (`failed`), as on a listen-only channel with no access point
  heard. The radio stays up receiving, so the station can scan and try another network.
- A country given again (`set-country`, or the world rules taken) forgets the refusals, because the policy may answer
  otherwise.
- What happens next is `wlan`'s and is not changed here. Falling back to the network's 2.4 GHz BSS is `NET`'s, and was
  told there on 2026-11-27.

## Acceptance criteria

- [x] Host case (`tests/rtw89_receive_service_cases.rs`), under `US` with channel 40 at 10 dBm:
  - the policy refuses 40 with `TssiPower`;
  - the refusal is logged once and the filter is opened again;
  - three later sends are refused with nothing asked;
  - `info` still answers;
  - 36 is admitted;
  - a country given again asks the policy again.
- [x] The case fails with the refusal's check removed (a mutation check).
- [x] `rtw89_service_host` whole and all of `scripts/host_tests.sh` pass; the driver builds.
- [ ] On the Ryzen (MC-12.1), with the access point on channel 40:
  - under `00` the line `CHANNEL 40 NOT ADMITTED` comes once;
  - no driver restart and no quarantine follow;
  - `UZ` given again joins channel 40.

## Progress (2026-11-28)

On LEG9, at b94ccbd66 and again at b6d4eec6b (main 4df3fe984 merged):
- `rtw89_service_host`: 370 passed, the new case among them. With `refused_here` taken out of the send's check, the
  new case fails.
- `rtw89_wifi` builds; `02_build.sh` passes.
- All of `scripts/host_tests.sh` exits 0: 282 `test result: ok`, none failed.

Left: the Ryzen's run.

## Related

- [213-DRV-0087](213-DRV-0087-rtl8852be-on-5-ghz.md): the 5 GHz work, and the second open item of its channel-40 section, which this is.
- `NET/2026-11-27T0547Z-DRV-the-access-point-moved-to-channel-40-under-00-the-station-loops-on-it-and-never-tries-2-4-ghz.md` on `internal-dev-messages`: the station's side.
