# 100000-PRT-0012 — The A64's counter erratum: a stable read of the generic timer

**Type:** porting (kernel) · **Owner:** `PRT` (the kernel session) · **Priority:** P2 · **Status:** open (the workaround made and tested on the host and in QEMU, below; the phone's run left) · **Blocked by:** a PinePhone at hand to show the fault and its fix ([issues-human, section 12](../issues-human/README.md#12-a-pinephone-to-test-on)) · **Main task:** [100000](100000-a-phone-the-pinephone-first.md), stage 1 · **Roadmap:** track H · **Constitution:** MC-1.3, MC-12.1

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
- **The same for programs**, which may read `CNTVCT_EL0` at EL0 (EL0VCTEN): none does today but a test fixture (`tests/busy_app.rs`); `libmind`'s time comes from the kernel. A program that reads it itself gets the retry, or the kernel traps EL0's counter reads on such a board.
- **Other errata later,** by their own properties (`fsl,erratum-a008585` is already recognised as one).

## Acceptance criteria

- **Host tests** of the retry against a model counter that returns the erratum's bad values.
- **On the PinePhone:** a run of hours with the clock checked against the host's time through the link, no tick lost or doubled beyond the budget the run states.
- **docs/profile** names the erratum and the workaround for that board.

## Progress (2026-11-08)

- **`common/counter.rs`:** `sun50i_stable`, the read taken again while its low 9 bits are all ones or all zeros, at most 150 reads, as Linux's sun50i workaround.
- **The kernel** reads the count in one place (`arch/aarch64/clock.rs`, `count()`); the tick (`interrupts.rs`) calls it too.
  - The device tree's reader sets `board::COUNTER_SUN50I` for `allwinner,erratum-unknown1` and says so: `… THE TIMER NAMES THE A64'S COUNTER ERRATUM: EACH READ RETRIED WHILE ITS LOW BITS ARE ALL ONES OR ZEROS`.
  - Freescale's A-008585, a different workaround, is named and not worked around.
  - `fdt::Board.timer_erratum` is now an `Erratum` kind, no longer a flag.
- **Tests:**
  - `tests/counter_host.rs` (a model counter: good reads taken at once, the errata values read again, the tries bounded);
  - `tests/fdt_host.rs` (the PinePhone's tree names `Sun50iA64`);
  - QEMU `virt,acpi=off,gic-version=2`, four Cortex-A53, U-Boot 2025.10, with `-dtb` a `virt` tree whose timer node names `allwinner,erratum-unknown1` (the kernel session's container, 2026-11-08): the kernel names the erratum and turns the retry on; the shell starts, four CPUs online, and `uptime` advanced 20 s over 20.3 s of the host's. QEMU's counter has no such fault: this shows the workaround harms nothing, not that it cures the A64's (MC-12.1, 12.2).
- **Left:** the hours' run on the PinePhone, and programs' own reads.

## Related

[100000](100000-a-phone-the-pinephone-first.md), [100000-PRT-0009](../issues-done/100000-PRT-0009-the-board-from-the-device-tree.done).
