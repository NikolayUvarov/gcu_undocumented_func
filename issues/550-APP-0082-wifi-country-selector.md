# 550-APP-0082 — The Wi-Fi country and its channels in `wifi` and `wm`'s Settings

**Type:** tools (`wifi`, `wm`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open: stage 1, the commands without a screen, on the tools branch (2026-10-22) · **Blocked by:** — (`idl/wifi.wit` 1.1 is in `main`; the station's catalogue, 550-NET-0035 stage B, for the real answers) · **Main task:** [550](550-network-on-real-hardware.md) · **Roadmap:** tracks G, H · **Constitution:** MC-2.3, MC-11.5, MC-12.3, MC-12.4

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
- **Left:**
  - the screen's key for the country list, and `wm`'s Settings row;
  - the real catalogue (stage B) and the MacBook Pro.

## Related

[550-APP-0033](550-APP-0033-wifi-setup-program.md) (`wifi`), [550-APP-0078](550-APP-0078-wifi-join-without-a-screen.md) (`wifi` without a screen), 213-DRV-0032 (the radio's catalogue), [requests-NET.md](requests-NET.md).
