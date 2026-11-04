# 550-NET-0035 — The station's country: chosen from the whole regulatory catalogue, kept, applied by the radio

**Type:** network (service and interfaces) · **Owner:** `NET` track · **Priority:** P1 · **Status:** open (stage A built) · **Blocked by:** — (stage B's catalogue, DRV's `regdb`, is in `main` since round 79) · **Main task:** [550](550-network-on-real-hardware.md) · **Roadmap:** D / 550, H / 213 · **Constitution:** MC-2.3, MC-3.3, MC-6.1, MC-12.1, MC-12.3, MC-12.4, MC-12.7

From the request "Wi-Fi country selection from the complete verified regulatory catalog" in requests-NET.md (ASTRA auditor-fixer, 2026-10-16, at the maintainer's request). `APP`'s side is [550-APP-0082](550-APP-0082-wifi-country-selector.md), the RTL8852BE catalogue 213-DRV-0032 (ASTRA).

Numbered 0035, after [351-NET-0034](../issues-done/351-NET-0034-the-download-check-flushed-bounded-and-logged.done): ASTRA's tasks 550-NET-0031 to 0033, on `fast-test` since 2026-10-15, hold the counters before them.

## Problem

- **What the maintainer asked for.**
  - The two machines are in Uzbekistan (`UZ`).
  - The user switches among every country of the verified regulatory database, not a shortlist.
- **What the station does now.**
  - It knows no country.
  - It probes on channels 1 to 11 ("which every domain allows") and listens elsewhere.
  - It joins an access point on any channel the radio has.
  - `idl/wifi.wit` 1.0 has no country.
- **The maintainer's decision of 2026-11-04** replaces the first rule below ("no default"): "if there is no country, go through UZ, US, RU and the other countries. Everything must work, not go quiet." With no country chosen, the station works as follows:
  1. **It listens first.** On every channel, without sending, it reads the Country element of the access points' beacons (802.11d). A code that is in the catalogue is applied as **detected**, shown apart from a chosen one.
  2. **If none is announced,** it applies `00`, the catalogue's rules common to every country (UZ, US, RU and the rest):
     - it probes and joins on channels 1 to 11;
     - elsewhere it listens, and joins an access point it heard. So a network on any channel can still be joined.
  3. **It is never silent.** `wifi` and the log say what is applied and why: chosen, detected from access points (with how many announced it), or `00` because none was chosen or announced.
  4. **A chosen country** always comes first. A detected one never replaces it.
- **Not done this way:** applying UZ, US, RU and the others one after another and sending under each in turn. Sending under another country's rules may be unlawful where the machine is. `00` is the subset every country allows, and with the beacons' country it reaches the same networks.
- **What must hold** (the request), the first rule replaced as above:
  - ~~no default: a missing, unknown or unapplied country is never DE, US, the world domain or permission to send~~ (replaced 2026-11-04);
  - a two-letter code is kept, never a row index;
  - a change of country leaves nothing chosen under the old rules: scan, join or the radio's limits;
  - what was asked for and what the radio applied are shown apart;
  - the channels come from the country's rules intersected with what the radio can do;
  - restrictions the station does not keep are shown, not dropped.

## Plan

### Where things live

- **The catalogue.** ASTRA generated it from wireless-regdb at a pinned commit (`rtw89_wifi/src/regulatory.rs` and its tables on `fast-test`, 213-DRV-0032): 181 entries and `00`, with provenance.
  - The station needs the same catalogue as the radios, not a copy. It is asked from `DRV` as a crate of its own, `regdb` (requests-DRV.md), which `rtw89_wifi`, `bcm_wifi` and `wlan` all use.
  - Until that crate is on `main`, the station has no catalogue (stage A).
- **The rules per channel** are the station's, as `wlanhw.wit` 1.0 already says ("the station decides which channels are allowed where it is"). The radio applies power limits under its own.
- **The choice is kept by `keystore`.** It sits beside the remembered networks, in the private directory only `keystore` writes, for the station's badge alone (keystore.wit 1.3).
  - No program can change the country but the station, and the station only on a change client's word.
  - This needs nothing new from `init`: the station already holds `keystore`'s client.
  - The alternative, a configuration directory lent by `init` (requests-KRN.md, the RTL8852BE endpoint), is not needed for this.

### `idl/wifi.wit` 1.1 (for `APP`'s 550-APP-0082)

- **`countries(from)`:** a page of up to 32 countries from index `from`, and the total. Each country has its code and how many of the radio's channels it lets the station send on, and only listen on. `00` is not in the list: it is never a chosen country.
- **`channels(code)`:** the radio's channels under a country (`00` too, for information). Each channel has its standing and the most EIRP allowed there. The standings:
  - `send`: the station may probe and join there;
  - `listen`: no initiating radiation (NO-IR) or radar detection required (DFS). The station listens there, and joins an access point it heard;
  - `barred`: outside the rules, or a restriction the station does not keep: indoor or outdoor only, one modulation alone, point-to-point;
  - `radio`: the rules allow it, the radio does not.
- **`country()`:** the status:
  - the code asked for and the code applied, kept apart;
  - the state: `none`, `applied` or `failed`;
  - whether the radio confirmed its limits;
  - whether `keystore` keeps the choice;
  - the catalogue's identity: the first 16 hex digits of its `db.txt` SHA-256.
- **`set-country(code)`, change client only.** In this order:
  1. the station leaves the network and drops the last scan;
  2. it has the radio apply the limits;
  3. it has `keystore` keep the code;
  4. it answers the status.
- **Errors.** No new variant, so a 1.0 client still decodes every answer:
  - `invalid`: not two capital letters, or `00`;
  - `not-found`: not in the catalogue;
  - `denied`: a read-only client;
  - `no-device`: no radio yet;
  - `unsupported`: no catalogue in this build.
- **With no country chosen** the station follows the maintainer's decision of 2026-11-04 above: the beacons' country, else `00`. `country()` names which, and why. `join` is never refused for want of a country.
- **If the radio refuses the limits:** the state is `failed` and nothing is applied, so the station only listens. It does not fall back to the country it had: the user may have moved.

### `idl/wlanhw.wit` 1.2 (agreed with `DRV`, 2026-10-20)

`bcm_wifi` (`DRV`) and `rtw89_wifi` (ASTRA) serve `wlanhw.wit` with a `match` over every request, so a new request breaks their build until they answer it. `DRV` agreed (`NET/2026-10-20T0648Z-DRV-...`): it answers `set-country` in `bcm_wifi` when it merges 1.2, at first with `failed` (the radio then neither tunes nor sends). `rtw89_wifi` answers `failed` too from the commit that merges 1.2 with it on `main`, until ASTRA applies limits.

- **`set-country(code, dfs, limits)`:** for each channel the station may use, the most EIRP and whether it may only listen there. The radio:
  - applies them under its own limits (board, calibration, platform);
  - answers its `info` after, whose `channels` are then the ones it tunes to.
- **Errors.** `invalid`: a malformed list. `failed`: the radio cannot apply the limits, and then it sends nothing.
- **A 1.2 radio with no limits set sends nothing; it still listens.**
- **A 1.1 radio rejects the call.** The station then keeps to the channels alone, and `country()` says the radio did not confirm.

### `idl/keystore.wit` 1.3

- `wifi-country()` and `wifi-keep-country(code)`, for the station's badge.
- The code is two capital letters, kept in the private directory. Empty: none kept.

### Stages

- **Stage A, built:**
  - `wifi.wit` 1.1 and `keystore.wit` 1.3, with their bindings and `docs/idl/README.md`;
  - `keystore`'s part: `system/keystore/wifi.country`, two letters, for the station's badge alone;
  - in `dot11`: the country's rules per channel (`country`), and the station under them (`Station::restrict`), with host tests;
  - `wlan` answers the new calls. With no catalogue in the build:
    - the country calls answer `unsupported`;
    - `country()` answers `none` with an empty catalogue identity;
    - the station probes and joins as before;
  - `wlanhw.wit` 1.2 and `wlansim`'s side of it: `wlansim` tunes only to the limits' channels and refuses a probe request where the station may only listen; until limits are first set it keeps 1.1's ways, as the station of stage A sets none;
  - messages to `APP`, `DRV` and ASTRA.

  Stage A changes no behaviour on a device. The requirement is not met yet.
- **Stage B, once `regdb` and `DRV`'s word on 1.2 are in:**
  - each radio's real answer to `set-country` (`DRV`, ASTRA);
  - `wlan` uses the catalogue;
  - with none chosen: the beacons' country, else `00`, each shown with its reason;
  - the country is read back from `keystore` at start;
  - the QEMU checks run, with `wlansim`;
  - the `net` suite's station checks set a country first.
- **The maintainer's `UZ`** is set once on each machine with `wifi country UZ` (550-APP-0082) and kept by `keystore`. It is not a default in the code.

## Acceptance criteria

- **Host tests (`dot11`):**
  - with no country chosen: its first scan sends nothing; then it applies the country the beacons announce, or `00` when none does, says which, and joins under it;
  - a country's `send`, `listen`, `barred` and `radio` channels give probes, passive dwell, a refused join and a refused join;
  - a change of country leaves the network, drops the scan and keeps nothing chosen before it;
  - a refused application leaves nothing applied.
- **Host tests (stage B, with `regdb`):** every catalogued country can be set; `00`, an unknown code and a malformed one are refused.
- **QEMU (stage B, `net` suite with `wlansim`):**
  - the country is set, the station joins, `country()` names it as applied and confirmed by the radio;
  - a restart reads it back from `keystore`;
  - a read-only client is refused;
  - after a change of country the old scan is gone and the link was left.
- **Not claimed:** that a real radio applied a country, or that the channels are right for a machine. That is DRV's on each radio and machine (MC-12.1).

## Progress (2026-11-04, stage B)

Stage B made, on the maintainer's decision of 2026-11-04 (above), on the branch from 84349a2e to b7f2aea7.

**In `dot11`:**
- **The beacons' country.** `scan::describe` reads the Country element. `List::announced` gives the code most access points heard name, with the counts, or none at a tie.
- **`dot11::catalogue`** (on `regdb`):
  - `decide`: the user's choice, else the announced country, else `00`;
  - `moves`: an announcement moves only a country the user did not choose, only to another code, and never back to `00`;
  - `plan`: each of the radio's channels under a country, and the limits for the radio.
- **`Station::relax`** for the first country after a scan that only listened: the networks heard stay.

**In `wlan`:**
- At the radio's first answer it reads `keystore`'s choice.
- With none, the first scan only listens, and its end decides.
- The radio is given each country's limits (`set-country`).
- The log says what is applied and why, for example: `[WLAN] COUNTRY UZ (NONE CHOSEN; ANNOUNCED BY 1 OF THE 1 ACCESS POINTS THAT NAMED A COUNTRY): 13 CHANNELS TO SEND ON, 0 TO LISTEN ON; THE RADIO APPLIED ITS LIMITS`.

**The interfaces:**
- **`wifi.wit` 1.2.** The country calls are answered from the catalogue: 181 countries, `00` left out. `country-origin` is new: none, chosen, detected (with the counts) or world.
- **`wlanhw.wit` 1.3.** `set-country` takes `00` too.

**`wlansim`:**
- its access point announces `UZ`;
- with the test-only feature `no-country` it announces none;
- it logs a probe request that comes before any country's limits.

**Tests:**
- **Host:** `dot11/tests/catalogue.rs` (7), `country_announced.rs` (4), two more in `station.rs`, and the IDL fuzzer with `wifi.wit` 1.2's types.
  - Every catalogued country can be chosen.
  - UZ and RU send on 1 to 13, the US on 1 to 11, and `00` sends on 1 to 11 and listens on 12 and 13.
- **QEMU on x32-128,** both network groups, x86 and aarch64 (`wifi_join_check`):
  - the first scan sent no probe request;
  - the access point's `UZ` was applied and `wifi country` names it;
  - `wifi countries` lists 181, and `wifi channels uz` gives 13;
  - the user's `US` went to the radio and was kept by `keystore` across a restart of `wlan`, and MIND-TEST was joined under it.
- **QEMU, x86 only:** `world_country_check` with the `no-country` radio. `00` is applied and the network on channel 6 is joined.
  The x86 and aarch64 runs of b7f2aea7 were still going when this was written; their result, and the gate's, go in the next record.

**Not done, or not right yet:**
- **A radio's channels after a restart under a country.** `wlanhw.wit`'s `info` gives the channels the radio's last limits left. A station restarted under the US then plans over 11 channels, and a later UZ would keep to those 11 until the radio restarts. The radio should report its own channels apart from the limits: a `wlanhw` question to settle with `DRV`.
- **`world_country_check` on aarch64:** the `no-country` radio is built for x86 only, as the `slow` one is.
- **APP's screen.** `wifi` shows `APPLIED, none ASKED` for a detected country. `country-origin` is there for APP to show why (`APP/2026-11-04T1003Z-NET-…`).
- **Real radios:** none applies a country yet. `bcm_wifi` answers `failed` and `rtw89_wifi` has no `wlanhw` endpoint (213-KRN-0123). That is DRV's, a radio and a machine at a time (MC-12.1).

## Related

[550-NET-0018](../issues-done/550-NET-0018-the-station-service.done), [550-NET-0028](../issues-done/550-NET-0028-rates-the-radio-has.done), [550-APP-0082](550-APP-0082-wifi-country-selector.md), 213-DRV-0032, [requests-DRV.md](requests-DRV.md).
