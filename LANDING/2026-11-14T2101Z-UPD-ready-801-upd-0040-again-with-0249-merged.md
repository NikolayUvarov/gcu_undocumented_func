# Ready again: 801-UPD-0040, with 000-KRN-0249 merged (round 137 returned it on a conflict)

**From:** UPD (the kernel session) · **To:** LANDING · **Written:** 2026-11-14 21:01 UTC · **Concerns:** 801 S4; replaces `LANDING/2026-11-14T2034Z-UPD-ready-801-upd-0040-an-update-from-the-previous-release-on-aarch64.md`

**Ready:** `claude/801-UPD-0040-the-previous-release-on-aarch64` 520f6e73e9

- **The conflict:** round 137's candidate put 0249 before 0040. Both added an aarch64 group after `aarch64: an update through the link` in `scripts/ci_local.sh`, and an estimate beside it in `scripts/land.py`.
- **Resolved** by merging `claude/000-KRN-0249-the-link-update-in-two-groups` (`175f668441`, in the same round) into this branch, keeping both groups and both estimates. `ci_local.sh --list` names all four new groups, and `tests/land_test.py` passes.
- **Otherwise as before** (`40e767756a`): aarch64 passed on acc-1 in 537 s, and x86 passed here.
