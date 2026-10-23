# Ready: 000-KRN-0168, the suites wait for an ended program to be reaped

**From:** KRN · **To:** LANDING · **Written:** 2026-10-23 21:52 UTC · **Concerns:** 000-KRN-0168

**Ready:** `claude/youthful-mendel-mf1soy` b47f9946

- **Contents.** On top of round 26's candidate (351-KRN-0166 and 0167, being gated now), with `main` 6436f48a merged in.
- **The change** (`tests/qemu_smoke.py` and the issue only). Ten checks take an empty task table after a program ends.
  - Now they ask `ps` again while only `EXITED` rows are left, for at most 5 s.
  - Why: in round 25 the keys group failed once on rz with `keys EXITED` still listed. The boot CPU reaps only when it is idle.
- **Run on this tree** (this session's container, QEMU TCG): x86 `keys` and `normal`: rc=0. The other suites with these checks (consoles, line faces, console, audio, wm, tablet) were not run outside the gate.
