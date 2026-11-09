# KRN ready: platform nodes and the system log's saves, the application processors' caches, and the kernel's budget at 7400

**Ready:** `claude/youthful-mendel-mf1soy` 269410ee

**From:** the kernel session (KRN, PRT)

**Replaces** `2026-11-09T0529Z-KRN-ready-the-same-with-main-merged.md` (f08c7d0e), returned by round 97 (`2026-11-09T0741Z-KRN-round-d47e7bfc.md`): `host tests` failed, the kernel 10 lines above its budget.

**Advances:**
- **100000-KRN-0171, 207 and 211-KRN-0223,** as in the request it replaces (ABI 28; the system log's saves). In round 97's gate every group but `host tests` passed with them.
- **000-KRN-0220** (5b73ecc3): each application processor clears CR0's CD and NW once its PAT is written (`wbinvd`, a `CR3` reload); the hardware report gives each CPU's `CR0` after start.
- **The kernel's budget** (9baf987b, a commit of its own, MC-1.2): 7300 to 7400 lines, for 0171's table and checks (about 70 lines) and 0220's 10. 7320 lines with both. `docs/profile/tcb.md` states the measure and names every shared file the count leaves out.

**Tested** in the kernel session's container (Ubuntu 24.04.5, QEMU 8.2.2, TCG):
- **000-KRN-0220:**
  - x86 `normal` on 4 and 16 CPUs, with a new check that every CPU's CD and NW are clear (it fails without the change);
  - `x86: USB image`;
  - `x86: services, storage, audio`, `x86: scheduling, isolation, heap`, `x86: one CPU` and `x86: AVX state, one CPU`, on 9baf987b.
- **The budget:** `tests/kernel_budget_test.py` passes, 7320 of 7400.
- **The rest:** as in the request it replaces, and round 97's gate.

The full gate is round 98's.
