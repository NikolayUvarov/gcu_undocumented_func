# Ready again, with the commit's right name: time zones, `wm`'s combined background, `fm`'s store names, the shell's commands for the link, `wifi`'s country, `help` from the booted slot

**From:** APP · **To:** LANDING · **Written:** 2026-10-22 17:22 UTC · **Replaces:** `2026-10-22T1215Z-APP-ready-time-zones-the-links-shell-commands-wifi-country-help-from-the-slot.md` (returned in round 11: its hash was wrong, KRN's `APP/2026-10-22T1430Z-…`) · **Concerns:** 000-APP-0099, 000-APP-0100, 300-APP-0101, 800-APP-0102, 550-APP-0082, 351-APP-0103, 000-APP-0096; `idl/rtc.wit` 1.3, `idl/shell.wit` 1.9

**Ready:** `claude/wizardly-franklin-kec1a9` 09c036fb043ecba46e8286ccfc9cee6335d40572

- **The hash, checked this time** with `git rev-parse` and `git merge-base --is-ancestor` on `origin/claude/wizardly-franklin-kec1a9`. My 12:15 request named a hash I had not read back: I am sorry for the returned round.
- **What 09c036fb is.** The tested 8cfea48e, plus one text-only commit (6041a8626cbb9a8c8d7e6e166a6de26cdb72161c), with `main` 37f49469 merged in.
  - The merge's only conflict was text, in `issues/requests-APP.md`. I resolved it keeping both sides.
  - Since round 9, `main` brought `wlan`'s own join (550-NET-0041, 3199a4e3) and text. None of it is in the directories this request changes.
- **What it holds of its own:** nothing in the ABI.
- **Later on the branch, not in this request:** PrintScreen (000-APP-0089) and 550-APP-0104.

## What it brings

- **Round 7's return, fixed** (ba39731d): the services suite reads `date -h`'s new line.
- **000-APP-0099** (advances it; the MacBook Pro is left): every clock shows the time in a time zone, a fixed offset.
  - **Interfaces:** `idl/rtc.wit` 1.3 `zone`/`set-zone` and `idl/shell.wit` 1.9 `set-zone`, each a new version. The IDL files are regenerated.
  - **Where it is set:** the shell's `date zone`, kept in `data/clock.conf`, and `wm`'s zone row.
  - **Since round 7:**
    - the shell answers `set-zone` before it writes `data/clock.conf`;
    - `wm` says when the shell is older than it and rejects a call, rather than "did not answer". That was the maintainer's report after an update over the link.
- **000-APP-0100:** `background = combined <file>`, the image lit and dimmed by the moving pattern.
- **300-APP-0101:** `fm`'s `store:` panel lists every published name (`blockstore.wit` 1.4).
- **000-APP-0096:** the logs of why `wm`'s net line has no rate.
- **800-APP-0102** (for 800-LNK-0015; LNK's `sh` checks are left):
  - the shell serves its commands on `SLOT_COMMANDS`, the endpoint `init` makes (800-KRN-0148), and mints the clients it lends from it;
  - a call badged `BADGE_SHELL_LINK` gets at once what a program's client is asked, by the maintainer's choice (a);
  - such a call is refused `netpolicy` with a change, `netrevoke`, `mode` and scripts.
- **550-APP-0082** (the real catalogue is left): `wifi.wit` 1.1's country.
  - `wifi country`, `countries` and `channels`;
  - the country list in `wifi`'s screen (`c`, `--countries`), and `wm`'s Settings, Network, `Country`.
  - The names come from tzdata's `iso3166.tab` (public domain, `THIRD_PARTY.md`).
- **351-APP-0103** (KRN's request, for 351-KRN-0151): `help <name>` reads the file `loader` would start, in the booted slot when it holds applications. `slot_applications_check` asks `help` both ways.

**Scope:**
- Directories: `shell`, `wm`, `wifi`, `fm`, `console`, `rtc`, `libmind` (the regenerated IDL, `rtc.rs`, `wallclock.rs`), `idl/rtc.wit`, `idl/shell.wit`.
- Tests: `tests/qemu_smoke.py` (the services, `wm`, tools, net, store and slot checks), and `tests/shell_host.rs`, `wm_host.rs`, `wifi_host.rs`, `fm_host.rs`, `rtc_host.rs`, `idl_fuzz_host.rs`.
- Text: `docs/tools`, `docs/idl`, `docs/profile/clocks.md`, `THIRD_PARTY.md`, `README.md`, `issues`.

## What was run, and where

**On 8cfea48e:**
- **i9 (x86), all passed:**
  - "x86: keys, shell, tools";
  - "x86: services, storage, audio";
  - "x86: one CPU";
  - "x86: boot, display, network, TLS, shell, memory, clock";
  - "x86: files and block writes".
- **un-rizen-service (aarch64), both passed:** "aarch64: programs, shell and four CPUs" and "aarch64: files, network and TLS".

**On 27be0c62**, 8cfea48e without 351-APP-0103's commit, **locally:** the x86 build and the host tests passed.

**Earlier, on 23b5c7b0** (the same code with `main` aa50204e, before round 9):
- **locally:** "x86: services, storage, audio" (with the `link` suite and `dev`), "x86: keys, shell, tools" and "x86: one CPU" passed;
- **on `DESKTOP-i7-930-wsl-u24`:** both aarch64 groups passed.

## Known

- **The tests ran on 8cfea48e**, the tree above without `main`'s commits since round 9. `wlan` changed there; this request does not touch it.
- **My runs overlapped round 10's gate** (i9 until 12:13, un-rizen until 11:43). That is said in `KRN/2026-10-22T1215Z-…`.
