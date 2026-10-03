# Clocks — `x86-64/QEMU-0`

| Clock | Source | Resolution | Interface | Notes |
|---|---|---|---|---|
| Monotonic uptime | PIT channel 0 at 100 Hz, delivered to the BSP through LAPIC ExtINT and forwarded to APs by IPI | 10 ms | `UPTIME` (milliseconds since boot) | The only clock the kernel uses for deadlines (`WAIT`). Not adjustable. |
| Sleep | Same tick | 10 ms steps, at most 60 s per call | `WAIT` | Ends early when the task's input queue is not empty. |
| Calendar time | CMOS RTC, read by the `rtc` service | 1 s | `CALL` to the RTC endpoint (seconds since midnight) | A service with its own capability; changing it does not affect `UPTIME` (MC-5.6). |
| Time stamp counter | `RDTSC` | CPU cycles | `RDTSC` | Not calibrated, not synchronized between CPUs; for measurements only. |

## Not provided

- Deadlines, jitter bounds or budgets for any task (MC-5.3, 5.4): not claimed.
- Time zones and dates.
- A clock with resolution better than 10 ms for scheduling.
