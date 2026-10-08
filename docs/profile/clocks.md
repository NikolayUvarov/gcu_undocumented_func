# Clocks — `x86-64/QEMU-0`

| Clock | Source | Resolution | Interface | Notes |
|---|---|---|---|---|
| Monotonic clock | TSC calibrated at boot over 50 ms of the ACPI PM timer (without one, over five PIT ticks), used when its rate is constant (CPUID invariant TSC, or running under a hypervisor); otherwise the tick | 1 ns step with a calibrated TSC (QEMU: about 2.7 GHz), 10 ms with the tick | `CLOCK` (nanoseconds since boot, resolution, TSC Hz), `mind::time::monotonic_ns` | Never decreases on a CPU (the kernel keeps the last value per CPU); tasks are pinned, so a task never sees it go backwards. Values from different CPUs may differ by the TSC skew between them. |
| Uptime | The BSP's LAPIC timer at 100 Hz, periodic, its rate measured over the same 50 ms of the ACPI PM timer; without a PM timer, PIT channel 0 through the 8259 and LAPIC ExtINT (211-PRT-0003). Forwarded to APs by IPI. The boot line `MIND CORE KERNEL: TICK: …` names the source and whether the PIT counts | 10 ms | `UPTIME` (milliseconds since boot) | Deadlines of `WAIT` use it. Not adjustable. |
| Sleep | Same tick | 10 ms steps, at most 60 s per call | `WAIT` | Ends early when the task's input queue is not empty. |
| Calendar time | CMOS RTC, read by the `rtc` service | 1 s | `CALL` to the RTC endpoint (seconds since midnight) | A service with its own capability; changing it does not affect `UPTIME` (MC-5.6). |
| Time stamp counter | `RDTSC` | CPU cycles | `RDTSC` | Raw cycles; `CLOCK` reports the calibrated frequency. Not synchronized between CPUs by the kernel. |

`UPTIME`, `CLOCK` and `RDTSC` are answered without the scheduler lock. They read only the clocks, so a task reading time does not wait behind other CPUs' scheduling work (000-KRN-0011). Their counts reach `STAT` (`calls`, `interrupts`) at the CPU's next switch.

## Not provided

- Deadlines and jitter bounds (MC-5.3, 5.4): not claimed. CPU budgets per period exist (C7) but are enforced at the 10 ms tick.
- Time zones and dates.
- Sleeping or scheduling with resolution better than 10 ms: `WAIT` still uses the tick.
- Accuracy of the TSC and LAPIC timer calibration beyond the reference: 50 ms of the ACPI PM timer (3.579545 MHz), or of the PIT without one.
- A LAPIC timer that keeps running in deep C-states (no ARAT check): idle CPUs use `HLT` (C1), where it runs.
