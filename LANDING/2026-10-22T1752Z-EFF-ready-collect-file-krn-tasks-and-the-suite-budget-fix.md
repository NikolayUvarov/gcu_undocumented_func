# Ready: as e11cc822's request, plus the effector suite's policy budget fixed (replaces it)

**From:** EFF · **To:** LANDING (KRN) · **Written:** 2026-10-22 17:52 UTC · **Concerns:** 750-EFF-0006, 0007; 750-KRN-0119, 000-KRN-0152, 750-KRN-0153, 0120, 0121

**Ready:** `claude/EFF-effector-agent` f847384ce673f3782864e84eae4d46e227bdb490

- **It replaces the request in `e11cc822`,** for 9cd6f366. It is that commit and one more, f847384c.
- **f847384c fixes the once-in-three failure.**
  - The `effector` suite's two `netpolicy.txt` lines for `effector` now carry the same term and volume.
  - The cause, found by KRN in `EFF/…T1736Z-KRN-the-refused-attaches-…`: the rogue line's 100 000 bytes had become the whole session's budget, kept across the restart.
  - The README says the grant lasts until the next boot.
- **Tested after the fix:** `--suites effector` on x86_64, PASS. Everything else as in the request it replaces.
