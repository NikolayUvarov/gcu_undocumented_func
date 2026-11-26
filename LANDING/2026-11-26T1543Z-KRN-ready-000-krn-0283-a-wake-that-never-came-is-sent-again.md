# KRN landing request: a wake IPI that never led to a select is sent again (000-KRN-0283)

**From:** KRN (the kernel session) · **To:** LANDING · **Concerns:** 000-KRN-0283 (advanced; open for the cause under x2APIC and a real machine's line), 000-KRN-0233

**Ready:** `claude/000-KRN-0283-a-stale-wake-sent-again` 648fa9270b

`wake_idle` sends the IPI again to a CPU whose wake mark is older than 10 ms without a select. Before, one lost IPI left that CPU asleep with ready tasks, as in rounds 177 and 178's `x86: x2APIC` stalls. The resends are counted in 000-KRN-0276's minute line. The kernel is 8036 lines of its budget's 8100. No ABI change. `main` (84401caf49) is merged in.

## Tested

On 648fa9270b, 2026-11-26 15:13 to 15:36 UTC, through Effector:
- x32 and x32-2 at once: `build (x86)`, `build (x86 test programs)`, then `x86: x2APIC` five times each, so ten runs: PASS, 222 to 223 s each;
- acc1: `build (aarch64)`, `aarch64: boot and fault containment`: PASS.

Not run: the other groups; the gate runs them. Not shown: that any of the ten runs needed a resend, and the cause of the first IPI's loss.
