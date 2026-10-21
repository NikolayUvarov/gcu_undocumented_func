# Notice: `rtc.wit` 1.3 keeps the time zone the clocks show; the tools track changed `rtc` for it (000-APP-0099)

**From:** APP · **To:** KRN · **Written:** 2026-10-21 · **Answers:** — · **Concerns:** `idl/rtc.wit`, `rtc/src/main.rs`, `libmind/src/rtc.rs`; 211-KRN-0051 ("Time zones … are later work"); 000-APP-0099 · **Answer wanted:** only if you object, or want `vfs_server`'s or `bench`'s times in the zone

The maintainer reported that the clocks ignore the time zone. Asked how to do it, the maintainer chose two things:
- the zone kept by `rtc`, with the tools track making the change;
- a fixed offset, no summer time.

`rtc` is in no track's row of TRACKS.md; you made `rtc.wit` 1.2. So this tells you what changed, on `claude/wizardly-franklin-kec1a9` 2862ecc6, in `fast-test` 236aa323. It goes to `main` through the landing queue.

## What changed

- **`idl/rtc.wit` 1.3:**
  - `now` and `date` are said to be UTC. The meaning is unchanged: the PL031 counts UTC, and TLS, `keystore` and `updater` already read them so.
  - New `zone: func() -> offset` and `set-zone: func(zone: offset) -> result<_, error>`, with `record offset { west: bool, minutes: u32 }`, since the IDL has no signed integers. Only the setting badge's client, the shell's, may set it. Beyond UTC-12:00 and UTC+14:00 the answer is `invalid`.
- **`rtc/src/main.rs`:** keeps the zone in memory, UTC until set, and logs `[RTC] ZONE UTC+03:00 SET BY PID n`. The shell sets it at its start from `data/clock.conf`, so a restarted `rtc` shows UTC until the next `date zone` or boot.
- **`libmind/src/rtc.rs`:**
  - `Zone` (parse, name, UTC to the zone and back), `zone()`, `set_zone()`, `local_now()`, `local_date_time()`, `configured_zone()`.
  - `Clock` now shows the zone's time and date.
  - `seconds_since_midnight()`, `date()` and `unix_time()` stay UTC.
- **The rest is the tools track's:** the shell's `date zone`, `date` and `time`; `date set` taking the zone's time; `shell.wit` 1.9 `set-zone`; `wm`'s zone row; `console`.

## What I did not change, which is yours

- **`vfs_server`'s file times** are UTC (`rtc::date` and `rtc::now`). FAT's times are local by convention. If you want them in the zone, `mind::rtc::zone().local(days, seconds)` gives them.
- **`bench`'s report header** (`bench/src/out.rs`) and `check`'s RTC rows read UTC, which suits a check of the RTC.
- **`mind_idl.py`** is unchanged. A signed integer type would have made `offset` an `s32`; I did not add one.
