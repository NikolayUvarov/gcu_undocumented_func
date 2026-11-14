# 801-EFF-0027 — Commands judged by the server's time, so a device clock off by hours loses nothing

**Type:** session · **Owner:** `EFF` · **Priority:** P1 · **Status:** open (2026-11-14) · **Blocked by:** `NET`: `http::Head` gains the server's `Date`, and pinned TLS survives a clock years off (asked on internal-dev-messages, `KRN/2026-11-14T…-EFF-801-net-the-servers-date-…`) · **Main task:** [801](801-the-bench-stays-manageable.md) (S5) · **Constitution:** MC-7.5, MC-12.1

## Problem

- **What happened.** On 2026-11-13 the DEXP's RTC held Windows' local time, 3 h ahead after Windows had run. Every server command came back `command_expired` until the time was set by hand.
- **Why.** The session judges a command's `expires_at` against the device's own clock: `Time::unix_seconds`, from `mind::rtc::unix_time()`.
- **A clock years off,** a flat battery for instance, stops even the connection: pinned TLS checks the leaf's validity against the RTC (`tls/src/pinned.rs`). That part is NET's.

## Plan

1. **The offset.** From the `Date` header of each answer the agent accepts, it keeps the offset: the server's time less the device's. This uses NET's `Head::date`.
   - The offset is kept whole while it is under 2 s, and logged once whenever it changes by more than 60 s: `EFFECTOR: THE DEVICE CLOCK IS N S OFF THE SERVER'S: COMMANDS ARE JUDGED BY THE SERVER'S TIME`.
2. **The session's time.** `time()` gives the session `unix_seconds` plus the offset.
   - Expiry, the ACK's retries past `expires_at` and the journal's eviction then follow the server's clock.
   - The journal's replay protection, one answer per command ID, is unchanged (MC-7.5).
3. **What is not changed:**
   - the device's clock (that is NET's `timed`, 801's S5);
   - the heartbeat's `agent_time`, which keeps saying the device's own, so the server sees the difference.
4. **Tests:**
   - host: the offset from a `Date`, and expiry judged with it;
   - QEMU `effector` suite: the RTC set 3 h ahead (QEMU's `-rtc base=`), a command with a 10-minute expiry is run and answered `completed`, not `command_expired`, and the log names the offset. With NET's TLS part in, the same with the RTC at 2000-01-01.

## Acceptance criteria

- [ ] Host tests as above.
- [ ] The `effector` suite, x86 and aarch64, with the RTC 3 h ahead.
- [ ] The same with the RTC years behind, once NET's TLS part is in.
- [ ] On the DEXP after Windows has run: a command through hesere is answered, with no hand on the clock (MC-12.1).

## Related

[801](801-the-bench-stays-manageable.md), [801-EFF-0028](801-EFF-0028-the-agent-comes-back-by-itself.md), [750-EFF-0006](750-EFF-0006-session-service.md).
