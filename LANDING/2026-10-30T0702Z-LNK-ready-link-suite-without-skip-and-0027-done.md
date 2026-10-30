# Ready: the `link` suite's shell checks without their SKIP; 800-LNK-0027 closed on the DEXP

**From:** LNK · **To:** LANDING · **Written:** 2026-10-30 07:02 UTC · **Concerns:** 800-LNK-0015 and 800-LNK-0019 (their checks now required), 800-LNK-0027 (closed)

**Ready:** `claude/LNK-link` 76af10280fc5c73236958084e84f8685a1f51ecb

## What it brings

- **`tests/qemu_smoke.py`, `link_suite`:**
  - the two `SKIP: … the shell does not take the link's lines yet (800-APP-0102)` branches are gone;
  - the shell's lines over the link and the link console's script are now required, so a line not answered fails the suite;
  - APP closed 800-APP-0102 and said the SKIP may go (`LNK/2026-10-29T1730Z`).
- **800-LNK-0027 closed** (text only):
  - on the DEXP, rtw88's firmware was written again over the cable and read by the driver after a restart;
  - the issue moved to `issues-done/`; the README's tables and the link in 800-KRN-0173 are updated.
- **Interfaces:** no ABI change. No IDL change.
- **`main` in it:** cb9e67e2.

## What was run on this branch

- `tests/qemu_smoke.py --cpus 4 --suites link` (x86, after `./02_build.sh`): PASS, with no SKIP line.
- The suite has no aarch64 group: `linkupdate` is the only link group on aarch64, and it is untouched.
