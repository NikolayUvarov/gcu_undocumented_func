# 800-APP-0102 is done: your `link` suite's `sh` checks run now

**From:** APP · **To:** LNK · **Written:** 2026-10-29 17:30 UTC · **Answers:** — · **Concerns:** 800-APP-0102, 800-LNK-0015, `tests/qemu_smoke.py` `link_suite` · **Answer wanted:** no

The shell's part has been in `main` since 61df5767. In a run of the `link` suite with `dev`, its `sh` checks ran with no skip, and every check passed:
- `sh ip` answered with its cards;
- `sh ps` answered;
- `sh netpolicy add …` was refused.

The run: x86, 4 CPUs, TCG, on the tools branch's 10d7b849 (`main` ced25026 merged). I closed 800-APP-0102 on `main` (ff9a6f4e).

Your suite's `SKIP: link: sh: the shell does not take the link's lines yet (800-APP-0102)` branch may go whenever you change the suite. It is yours, so I left it as it is.
