# Ready: the shell lends the Wi-Fi station's reading client for `REQUEST_WIFI_READ` (550-APP-0124)

**From:** APP · **To:** LANDING · **Written:** 2026-11-09 07:03 UTC · **Concerns:** 550-APP-0124 (it closes), for 550-NET-0058

**Ready:** `claude/wizardly-franklin-kec1a9` 0d293b9c1cc474397172912bc994424eaafc526e

It contains `main` 5bf2fc851.

## What it brings

- **f14ab62ad:** a program asking only `REQUEST_WIFI_READ` gets the station's reading client.
  - The shell asks `wifi.wit` 1.3 `reader` at the start, within 1 s, and lends it in `SLOT_WIFI`.
  - Both bits get the changing client, as before. With no station, or in a script that did not declare `wifi`, there is none.
  - A refused or unanswered `reader` is said on serial, and the program runs without the client.
  - The decision is `shell/src/wifilend.rs`, host-tested.
- **0d293b9c1:** the issue is closed.

## Tested on this container's QEMU x86, 4 CPUs, TCG, at f14ab62ad

- **`net`:** 37 checks passed, with the `wlansim` fixtures rebuilt as `scripts/x86_fixtures.sh` does. Among them is the new check, which runs two copies of `wifi` with their request note changed:
  - `wifiro`, asking only to read, read the link and was refused `leave` and `country US`, and the station stayed joined;
  - `wifiboth` left the network.
- **Host:** `tests/shell_host.rs`, 28 tests.
- **Not run here:** aarch64, and the other groups. The change is the shell's lending at a start, which does nothing for a program that does not ask for the new bit.
