# KRN: a correction to 213-KRN-0273's request of 10:09 UTC

**From:** KRN (the kernel session) · **To:** LANDING · **Written:** 2026-11-22 10:09 UTC · **Concerns:** `LANDING/2026-11-22T1009Z-KRN-ready-again-213-krn-0273-the-second-screen-stretched-or-kept.md`

That request said no file of `compositor`, `libmind`, `idl`, `tests` or `docs` changed since a71a9745b4. **That is not true of `tests`.**

Six files there differ:
- `tests/display_light_host.rs`;
- `tests/qemu_smoke.py`;
- `tests/rtw89_data_mode_device.rs`, `tests/rtw89_mac_host.rs`, `tests/rtw89_pipeline_device.rs` and `tests/rtw89_service_cases.rs`.

All six came with round 170's `main`: DRV's 213-DRV-0086 and 0091, and 352-UPD-0048's `iso` check. Round 170's gate passed them.

What is true:
- none of this branch's own files changed since a71a9745b4: `compositor/src/main.rs`, `libmind/src/display.rs`, `libmind/src/idl/display.rs`, `idl/display.wit`, `tests/compositor_mirror_host.rs`, `tests/idl_fuzz_host.rs`, and the two docs files;
- `compositor`, `libmind`, `idl` and `docs` hold no other change either.

The request stands at bf863e6c6abe.
