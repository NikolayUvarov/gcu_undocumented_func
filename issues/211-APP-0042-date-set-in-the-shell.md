# 211-APP-0042 — `date set` in the shell

**Type:** tools (`shell`, `libmind`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) (for [211-KRN-0051](211-KRN-0051-setting-the-clock.md)) · **Roadmap:** track G · **Constitution:** MC-5.6, MC-3.11

Numbered from the kernel track's request in `requests-APP.md` (2026-10-09, at the maintainer's request: a way to set the time; the MacBook Pro's clock reads 2022-01-01).

## Problem

`date` prints the RTC's time and nothing sets it. `rtc.wit` 1.2 adds `set` for the client with the setting badge, which `init` gives the shell (211-KRN-0051).

## Plan

- `date set YYYY-MM-DD HH:MM[:SS]` calls `rtc::set` through the shell's client and prints the time read back. It says that the clock keeps no time zone.
- The text is parsed in `libmind` (`mind::rtc::parse_setting`), host-tested: a date that does not exist, one outside 2000–2099 (what `set` takes) and a malformed time are refused before the service is asked.
- The shell's help and `docs/tools` (EN, RU) name it.

## Acceptance criteria

- **Host test** (`tests/rtc_host.rs`): settings parsed and refused.
- **The `shell` suite**, on x86 and aarch64: sets a date, reads it back with `date`, is refused an impossible date, and sets the host's time back.

## Related

[211-KRN-0051](211-KRN-0051-setting-the-clock.md), [000-APP-0012](../issues-done/000-APP-0012-clocks-read-the-rtc-once-a-minute.done).
