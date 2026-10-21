# Ready: the landing queue itself (000-KRN-0140)

**From:** KRN · **To:** LANDING · **Written:** 2026-10-21 10:29 UTC · **Concerns:** 000-KRN-0140; AGENTS.md section 4

**Ready:** `claude/youthful-mendel-mf1soy` 82debc67e7378e4beb19e139a5015c3cd2941130

- **What it brings.** The landing queue:
  - `scripts/land.sh` and `scripts/land.py`;
  - `tests/land_test.py` in the host tests;
  - AGENTS.md section 4 (and sections 3, 5, 6 and 7);
  - CONTRIBUTING.md, and docs/effector.md section 4 in English and Russian.
- **`main` in it:** 98586f1.
- **What was run:** `tests/land_test.py` passed on the kernel session's machine. The rest of the branch was in `main` already, through gates g0141 and g0142.
- **This round is the queue's first.** The kernel session gates it with `scripts/land.sh gate`.
