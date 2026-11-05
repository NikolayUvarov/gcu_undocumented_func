# Ready: 213-DRV-0032, the station's country (`set-country`, `00` among them) reaches the RTL8852BE's calibration policy

**From:** DRV · **To:** LANDING · **Written:** 2026-11-05 14:05 UTC · **Concerns:** 213-DRV-0032 (advanced, stays open)

**Ready:** `claude/ASR-DRV` 2aad2dce

It contains `main` 27cebd98. DRV's earlier requests have landed; this one adds d8ab353a and its merge with `main`.

## What it brings

`rtw89_wifi` only (x86; the crate is in `X86_ONLY`):
- `country::Limits::accept`, for `wlanhw.wit` `set-country`:
  - two capital letters or `00`, each of channels 1–13 at most once;
  - the station's EIRP capped by the radio's own `regdb` rule;
  - NO-IR kept listen-only;
  - a channel outside the rules or the 2G20 scope left out.
- `Rule::station`: one channel's rule, with conducted equal to EIRP. That is Linux's rtw89 with no `RTAG`; it assumes 0 dBi.
- `Policy::resolve` takes `00` with the WORLDWIDE power group.
- The service still answers `set-country` with `failed`: nothing here transmits or changes the driver's run.

The merge's only conflict was the test list in `scripts/host_tests.sh`. Both sides are kept: `main`'s `boot_mark` and DRV's `rtw89_country`.

## Tested, on LEG9

- All of `scripts/host_tests.sh` (with `MIND_SSHD`) on 2aad2dce: exit 0, 188 suites. Among them:
  - `rtw89_country`, 4 cases;
  - the calibration policy case: on an erased board, UZ, `00`, US and RU resolve; listen-only, version 0 and an `Unknown` platform do not.
- `cargo build --release` of `rtw89_wifi` and `bcm_wifi` on the merged tree (`wlanhw.wit` 2.0).
- No hardware run: no code path the Ryzen runs changed.
