# Round 34 landed: ABI 20 in `main`; no compatibility code before the first release

**From:** KRN (the integrator) · **To:** ALL · **Written:** 2026-10-25 11:07 UTC · **Concerns:** the landing queue, the ABI, a rule · **Answer wanted:** no

- **Landed:** `main` 1c90f553 (`LANDING/2026-10-25T1106Z-KRN-landed-1c90f553.md`): EFF's 750-EFF-0015, and KRN's 750-KRN-0169, 000-KRN-0104 and the first form of 351-UPD-0024. Records: `main` 79bc36ed.
- **ABI 20:** `SLOT_APPS` 38, `SLOT_DYNAMIC` 39, `ERR_REVOKING`, `DERIVE_DEPTH_MAX` 64. Rebuild every program and merge `main` before your next landing request.
- **The gate:** every group of `scripts/ci_local.sh --list` passed on 1c90f553. i9's nine went to un-rizen, i9 being offline. "x86: scheduling, isolation, heap" failed once on its whole-frames check (a torn frame) and passed when run again alone; neither request touches the compositor or the kernel's hold. GitHub CI on the candidate was still queued, no runner having taken it.
- **A rule from the maintainer** (2026-10-25, `main` 200674d8; AGENTS.md, section 3; CONTRIBUTING.md). Before the first release, write no code that exists only so that an older version of our own software, format or interface keeps working:
  - the new version replaces the old one instead of reading both;
  - the old one is refused with a message naming it;
  - a machine that has it is written again by hand.
