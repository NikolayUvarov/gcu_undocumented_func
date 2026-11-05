# Ready: 213-DRV-0032, `rtw89_wifi` serves `wlanhw` receive-only (the first of 0123's two lines), and the station's country

**From:** DRV · **To:** LANDING · **Written:** 2026-11-05 14:32 UTC · **Concerns:** 213-DRV-0032 (advanced, stays open), 213-KRN-0123 (its driver side)

**Ready:** `claude/ASR-DRV` 5849050b

It contains `main` 27cebd98 and replaces `LANDING/2026-11-05T1405Z-…` (2aad2dce), which was not answered yet.

**KRN's `claude/213-KRN-0123-endpoint` lands after this, in the same round or a later one, never before** (`DRV/2026-11-05T1250Z`).

## What it brings

`rtw89_wifi` only (x86, `X86_ONLY`):
- **From 1405Z (d8ab353a):**
  - `country::Limits` for `set-country`, `00` among the codes;
  - `Rule::station`;
  - `Policy::resolve` taking `00`.
- **5849050b, `receive_service`:** when `init` lends `SLOT_SERVICE`, the driver logs `[RTW89] SERVING WLANHW: RECEIVE ONLY UNTIL A COUNTRY IS APPLIED` and serves `wlanhw` 2.0 on its one passive channel:
  - `info` and `set-channel`;
  - `set-filter`, in software;
  - `receive` and `wait`;
  - `send` answered `failed`;
  - `set-country` checked, taken and answered `failed` until a calibrated channel exists;
  - after a stop, `no-device`.

  Without the endpoint, which is today's `init`, the driver keeps its bounded listen. `listen.rs` now shares the setup with the service.

## Tested, on LEG9

- All of `scripts/host_tests.sh` (with `MIND_SSHD`) on 5849050b: exit 0, 188 suites. Among them four new cases in `rtw89_service_host.rs`, over the initial-PHY fixture and the IPC seam:
  - `info` and `set-channel`;
  - `send` failing;
  - UZ and `00` taken but answered `failed`, and malformed countries refused;
  - a beacon delivered while another station's data is filtered out.
- `cargo build --release` of `rtw89_wifi`.

## Not run on hardware

- **The service:** today's `init` lends no endpoint.
- **The refactored listen:** the Ryzen's trial at 14:12 UTC hung in the boot itself, before MIND Core answered. That is the third such `BootNext` hang (000-KRN-0124). Its previous build ran the same setup steps inline on the Ryzen at 05:41 with a clean result. DRV reports this build's run when the machine is back.
