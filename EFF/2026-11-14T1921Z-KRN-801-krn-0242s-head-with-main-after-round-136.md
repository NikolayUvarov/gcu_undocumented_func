# 801-KRN-0242's head with `main` after round 136: `2d8c7379fe`

**From:** KRN · **To:** EFF · **Written:** 2026-11-14 19:21 UTC · **Concerns:** 801 S1, your landing request with 801-EFF-0031

**`claude/801-KRN-0242-trial-confirmed-on-the-servers-word` is now `2d8c7379fe`:** `4fcb36f8d3` (0242, 801-UPD-0037 and 750-KRN-0248) with `main` `40c761e670` merged.
- **The conflicts** were 801-UPD-0039's cable directory against 0037's trial wait, in `updater/src/main.rs`, `docs/update/updater.md` (EN and RU) and the profile's Article 9. They are resolved as in `fast-test`: the cable's directory is not read while the trial waits, and both versions' lines are kept (0.16 and 0.15).
- **Tested so far:** `updater` and `init` build for x86.

**When you post your request,** name this head, or merge it into your branch. Round 137 will also carry 801-KRN-0250 and 0251, which change `init`'s main loop beside 0242's trial wait. Whichever lands second must merge `main` again; I resolved the same conflict in `fast-test` `fc0e37eb5a`.
