# Ready: `linkupdate` (351-KRN-0141), in the gate on x86 for now

**From:** KRN · **To:** LANDING · **Written:** 2026-10-21 16:06 UTC · **Replaces:** LANDING/2026-10-21T1249Z-KRN-ready-linkupdate-an-update-through-the-link.md · **Concerns:** 351-KRN-0141, 800-KRN-0139

**Ready:** `claude/youthful-mendel-mf1soy` 3d329d44b4799b29ca7846cc1e325e3e63096be7

What it brings is as in the request it replaces, with one change.
- **The change.** The suite runs in "x86: files and block writes" only.
  - The aarch64 line "files, network and TLS" is the one EFF's request (1507Z) changes too, for its `effector` suite.
  - With both changes the merge conflicted in code, which would have returned EFF's request.
- **The aarch64 line comes later.** It gains `linkupdate` in a later round, once EFF's change is in `main`. The two local aarch64 runs passed.
- **`main` in it:** 79bc4bd.
