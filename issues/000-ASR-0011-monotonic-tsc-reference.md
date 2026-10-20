# 000-ASR-0011 — Compare guest TSC elapsed time with the host monotonic clock

**Type:** assurance / QEMU fixture · **Owner:** ASTRA auditor-fixer (the maintainer's assignment, 2026-10-15) · **Priority:** P1 · **Status:** in progress · **Blocked by:** full gate · **Roadmap:** Assurance · **Constitution:** MC-12.1, MC-12.3, MC-12.9

## Problem

The TSC recalibration regression compares guest monotonic elapsed time against
host realtime (`time.time`). A host clock adjustment changes that reference
independently of elapsed time. The old full gate at `9a47d0dd` failed with a
ratio of 0.95455. A focused instrumented reproduction, retaining the original
kernel, interval and assertion, failed with a realtime ratio of 0.94605.

That reproduction measured 31.470069 seconds of host realtime but only
30.050605 seconds of host monotonic time. The guest advanced 29.772393 seconds,
giving a monotonic ratio of 0.99074, inside the existing three-percent bound.
Both serial command round trips were about 50.34 ms. The recorded realtime
offset change explains this reproduction; the earlier uninstrumented failure
does not establish an independent kernel defect.

## Plan

Use `time.monotonic_ns()` around the same serial command and retain its
midpoint as the host elapsed-time reference. Preserve the 30-second interval,
three-percent bound and required reduction of the deliberately skewed TSC
calibration. Do not alter the kernel or hide failed runs.

## Acceptance criteria

- The reference measures monotonic elapsed time and is unaffected by wall-clock
  adjustments; serial-command midpoint sampling is retained.
- The existing calibration and elapsed-rate assertions remain unchanged.
- A focused QEMU clock run and all standard full-gate groups pass on the exact
  final candidate, with the old failure retained as separate evidence.

## Related

- [000-KRN-0100](../issues-done/000-KRN-0100-the-tsc-measured-again.done).
- `tests/qemu_smoke.py::clock_suite`.
- Private diagnostic: `/tmp/mind-astra-tsc-observation.log` on the audit host.

## Progress

The corrected complete clock suite passed on the unchanged earlier image,
using the new test function: deliberately skewed 3511 MHz was remeasured as
3201 MHz, and the 30-second guest/host monotonic ratio was 1.003. The original
three-percent assertion was retained. This focused run is separate from the
required full gate on the resulting commit.

On the final code candidate `1aea9100`, the dedicated native-Ryzen
`x86: TSC measured again` group passed in 47 s (Effector operation
`20261015193238.156862195_000072`). The deliberately skewed 3593 MHz was
remeasured as 3294 MHz; the 30-second guest/host monotonic ratio was 1.000.
The maintainer then explicitly deferred ARM to prioritize Intel/x86. The
cross-architecture gate criterion therefore remains open; the x86 result is
recorded in [the validation report](../issues-audit/2026-10-15-astra-validation.md).
