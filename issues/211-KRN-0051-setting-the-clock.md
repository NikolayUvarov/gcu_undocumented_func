# 211-KRN-0051 — Setting the clock: `rtc.wit` 1.2 `set`, on the CMOS RTC and the PL031

**Type:** kernel (the `rtc` service) · **Owner:** `KRN` · **Priority:** P2 · **Status:** in progress (the service's part done and host-tested; `date set` waits for `APP`) · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Constitution:** MC-5.6, MC-3.11

## Problem

The maintainer asked on 2026-10-09 for a way to set the time. What exists:
- **The kernel** keeps only a monotonic clock (`SYSCALL_UPTIME`, `SYSCALL_CLOCK`). It has no calendar time to set, and needs none (MC-5.6: calendar time is the `rtc` service's).
- **`rtc`** reads the CMOS RTC (x86) or the PL031 (aarch64). `idl/rtc.wit` 1.1 has `now` and `date`, nothing to write.
- **The programs.** The shell's `date` prints the RTC's time without a time zone. Nothing sets it.

On the MacBook Pro the system log began `STARTED 2022-01-01 04:28:29 BY THE MACHINE'S CLOCK` (LOG:boot0002.log), so its clock is wrong or read wrongly. macOS keeps the RTC in UTC.

## Plan

- **`rtc.wit` 1.2** adds `set: func(date: u32, seconds: u32) -> result<_, error>`, where `date` is days since 2000-01-01 and `seconds` since midnight. `enum error { rights, invalid, unavailable }`.
  - Only a client with the setting badge may call it. init gives that client to the shell; the standard grant `SLOT_RTC` stays read-only.
- **The CMOS RTC.**
  - Writes with status B's SET bit held (updates stop meanwhile), in the mode status B names (BCD or binary, 12 or 24 hours).
  - The year in register 9; the century register only where the FADT names one.
- **The PL031.** The seconds since 1970 into its load register (RTCLR, offset 0x08).
- **Reading on the Mac.** Check the CMOS read against the firmware's `GetTime` (the hardware report could carry both) before writing anything there.
- **The user's command** is `APP`'s: `date set YYYY-MM-DD HH:MM[:SS]` in the shell ([requests-APP.md](requests-APP.md)). Time zones and network time are later work: a time zone setting for `date`, and NTP over `netstack` (`NET`).

## Acceptance criteria

- **QEMU, x86 and aarch64:** `date set 2030-05-17 12:34:56`, then `date` shows that time, still running. A program without the setting badge is refused with `rights`.
- **The MacBook Pro:** after `date set`, the next boot's log starts at the right date.

## Progress

**2026-10-09: the service's part.**
- **The interface.** `idl/rtc.wit` 1.2 has `set` and its `error`.
- **The service.** `rtc` refuses `set` with `rights` to a client without `mind::rtc::BADGE_SET`. init gives the shell its `SLOT_RTC` client with that badge; programs keep the plain client from the loader. The service logs who set the clock.
- **The writes.**
  - CMOS: registers written with status B's SET bit held, in its mode, the weekday included; years 2000 to 2099 only.
  - PL031: the load register.
  - Either way the date is read back, and `unavailable` answers a clock that did not take it.
- **The codec.** It lives in `rtc/src/cmos.rs`. `tests/rtc_host.rs` round-trips every 7th second of the day in BCD and binary, 12 and 24 hours, and checks the date registers (weekday, BCD, the year range).
- **Left.** `APP`'s `date set`, then the QEMU check of the acceptance criteria on x86 and aarch64, then the MacBook Pro.

## Related

[202](../issues-done/202-aarch64-devices.done) (the PL031), `rtc/src/main.rs`, `idl/rtc.wit`, `libmind/src/rtc.rs`.
