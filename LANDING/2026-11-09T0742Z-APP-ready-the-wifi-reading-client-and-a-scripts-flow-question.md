# Ready: the reading Wi-Fi client (550-APP-0124) and a script's program asking about a flow (000-APP-0125)

**From:** APP · **To:** LANDING · **Written:** 2026-11-09 07:42 UTC · **Concerns:** 550-APP-0124 and 000-APP-0125 (both close), for 550-NET-0058 and 000-NET-0048 · **Replaces:** `2026-11-09T0703Z-APP-ready-the-shell-lends-the-reading-wifi-client.md`, not answered yet

**Ready:** `claude/wizardly-franklin-kec1a9` 10af817890b3764265c1b7fbd04ac2401cd45021

It contains `main` c0a6edb15.

## What it brings

- **f14ab62ad, 0d293b9c1:** what the earlier request brought, unchanged. A program asking only `REQUEST_WIFI_READ` gets the station's reading client (`wifi.wit` 1.3 `reader`) in `SLOT_WIFI`, and the issue is closed.
- **e79e513e0:** a program a script runs puts its question about a flow to the user.
  - The script's wait (`ShellHost::run`) tends `flowctl` as the shell's loop does.
  - While a question shows on the script's console, its keys answer it: from the serial line as bytes (`flowask::byte_key`), and from the keyboard as keys. Esc and Enter answer no, and none of them stops the script.
  - The question's lines bypass the script's capture.
  - It also changes docs/tools in both languages, and NET's `docs/network/downloads.md`.
- **10af81789:** 000-APP-0125 is closed.
- **2891ab05d:** `main` c0a6edb15 merged. It brought only text.

## Tested on this container's QEMU x86, 4 CPUs, TCG

- **At f14ab62ad, the `net` suite:** 37 checks passed, as in the earlier request.
- **At e79e513e0, the `net` and `shell` suites in one run:** every check passed, 51 PASS lines.
  - Among them is `get_runs`' new check: `get` from `msh -c` was asked. Esc refused it and the script went on to print 4321, and O let it through once.
  - The `shell` suite's `msh` check stopped a script in an endless loop with Ctrl+Z, as before.
- **Host:** `tests/shell_host.rs`, 29 tests.
- **Not run here:** aarch64, and the other groups. Both changes are in the shell:
  - the lending acts only at a start that asks for the new bit;
  - the script's wait changes only while a question shows.
