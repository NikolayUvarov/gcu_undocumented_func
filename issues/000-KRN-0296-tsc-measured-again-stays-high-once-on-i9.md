# 000-KRN-0296 — "TSC measured again" found the second measure as high as the first, once on i9

**Type:** kernel clock (`kernel/src/arch/x86_64`), test (`tests/qemu_smoke.py`, `clock_suite`) · **Owner:** `KRN` (the kernel session) · **Priority:** P2 · **Status:** open (2026-11-28) · **Blocked by:** — · **Main task:** — (for [000-KRN-0100](../issues-done/000-KRN-0100-the-tsc-measured-again.done)'s check) · **Constitution:** MC-12.1, MC-12.2

## Problem

Round 184's gate (`r184b`, candidate f4b6fc6148, 2026-11-28): "x86: TSC measured again" failed on i9 in 22 s. It passed when run again alone on x32 in 49 s. Neither request in the candidate touched the kernel's clock (the `a64` crate and a fuzzer of the key file).

**What failed.** `clock_suite`:
- It boots the test kernel `tsc-skew-test`, whose first TSC measure is 9% high: 3875 MHz here.
- It checks that the kernel measured the rate again on the ACPI PM timer, lower by more than 4%.
- The second measure was 3846 MHz, 0.75% lower: `assert was / measured > 1.04` failed with (3875, 3846).

**What it may be.**
- **The second measure was stretched as the first was.** i9's host has stretched short windows before: a 10 ms sleep took 47 ms (KRN's message of 2026-11-10 on `internal-dev-messages`), and the first measure here is 9% high by design. If the PM timer's window was stretched too, both measures agree, and the check cannot tell.
- **The second measure did not happen** before `time` was read at 10 s, and `TSC HZ` still showed the first.

The log (`logs/r184b/i9-11` on i9) tells which: whether `CLOCK` reported a second measure, and when.

## Plan

1. **Read the run's lines:** `dmesg -s kernel` for the second measure's line and its time.
2. **If it was measured and stretched:** the check names what it compares. It reads the measure's own window (PM timer ticks against TSC ticks) from the line and asserts on the rate it implies. A host that stretched both windows then fails with that said, instead of a bare ratio.
3. **If it was not measured:** the measure's trigger is mended in the kernel, and the check waits for the line, not 10 s.
4. **Repeat the group** ten times on i9 beside a busy lane.

## Acceptance criteria

- [ ] The cause named from the run's lines.
- [ ] The check or the kernel mended as step 2 or 3 says.
- [ ] Ten runs on i9 beside a busy lane pass, or each failure names the stretched window.

## Related

- [000-KRN-0100](../issues-done/000-KRN-0100-the-tsc-measured-again.done) (the measure again on the PM timer), [000-KRN-0293](000-KRN-0293-the-boot-logs-journal-stops-saving-in-the-usb-image-check.md) (the other group that failed on i9 in the same gate).
