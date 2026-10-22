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

## Progress (2026-10-21)

- **Done:**
  - `rtc.wit` 1.3: the zone kept by `rtc`;
  - `mind::rtc::Zone` and the local time;
  - `date zone`, kept in `data/clock.conf`;
  - `shell.wit` 1.9 `set-zone`;
  - the date page's zone row;
  - every clock face in the zone's time.
- **Test runs:**
  - On ac7103cb, i9 and un-rizen-service each timed out once:
    - i9: at the second `run clock &` of the four clocks;
    - un-rizen-service: at `check kernel` in the bench suite.
  - a4e091ea made a face read the zone once a resynchronization, not each second.
  - Since then, every run passed but one:
    - locally, "x86: keys, shell, tools" on a4e091ea and on 543ea7ea;
    - on `DESKTOP-i7-930-wsl-u24`, "aarch64: programs, shell and four CPUs" on 2761c834;
    - also on the i7-930, "x86: keys, shell, tools" on 2761c834: the keys, shell and tools suites, the four clocks among them, then the bench suite's `check kernel` ran past its 8 s. That wait is the kernel track's, recorded in `requests-KRN.md` (6c062390).
- **Left:** the MacBook Pro.

## Progress (2026-10-22): "shell did not answer" on the MacBook Pro

- **The maintainer's report:** after an update, setting the zone from `wm`'s Settings says the shell does not answer. Setting the time works.
- **The cause,** found by LNK (`ALL/2026-10-22T0428Z-LNK-an-update-through-the-link-leaves-the-applications-behind.md`):
  - the update through the link replaced the boot services, the shell among them, with a release's older ones (`shell.wit` 1.8, `rtc.wit` 1.2);
  - `wm`, an application at the volume's root, stayed at 1.9.
  - The older shell rejected `set-zone` as a call it does not know: it answers a rejected request since 211-APP-0044.
  - `wm` called every failure of the call "does not answer".
  - The gap between releases is UPD's and KRN's to close.
- **What changed here:**
  - `wm` tells a rejection from a timeout: "the shell does not know set-zone, so it is older than this wm (shell.wit 1.9)". The same goes for `set-clock` (1.2) and `new-window` (1.8). It waits 8 s for `set-clock` and `set-zone`.
  - The shell answers `set-zone` once the rtc service has the zone, and writes `data/clock.conf` after its answer, logging `[SHELL] ZONE …, KEPT IN data/clock.conf`. So a slow disk does not hold `wm`.
  - The `wm` suite checks the kept line.
- **On the Mac:** LNK puts a release with the 1.9 shell and the 1.3 rtc; the zone is to be set again there.

## Related

[211-KRN-0051](211-KRN-0051-setting-the-clock.md) (`rtc.wit` 1.2 `set`), [211-APP-0042](../issues-done/211-APP-0042-date-set-in-the-shell.done) (`date set`), [000-APP-0055](000-APP-0055-date-and-time-from-wms-settings.md) and [000-APP-0060](../issues-done/000-APP-0060-the-clock-from-wms-settings-at-once.done) (the date page), [000-APP-0012](../issues-done/000-APP-0012-clocks-read-the-rtc-once-a-minute.done) (`Clock`).
