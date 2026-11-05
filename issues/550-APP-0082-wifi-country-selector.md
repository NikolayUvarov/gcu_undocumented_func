# 550-APP-0082 — The Wi-Fi country and its channels in `wifi` and `wm`'s Settings

**Type:** tools (`wifi`, `wm`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open: the commands, the screen's list and the country's origin done (2026-11-05); the MacBook Pro left · **Blocked by:** — (`idl/wifi.wit` 1.2 and the station's catalogue, 550-NET-0035 stage B, are in `main`) · **Main task:** [550](550-network-on-real-hardware.md) · **Roadmap:** tracks G, H · **Constitution:** MC-2.3, MC-11.5, MC-12.3, MC-12.4

**Recorded by:** ASTRA auditor-fixer, 2026-10-16, in `requests-APP.md`, at the maintainer's request. Numbered by the tools track on 2026-10-20. The radio's catalogue is 213-DRV-0032's; the station's interface is the network track's.

## Problem

The maintainer chose the country for their two machines (Uzbekistan, `UZ`) and asked to switch among all the countries of the verified regulatory database. `wifi` and `wm`'s Settings have no country, and `idl/wifi.wit` 1.0 has none to set.

## Plan

Once the network track's interface has the country:

- **The list.** Every country of the catalogue, with bounded paging and search, by its two-letter code and a readable name, and the world domain `00` apart from a chosen country.
  - The channels a country allows come from the station's answer, never from a table in the tool.
  - A country the radio can use no channel of stays in the list, and says so.
- **Asked and applied, apart.**
  - The tool shows the country asked for and the one the radio runs with.
  - A change that needs the link to drop says so before it is made.
  - A change that fails says why and is not shown as applied.
- **Kept.** The station keeps the choice through its owner (the two-letter code, not a row), and `wifi` reads it back at start; a missing or unknown one is named.
- **While scanning and joining** the country stays on view.
- **Authority.** Only the change client (`BADGE_WIFI_CHANGE`, or a narrower right the interface defines) sets it; a read-only client is told why not (MC-11.5).
- **Where.**
  - `wifi`: a key opens the country list.
  - `wifi country [<code>]` without a screen (550-APP-0078's form) for scripts.
  - `wm`'s Settings, Network: the country beside Wi-Fi.
- **`--demo`** shows the list against the stand-in. It never claims a radio applied a country.

## Acceptance criteria

Against the agreed interface, and its stand-in in the tools suite:

- a country is chosen, and it is read back after a restart;
- the whole catalogue is browsable through bounded paging and search;
- a read-only client is refused;
- no device, and a country with no usable channel, are named;
- a failed change is shown as failed;
- scan, join and leave work as before.

## Progress (2026-10-22): stage 1, without a screen

- **The interface:** `idl/wifi.wit` 1.1 (550-NET-0035), in `main`. NET's questions of 2026-10-20 are answered on `internal-dev-messages`: 1.1 gives what this task needs.
- **`wifi`'s `Station`** has 1.1's four calls:
  - `countries(from)` and `channels(code)`;
  - `country()` and `set-country(code)`.

  A station of 1.0's calls alone answers as one without a catalogue. The service client maps them, and the stand-in has a small catalogue: `DE`, `JP`, `US`, `UZ`, and `XX`, which its radio refuses.
- **Without a screen** (550-APP-0078's form):
  - `wifi country`: asked for and applied apart, the radio's confirmation, `keystore`'s keeping, the catalogue;
  - `wifi country <code>`;
  - `wifi countries [<text>]`: paged, 32 a page, searched by code or name;
  - `wifi channels [<code>]`: under the country applied, else `00`.

  No catalogue, a read-only client, a country with no usable channel and a failed change are each named.
- **The names:** from tzdata's `iso3166.tab` (public domain, `THIRD_PARTY.md`) in `wifi/src/countries.rs`, since the catalogue has codes only.
- **Tests:**
  - host, `tests/wifi_host.rs`, against the stand-in;
  - the `net` suite, against `wlan` built without a catalogue: `NO CATALOGUE IN THIS STATION`.
- **The screen** (stage 2, the same day):
  - the line above the notice names the country;
  - `c` opens the catalogue's list, found by code or name as it is typed;
  - Enter asks for the country chosen; a failed one is shown as failed;
  - a station without a catalogue says so and stays in the list.

  Tested by a host test, and by the tools suite's `wifi --demo`: UZ found and applied.
- **`wm`'s Settings, Network** (stage 3, the same day): a `Country` row beside `Wi-Fi` starts `wifi --countries`, the screen opened at the country list.
  - `wifi --countries` alone, or with `--demo`; with a command it is a usage error.
  - Tested by host tests (`tests/wm_host.rs`: the row and what Enter and a click start; `tests/wifi_host.rs`: the option and the list at start).
- **Left:** the real catalogue (stage B) and the MacBook Pro.

## Progress (2026-11-05): where the country applied came from, in `wifi` and in `wm`

**What asked for it:** the maintainer's decision of 2026-11-04 (NET, `APP/2026-11-04T1003Z-NET-…` and `…1335Z-NET-…`): "Everything must work, not go quiet."
- With no country chosen, the station applies the one the access points announce, else `00`, and always says which and why.
- `wifi.wit` 1.2's `country-origin` (NET, in `main`) gives the reason: `none`, `chosen`, `detected` (with how many of the access points that named a country named this one), `world`.

**Made:**
- **`wifi`'s `Station`** carries the origin in `CountryStatus`. The service asks `country-origin` after `country` and after `set-country`.
- **The stand-in** does as the station does:
  - before its first scan, none;
  - then UZ, announced by 3 of its 4 access points that name a country;
  - with `beacons` off, `00`;
  - a chosen country is not moved by a scan, and a refused one is not replaced.
- **`wifi country`** gives the reason in the station's own words (its log's): `NONE CHOSEN, ANNOUNCED BY 1 OF THE 1 ACCESS POINTS THAT NAMED A COUNTRY`, `NONE CHOSEN, AND THE ACCESS POINTS ANNOUNCED NONE THE CATALOGUE HAS: THE RULES EVERY COUNTRY ALLOWS`, `CHOSEN BY THE USER, KEPT BY KEYSTORE`, and before the first scan `NONE YET`.
  - The `00` line covers both of the station's cases: none announced, and one announced that the catalogue does not have.
  - The old `NONE ASKED: THE STATION PROBES NOWHERE AND JOINS NOTHING` is gone, as the station no longer waits.
- **The screen's line** says the same within 76 columns (`Country: UZ (Uzbekistan), announced by 3 of 4 access points (c: choose one)`), and its status line adds `ORIGIN=`.
- **`wm`'s Settings, Network** shows the station's country and why below its rows.
  - It reads `country` and `country-origin` through `wm`'s copy of the shell's client every 2 s while the page shows, bounded to 500 ms. It only reads.
  - It logs a change on serial: `[WM] NETWORK PAGE: …`.
  - Without a station it says `no station to ask`.

**Tests:**
- **Host:** `tests/wifi_host.rs` (20), the lines of every origin, the stand-in's way and the screen's widths; `tests/wm_host.rs` (47), the page's line for each case.
- **QEMU:**
  - the `net` suite's `wifi country` checks, on the station's three cases: detected UZ, chosen US and `00`;
  - `wm` over `wlansim`, Settings' network page naming UZ as announced by 1 of 1 access points;
  - the `wm` suite's page without a station.

## Related

[550-APP-0033](550-APP-0033-wifi-setup-program.md) (`wifi`), [550-APP-0078](550-APP-0078-wifi-join-without-a-screen.md) (`wifi` without a screen), 213-DRV-0032 (the radio's catalogue), [requests-NET.md](requests-NET.md).
