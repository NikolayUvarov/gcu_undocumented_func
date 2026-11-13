# Clocks — `x86-64/QEMU-0`

| Clock | Source | Resolution | Interface | Notes |
|---|---|---|---|---|
| Monotonic clock | TSC calibrated at boot over 50 ms of the ACPI PM timer (without one, over five PIT ticks), used when its rate is constant (CPUID invariant TSC, or running under a hypervisor); otherwise the tick. With the PM timer, its rate is measured again at 1, 8 and 60 s over all the time since the first measure (the PM timer read once a second on the boot CPU's tick; wraps a longer gap hid, while the boot CPU zeroed a large block, counted from the TSC at its current rate, 262-KRN-0160); where it differs by more than 0.1%, the clock goes on from its value at that rate, `CLOCK` reports the new rate, and the kernel's lines in the hardware report say so (000-KRN-0100) | 1 ns step with a calibrated TSC (QEMU: about 2.7 GHz), 1 ms with the tick | `CLOCK` (nanoseconds since boot, resolution, TSC Hz), `mind::time::monotonic_ns` | Never decreases on a CPU (the kernel keeps the last value per CPU); tasks are pinned, so a task never sees it go backwards. Values from different CPUs may differ by the TSC skew between them. |
| Uptime | The BSP's LAPIC timer at 1000 Hz, periodic, its rate measured over the same 50 ms of the ACPI PM timer; without a PM timer, PIT channel 0 through the 8259 and LAPIC ExtINT (211-PRT-0003). Every tenth tick ends a time slice and is forwarded to the APs that run a task, by IPI (000-KRN-0083). The boot line `MIND CORE KERNEL: TICK: …` names the source and whether the PIT counts. With a calibrated TSC, `UPTIME` is read from the monotonic clock, so a tick taken late is not lost | 1 ms | `UPTIME` (milliseconds since boot) | Sleeps end on its ticks; IPC timeouts count from its value. Not adjustable. |
| Sleep | A deadline on the monotonic clock (the time asked plus the clock's resolution), checked at each tick of the boot CPU (000-KRN-0065) | Ends at the first tick after the deadline: never shorter than asked by the asking CPU's clock (give or take the skew between CPUs), at most a tick (1 ms) longer, more when the boot CPU takes the tick late; at most 60 s per call | `WAIT` | Ends early when the task's input queue is not empty. IPC timeouts count from the uptime in milliseconds and may end up to a millisecond early: drivers poll with them. |
| Calendar time | CMOS RTC, read by the `rtc` service | 1 s | `CALL` to the RTC endpoint (seconds since midnight and days since 2000-01-01, UTC; `idl/rtc.wit` 1.3) | A service with its own capability; changing it does not affect `UPTIME` (MC-5.6). It keeps the time zone the clocks show as a fixed offset (000-APP-0099); the shell sets it from `data/clock.conf` at its start. Set by hand (`date set`) or from time servers by `timed` (below). |
| Time stamp counter | `RDTSC` | CPU cycles | `RDTSC` | Raw cycles; `CLOCK` reports the calibrated frequency. Not synchronized between CPUs by the kernel. |

`UPTIME`, `CLOCK` and `RDTSC` are answered without the scheduler lock. They read only the clocks, so a task reading time does not wait behind other CPUs' scheduling work (000-KRN-0011). Their counts reach `STAT` (`calls`, `interrupts`) at the CPU's next switch.

## Network time

`timed` (111-NET-0057, 111-KRN-0206) sets the calendar clock from the time servers the network policy names (`timed <server> udp <port>`, at most 4), over SNTPv4 without authentication. Stage 1 of 111.
- **What it takes:**
  - at least two answers that agree within 1 s beyond half their round trips, and their median;
  - only answers to its own request (the origin time is an unpredictable nonce), of stratum 1 to 15 and not unsynchronised.
- **Its bounds:**
  - **never before the floor:** the commit time of the build (`MIND_SOURCE_TIME`) and the latest time it set, kept in `system/timed/floor`, which only its badge opens;
  - **never past 2099,** which the RTC does not keep;
  - **a step of more than 24 hours only at the first set since boot.**
- **When:** every 10 s until the first set, then every 6 hours.
- **Only the calendar clock moves.** `UPTIME`, `CLOCK`, sleeps and IPC timeouts are untouched. A script's 25-second sleep across a step of more than two years lasted 25 001 ms by the monotonic clock and about 24 s by the host's (QEMU x86).
- **What it is trusted for:**
  - an answer anyone on the path can forge is taken only within these bounds;
  - two forged answers that agree can still move the clock:
    - at the first set since boot, forward without limit and back as far as the floor;
    - at each later round, forward by up to 24 hours, and never back before the latest time set;
  - so the network time is trusted for the logs' times, and for the checks that refuse what is too old (a channel's expiry, a certificate's validity) only as far as the floor allows. Stage 2 (NTS) authenticates the answers.
- **Tested:** host tests and seeded fuzzing of `mind::sntp`, and the `net` suite's `timed_check` in QEMU. The floor's file is written there, but the suite boots on a snapshot, so reading it back at the next boot is not tested. Not on a machine (MC-12.1).

### NTS (111's stage 2)

`timed` (111-NET-0075) authenticates the answers of the servers its policy names over TCP (`timed <server> tcp 4460`, with `timed <server> udp <port>` for the NTP port its key establishment names), by NTS (RFC 8915).
- **The key establishment** runs through `tls` (111-NET-0073): TLS 1.3 agreeing on `ntske/1`, the certificate checked against `tlsroots.pem` for the name the policy gave (111-NET-0074), the keys taken from the session's exporter. `timed` holds the keys and the cookies, never the TLS session's secrets (MC-11.9).
- **An answer counts** only when it carries this request's unique identifier and its authenticator holds under the server's key (AEAD_AES_SIV_CMAC_256). Its new cookies are kept, at most 8 a server; a server out of cookies, or one that sends an NTS NAK, has its key establishment run again, a failed one waiting 10 s and twice as long at each failure, up to 6 hours.
- **Plain servers** are used only as the owner's lines name them: an address with a UDP rule and no TCP rule.
- **The bound is wider:** when every server that agreed is authenticated by NTS, a step of more than 24 hours is taken at any set, not only the first since boot. The floor holds either way, and two servers must still agree.
- **What it is trusted for:** NTS authenticates the server, not its time. Two NTS servers that agree can move the clock forward without limit, and back as far as the floor; one on the path can delay or drop their answers but not change them.
- **The circularity:** the key establishment checks the server's certificate against the clock it is to set. A clock earlier than the certificate's start refuses it, and then only plain servers or the user set the clock. The bound: a clock at the floor (the build's commit time) accepts a certificate that was valid then, so a device whose clock was lost bootstraps by NTS only while its build is younger than the server's certificate.
- **Tested:** see 111-NET-0075's resolution.

## Not provided

- Deadlines and jitter bounds (MC-5.3, 5.4): not claimed. CPU budgets per period exist (C7) but are enforced at the end of each 10 ms time slice.
- Daylight saving and a zone database: a time zone is a fixed offset from UTC (`rtc.wit` 1.3), changed by hand (`date zone`, `wm`'s Settings).
- A CMOS clock that keeps local time (as Windows keeps it): `now` and `date` are taken as UTC; such a machine leaves the zone at UTC.
- The user's `date sync`: `timed` has no service endpoint yet. NTS servers that name their NTP server by a host name (`timed` resolves none), NTS cookies kept across reboots, and setting the clock to the floor so that NTS can bootstrap (111-NET-0075).
- Sleeping with resolution better than 1 ms, or scheduling better than the 10 ms time slice: `WAIT` and IPC timeouts end on the tick, and a task made ready waits for its CPU's time slice to end unless that CPU is idle.
- A tickless idle: the boot CPU takes its tick every millisecond even when nothing runs.
- Tasks of a CPU that zeroes a large block (more than 64 MiB): they wait until the block is zeroed, about 0.1 s a GiB expected on a machine (not measured) and seconds under QEMU's emulation, since the kernel does not leave a system call for another task. Between the block's 2 MiB pieces the CPU takes its interrupts, bookkeeping only: the boot CPU's tick still ends the sleeps and IPC deadlines of the other CPUs (262-KRN-0165).
- IPC timeouts no shorter than asked: they may end up to a millisecond early (000-KRN-0065 covers `WAIT` only).
- Accuracy of the TSC calibration beyond the reference: the ACPI PM timer (3.579545 MHz) over 50 ms at boot, then over 60 s; without a PM timer, five PIT ticks and no second measure. The LAPIC timer's rate is measured once, over the 50 ms.
- A TSC whose rate changes after 60 s: not measured again.
- A LAPIC timer that keeps running in deep C-states (no ARAT check): idle CPUs use `HLT` (C1), where it runs.
