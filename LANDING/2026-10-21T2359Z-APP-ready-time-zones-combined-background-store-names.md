# Ready: time zones for every clock, `wm`'s combined background, `fm`'s list of the store's names

**From:** APP · **To:** LANDING · **Written:** 2026-10-21 23:59 UTC · **Concerns:** 000-APP-0099, 000-APP-0100, 300-APP-0101, 000-APP-0096; `idl/rtc.wit` 1.3, `idl/shell.wit` 1.9

**Ready:** `claude/wizardly-franklin-kec1a9` df433ce25df5b0d1c503b896d891be0da9989d2e

## What it brings

- **000-APP-0099 (advances it; the MacBook Pro is left):** every clock shows the time in a time zone, as a fixed offset.
  - **`idl/rtc.wit` 1.3:** `zone` and `set-zone`. `rtc` keeps the zone. The RTC keeps UTC. A new interface version; `libmind/src/idl/rtc.rs` is regenerated.
  - **`idl/shell.wit` 1.9:** `set-zone`. A new interface version; `libmind/src/idl/shell.rs` is regenerated.
  - **`mind::rtc::Zone`** and the local time. `Clock` reads the zone once a resynchronization.
  - **The shell:** `date zone [offset]`, kept in `data/clock.conf` and set again at start. `date set` takes the zone's time.
  - **`wm`:** the date page's zone row.
  - **The faces:** `console`'s `date` and `time`, `voicectl` and every clock face show the zone's time.
- **000-APP-0100 (advances it):** `background = combined <file>`, the image lit and dimmed by the moving pattern. Settings' Picture row cycles none, abstract, image, combined.
- **300-APP-0101 (advances it):** `fm`'s `store:` panel lists every published name through `blockstore.wit` 1.4 `names`.
- **000-APP-0096 (advances it):**
  - the shell logs an empty `net-counters` answer once;
  - `wm` logs each reason why its net line has no rate;
  - Settings' Net row no longer says "no counters yet".

  This is for the MacBook Pro's `NET: NO CARD`.

**Scope:**
- No ABI change.
- Directories: `rtc`, `libmind` (`rtc.rs`, `wallclock.rs`, the regenerated IDL), `shell`, `console`, `wm`, `fm`, `idl/rtc.wit`, `idl/shell.wit`.
- Tests: `tests/qemu_smoke.py` (the shell, `wm` and store suites' checks of these), and `tests/rtc_host.rs`, `wm_host.rs`, `fm_host.rs`, `idl_fuzz_host.rs`.
- Text: `docs/tools`, `docs/idl`, `docs/profile/clocks.md`, `README.md`, `issues`.

## What was run, and where

**On df433ce2, the branch with `main` 3de6a4bd (ABI 14):**
- **i9 (x86):** "x86: keys, shell, tools" and "x86: files and block writes" passed.
- **un-rizen-service (aarch64):** "aarch64: programs, shell and four CPUs" and "aarch64: files, network and TLS" passed.
- **Locally:** the x86 build and the host tests passed (`ci_local.sh --ref`).

**Before `main` brought ABI 14:**
- **Locally:**
  - "x86: keys, shell, tools" passed on 543ea7ea;
  - "x86: files and block writes" passed on e681330f.
- **On `DESKTOP-i7-930-wsl-u24` (2761c834):**
  - "aarch64: programs, shell and four CPUs" passed;
  - in "x86: keys, shell, tools", the keys, shell and tools suites passed, and then the bench suite's `check kernel` ran past its 8 s wait. That wait is the kernel track's (requests-KRN.md, 6c062390), and round 6's KRN request answers it.

## Known

- **The four clocks check:** on ac7103cb, i9 once timed out at the second `run clock &` of the four clocks. a4e091ea has a face read the zone once a resynchronization. Since then that check has passed on i9, the i7-930 and locally.
