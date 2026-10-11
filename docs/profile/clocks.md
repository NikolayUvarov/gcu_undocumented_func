# Clocks — `x86-64/QEMU-0`

| Clock | Source | Resolution | Interface | Notes |
|---|---|---|---|---|
| Monotonic clock | TSC calibrated at boot over 50 ms of the ACPI PM timer (without one, over five PIT ticks), used when its rate is constant (CPUID invariant TSC, or running under a hypervisor); otherwise the tick | 1 ns step with a calibrated TSC (QEMU: about 2.7 GHz), 1 ms with the tick | `CLOCK` (nanoseconds since boot, resolution, TSC Hz), `mind::time::monotonic_ns` | Never decreases on a CPU (the kernel keeps the last value per CPU); tasks are pinned, so a task never sees it go backwards. Values from different CPUs may differ by the TSC skew between them. |
| Uptime | The BSP's LAPIC timer at 1000 Hz, periodic, its rate measured over the same 50 ms of the ACPI PM timer; without a PM timer, PIT channel 0 through the 8259 and LAPIC ExtINT (211-PRT-0003). Every tenth tick ends a time slice and is forwarded to the APs that run a task, by IPI (000-KRN-0083). The boot line `MIND CORE KERNEL: TICK: …` names the source and whether the PIT counts. With a calibrated TSC, `UPTIME` is read from the monotonic clock, so a tick taken late is not lost | 1 ms | `UPTIME` (milliseconds since boot) | Sleeps end on its ticks; IPC timeouts count from its value. Not adjustable. |
| Sleep | A deadline on the monotonic clock (the time asked plus the clock's resolution), checked at each tick of the boot CPU (000-KRN-0065) | Ends at the first tick after the deadline: never shorter than asked by the asking CPU's clock (give or take the skew between CPUs), at most a tick (1 ms) longer, more when the boot CPU takes the tick late; at most 60 s per call | `WAIT` | Ends early when the task's input queue is not empty. IPC timeouts count from the uptime in milliseconds and may end up to a millisecond early: drivers poll with them. |
| Calendar time | CMOS RTC, read by the `rtc` service | 1 s | `CALL` to the RTC endpoint (seconds since midnight) | A service with its own capability; changing it does not affect `UPTIME` (MC-5.6). |
| Time stamp counter | `RDTSC` | CPU cycles | `RDTSC` | Raw cycles; `CLOCK` reports the calibrated frequency. Not synchronized between CPUs by the kernel. |

`UPTIME`, `CLOCK` and `RDTSC` are answered without the scheduler lock. They read only the clocks, so a task reading time does not wait behind other CPUs' scheduling work (000-KRN-0011). Their counts reach `STAT` (`calls`, `interrupts`) at the CPU's next switch.

## Not provided

- Deadlines and jitter bounds (MC-5.3, 5.4): not claimed. CPU budgets per period exist (C7) but are enforced at the end of each 10 ms time slice.
- Time zones and dates.
- Sleeping with resolution better than 1 ms, or scheduling better than the 10 ms time slice: `WAIT` and IPC timeouts end on the tick, and a task made ready waits for its CPU's time slice to end unless that CPU is idle.
- A tickless idle: the boot CPU takes its tick every millisecond even when nothing runs.
- IPC timeouts no shorter than asked: they may end up to a millisecond early (000-KRN-0065 covers `WAIT` only).
- Accuracy of the TSC and LAPIC timer calibration beyond the reference: 50 ms of the ACPI PM timer (3.579545 MHz), or of the PIT without one.
- A LAPIC timer that keeps running in deep C-states (no ARAT check): idle CPUs use `HLT` (C1), where it runs.
