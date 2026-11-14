# 000-KRN-0249 — The aarch64 update through the link in two groups, each under the gate's limit

**Type:** the gate (tests) · **Owner:** `KRN` · **Priority:** P1 · **Status:** open (2026-11-14) · **Blocked by:** — · **Roadmap:** — · **Constitution:** MC-12.1, MC-12.2

## Problem

The group `aarch64: an update through the link` runs `tests/qemu_smoke.py --arch aarch64 --suites linkupdate`. Since 801-LNK-0044, that suite runs three checks in one group:
- `linkupdate_suite`;
- `slot_applications_check`;
- `cable_without_words_check`.

Its times:
- on acc-1, on `ec0264e037`: 3095 s;
- in round 136's gate, on pcu: it reached the gate's limit of 3300 s (`GROUP_LIMIT` in `scripts/land.py`) inside `cable_without_words_check`;
- when it ran again, on x32: it passed in 3054 s.

A group that ends at the limit on one machine and passes on another is no evidence for either run (MC-12.1). Every round now risks a red gate, or a rerun of almost an hour, from the time alone.

## Plan

- **Two groups in `scripts/ci_local.sh`:**
  - `aarch64: an update through the link` runs `linkupdate_suite` and `slot_applications_check`;
  - `aarch64: the cable with no words` runs `--suites cable`, LNK's check alone.

  The `cable` suite already exists in `tests/qemu_smoke.py`, added in 801-LNK-0044.
- **The same for x86** (2030 s on x32, 2070 s in round 136), so that the two architectures run the same checks in the same groups.
- **The `linkupdate` suite** no longer runs `cable_without_words_check`. LNK is told, because it added that call.
- **Each group gets its estimate** in `scripts/land.py`.

## Acceptance criteria

1. On aarch64, both groups pass on the Effector machines, each well under 3300 s, on the same commit.
2. The same on x86.
3. `cable_without_words_check` runs exactly once in the gate's list.

## Related

801-LNK-0044, [801-KRN-0247](../issues-done/801-KRN-0247-init-starts-the-cable-with-no-word-on-the-disk.done), 000-KRN-0210.
