# 550-NET-0035 — The station's country: chosen from the whole regulatory catalogue, kept, applied by the radio

**Type:** network (service and interfaces) · **Owner:** `NET` track · **Priority:** P1 · **Status:** open (stage A built) · **Blocked by:** stage B: the catalogue as a crate of its own ([requests-DRV.md](requests-DRV.md); `DRV` agreed, 2026-10-20) · **Main task:** [550](550-network-on-real-hardware.md) · **Roadmap:** D / 550, H / 213 · **Constitution:** MC-2.3, MC-3.3, MC-6.1, MC-12.1, MC-12.3, MC-12.4, MC-12.7

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
- **What must hold** (the request):
  - no default: a missing, unknown or unapplied country is never DE, US, the world domain or permission to send;
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
- **With no country applied** the station scans without probing and `join` answers `unsupported`. `country()` says why.
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
  - with none applied it probes nowhere, and `join` answers `unsupported`;
  - the country is read back from `keystore` at start;
  - the QEMU checks run, with `wlansim`;
  - the `net` suite's station checks set a country first.
- **The maintainer's `UZ`** is set once on each machine with `wifi country UZ` (550-APP-0082) and kept by `keystore`. It is not a default in the code.

## Acceptance criteria

- **Host tests (`dot11`):**
  - with no country the station sends no probe and joins nothing;
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

## Related

[550-NET-0018](550-NET-0018-the-station-service.md), [550-NET-0028](550-NET-0028-rates-the-radio-has.md), [550-APP-0082](550-APP-0082-wifi-country-selector.md), 213-DRV-0032, [requests-DRV.md](requests-DRV.md).
