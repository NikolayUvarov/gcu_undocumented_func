# Landing request: 213-DRV-0116, a channel the calibration's policy refuses leaves the radio up

**From:** DRV · **To:** `LANDING` · **Date:** 2026-11-28

**Ready:** `claude/213-DRV-0116-wifi-refuses-a-channel-and-stays-up` 5184b4189

**What it changes:** `rtw89_wifi` only (`receive_service.rs`, `main.rs`), its host case (`tests/rtw89_receive_service_cases.rs`), and the issue.

- **Before:** on the Ryzen under `00`, with the access point on channel 40, the policy refused the admission (TSSI's 12 dBm above the channel's world maximum) before any RF write. Each time, the driver ended for a fresh start, until init quarantined it for 240 s.
- **Now:** the serving loop asks the policy first (`admission()`, the part `calibrate` used to do in place). On a refusal:
  - the channel is marked;
  - the filter is opened again;
  - the refusal is logged once (`CHANNEL n NOT ADMITTED: …`);
  - every later send there is refused at once, until a country is given again.

  The radio stays up receiving.

**It advances:** 213-DRV-0116. Its host criteria are met; the Ryzen's run is next, on a bench image.

**Tested on LEG9:**
- `rtw89_service_host`: 370 passed, the new case among them;
- the mutation check: with the refusal's check taken out of the send, the new case fails;
- `rtw89_wifi` builds;
- `02_build.sh`;
- all of `scripts/host_tests.sh` at b6d4eec6b (`main` 4df3fe984 merged) exits 0, 282 `test result: ok`.

No QEMU suite starts `rtw89_wifi`, since QEMU has no such device.

In `fast-test` (ee33feb5a).
