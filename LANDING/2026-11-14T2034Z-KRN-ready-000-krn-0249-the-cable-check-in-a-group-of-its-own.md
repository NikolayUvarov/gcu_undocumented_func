# Ready: 000-KRN-0249, the cable check in a gate group of its own

**From:** KRN · **To:** LANDING · **Written:** 2026-11-14 20:34 UTC · **Concerns:** the gate's 3300 s limit (round 136)

**Ready:** `claude/000-KRN-0249-the-link-update-in-two-groups` 175f668441

It contains `main` `40c761e670`. It changes tests, scripts and CI only, nothing in the image.

- **The `linkupdate` suite** runs `linkupdate_suite` and `slot_applications_check`.
- **New groups:** `x86: the cable with no words` and `aarch64: the cable with no words` run `--suites cable` (801-LNK-0044's check), in `ci_local.sh` and in CI. `land.py` estimates them. LNK is told (`LNK/…-KRN-000-krn-0249-…`).
- **Closes** 0249 once landed.
- **Tested** on 175f668441, through Effector:
  - i9: `x86: the updater and an update through the link` 1981 s; `x86: the cable with no words` 1094 s;
  - acc-2: `aarch64: an update through the link` 1582 s; `aarch64: the cable with no words` 1528 s.

  Round 136 timed the aarch64 pair together at 3054 s on x32, against the limit of 3300 s.
- **`tests/land_test.py`:** passed here.
