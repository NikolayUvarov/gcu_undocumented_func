# 000-APP-0099 — Every clock shows the time in a time zone

**Type:** tools (`shell`, `wm`, `console`, `rtc`, interface) · **Owner:** tools track (`APP`) · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** — (follows [211-KRN-0051](211-KRN-0051-setting-the-clock.md), "Time zones … are later work") · **Roadmap:** track D · **Constitution:** MC-5.6, MC-12.3, MC-12.4

**Recorded by:** the tools track, 2026-10-21, from the maintainer's report: "часы при отображении времени не учитывают таймзону" (the clocks do not take the time zone into account). The maintainer chose, when asked:
- the zone kept by the `rtc` service through `rtc.wit` 1.3, the change made by this track;
- a fixed offset for now, without summer time.

## Problem

- **The system has no time zone.** The `rtc` service answers what the CMOS clock or the PL031 holds, and every clock shows it as it is: `clock`, `dzen-clock`, `wm`'s background and its Settings, the shell's `date` and `time`, `console`'s. `date` says `(RTC, NO TIME ZONE)`.
- **The MacBook Pro's RTC holds UTC**, as macOS keeps it, so the clocks show UTC there. The PL031 counts UTC by definition.
- **What reads the RTC for time stamps wants UTC:** TLS's certificate times, `keystore`, `updater` (`mind::rtc::unix_time`). Shifting the RTC itself to local time would break them.

## Plan

- **`idl/rtc.wit` 1.3:**
  - `now` and `date` are UTC, the time the RTC keeps;
  - `zone` answers the zone the clocks show, a `record offset { west, minutes }` (the IDL has no signed integers);
  - `set-zone` sets it, for the setting badge's client alone (the shell's), `invalid` beyond UTC-12:00 and UTC+14:00;
  - the service keeps the zone while it runs, UTC until set.
- **`libmind/src/rtc.rs`:**
  - `Zone` parses `+03:00`, `-5:30`, `UTC+3`, `UTC`, names itself `UTC+03:00`, and turns UTC's day and second into the zone's and back;
  - `local_now` and `local_date_time` read the RTC once with the zone;
  - `Clock`, which the clock faces use, shows the zone's time and date and reads the zone with the RTC, so a new zone shows within a minute;
  - `configured_zone` reads `data/clock.conf`'s line `zone <offset>`.
- **The shell:**
  - `date zone [offset]` shows or sets the zone and keeps it in `data/clock.conf`;
  - at its start the shell gives the file's zone to `rtc`, before the start script;
  - `date` and `time` show the zone's time, `date` with the zone's name;
  - `date set` takes the zone's time and sets the RTC to UTC.
- **`idl/shell.wit` 1.9:**
  - `set-zone` sets the zone as `date zone` does: at once for the window manager the shell started, after the user agrees in the shell's window for any other client;
  - `set-clock`'s fields are the zone's time.
- **`wm`'s Settings, the date and time page:**
  - a `Time zone` row, ← → 30 minutes a step, set at once through the shell;
  - the page's line names the zone;
  - the background's clock follows the zone at once.
- **`console`'s `date` and `time`** and the voice's spoken time show the zone's time.
- **Not changed:**
  - `vfs_server`'s file times stay UTC; they are the kernel track's.
  - `bench`'s report header stays UTC; it is the kernel track's.
- **Docs:** `docs/tools` (EN, RU), `docs/idl`, the README, `docs/profile/clocks.md` (what is not provided: summer time, a zone database, an RTC that keeps local time).

## Acceptance criteria

- **Host tests:**
  - `tests/rtc_host.rs`: zones parsed, refused and named; the zone's time across midnight both ways and back to UTC; `Clock` showing the zone's time and date; `data/clock.conf`'s line;
  - `tests/wm_host.rs`: the zone row's steps, ends and refusal without the shell's commands;
  - `tests/idl_fuzz_host.rs` with `rtc.wit` 1.3's `Offset`.
- **The `shell` suite** (x86, aarch64):
  - `date zone` says UTC; `date zone +03:00` shifts `date` by three hours and names the zone;
  - `data/clock.conf` holds it and `rtc` logs who set it;
  - `date set 2031-05-17 09:00:00` at UTC+03:00 reads back 09:00 there, 00:30 at UTC-05:30 and 06:00 at UTC;
  - wrong zones are refused; back to UTC.
- **The `wm` suite** (x86): the date page's zone row sets UTC+00:30 and UTC through the shell, at once, and the page names it.
- **The MacBook Pro:** with `date zone` set to the maintainer's zone, the clocks show local time. This is QEMU's RTC until then (MC-12.1).

## Related

[211-KRN-0051](211-KRN-0051-setting-the-clock.md) (`rtc.wit` 1.2 `set`), [211-APP-0042](../issues-done/211-APP-0042-date-set-in-the-shell.done) (`date set`), [000-APP-0055](000-APP-0055-date-and-time-from-wms-settings.md) and [000-APP-0060](../issues-done/000-APP-0060-the-clock-from-wms-settings-at-once.done) (the date page), [000-APP-0012](../issues-done/000-APP-0012-clocks-read-the-rtc-once-a-minute.done) (`Clock`).
