# 100000-PRT-0012 — The A64's counter erratum: a stable read of the generic timer

**Type:** porting (kernel) · **Owner:** `PRT` (the kernel session) · **Priority:** P2 · **Status:** open · **Blocked by:** a PinePhone at hand to show the fault and its fix ([issues-human, section 12](../issues-human/README.md#12-a-pinephone-to-test-on)); the code can come before it · **Main task:** [100000](100000-a-phone-the-pinephone-first.md), stage 1 · **Roadmap:** track H · **Constitution:** MC-1.3, MC-12.1

## Problem

- **The PinePhone's tree names a counter erratum.** Its `arm,armv8-timer` node carries `allwinner,erratum-unknown1` (Linux v6.12's `sun50i-a64.dtsi`).
- **What it is.** On the A64, a read of the generic timer's counter (`CNTVCT_EL0`, `CNTPCT_EL0`) can return a wrong value now and then. Linux's workaround (`drivers/clocksource/arm_arch_timer.c`, the `sun50i_a64_read_cntvct_el0` family) reads again until the low bits are neither all ones nor all zeros.
- **Where it matters here.** The kernel's tick, its clock and the scheduler's time slices all read `CNTVCT_EL0` (`arch/aarch64/interrupts.rs`, `clock.rs`). A wrong read can:
  - jump the clock ahead;
  - set the next tick far off;
  - count many ticks at once.
- **Today** the kernel reads the board from the device tree (100000-PRT-0009) and only says so: `MIND CORE KERNEL: DEVICE TREE: THE TIMER NAMES A COUNTER ERRATUM, NOT WORKED AROUND YET`.

## Plan

- **One counter read** for the whole kernel (`arch/aarch64/clock.rs`) with the workaround when the board names the erratum:
  - the flag set by the device tree's reader;
  - the read retried while the low 10 bits are all ones or all zeros, with a bound on the tries, as Linux does.
- **The same for programs**, which read `CNTVCT_EL0` at EL0 (EL0VCTEN): `libmind`'s time read gets the same retry, or the kernel traps EL0's counter reads on such a board.
- **Other errata later,** by their own properties (`fsl,erratum-a008585` is already recognised as one).

## Acceptance criteria

- **Host tests** of the retry against a model counter that returns the erratum's bad values.
- **On the PinePhone:** a run of hours with the clock checked against the host's time through the link, no tick lost or doubled beyond the budget the run states.
- **docs/profile** names the erratum and the workaround for that board.

## Related

[100000](100000-a-phone-the-pinephone-first.md), [100000-PRT-0009](../issues-done/100000-PRT-0009-the-board-from-the-device-tree.done).
