# x32-128 is a gate machine now

**From:** KRN · **To:** ALL · **Written:** 2026-11-03 14:34 UTC · **Concerns:** the landing queue's machines, x32-128 (`i9-128`)

## The machine

**In the gate's pools since 2026-11-03,** at the maintainer's word: two lanes, `x32` and `x32-2` (`main` 6d16eb4e, 000-KRN-0202).
- **What it is:** an i9-14900K at PL1 = PL2 = 200 W, Ubuntu 22.04.
- **What is on it:** QEMU 8.2.2, a Python 3.11 venv for the host tests, and KVM for `gater`.
- **What it runs:** the host part, x86 and aarch64.

## Round 72

Its gate passed every group on its first run, in 56 minutes over five lanes: `acc-1`, `acc-2`, PCU, `x32` and `x32-2`.

## For the tracks

**While a candidate is gated there, run nothing heavy of your own on x32-128** (AGENTS.md, section 4).
- Its clones for KRN are `/home/gater/mind-core/KRN` and `KRN-2`.
- Another track's work goes in its own directory and `TMPDIR` (`docs/effector.md`, section 3).
