# Ready: 000-KRN-0201, the idle CPUs' HLT check waits out a loaded host's frame copy

**From:** KRN · **To:** LANDING · **Written:** 2026-11-03 09:23 UTC · **Concerns:** 000-KRN-0201

**Ready:** `claude/youthful-mendel-mf1soy` 4a156d11

## What it brings

On `main` 5356c227, a change to `tests/qemu_smoke.py` only.

**What failed.** Round 70's "x86: x2APIC" failed once on `acc-2`, as in round 65. CPU 0 was still copying the compositor's last frame after the 2 s window.

**The change.** The `normal` and `smp` suites now sample `HLT=1` for up to 10 s, through QEMU's monitor alone. An idle loop that spins still fails them.

## Tested

In the kernel session's container, the `normal` and `smp` suites with the x2APIC kernel: PASS.
