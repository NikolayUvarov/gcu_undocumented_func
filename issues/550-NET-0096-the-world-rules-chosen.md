# 550-NET-0096 — The world rules chosen: `00` as the user's country, kept against announcements

**Type:** `wlan`, `dot11`, `keystore` (interface: `wifi.wit` 1.4) · **Owner:** `NET` (the kernel session) · **Priority:** P1 · **Status:** in progress (in `main` since round 177, bf0e9db15a, 2026-11-26; the Ryzen's 5 GHz join left) · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.3, MC-12.4

## Problem

DRV's message of 2026-11-25 (`NET/2026-11-25T2200Z-DRV-…`) asks for the Ryzen's 5 GHz join on channel 100. It quotes the maintainer of that day: «на этапе тестирования мы не привязываемся к странам, первый приоритет - чтобы работало, hardening - это отдельный этап».

What stands in the way:
- **With no country chosen,** the station takes the access points' announcement. On the bench that is RU, whose 5 GHz rules are all NO-OUTDOOR, so there is no 5 GHz channel.
- **A chosen UZ** ends at 5330 MHz.
- **Under `00`** channel 100 is listen-only (NO-IR, DFS). That is all the join needs (550-NET-0090, 213-DRV-0087).
- **But the station cannot stay under `00`:**
  - `set-country` refused `00` as `invalid` (`wifi.wit` 1.1 to 1.3);
  - `keystore` refused to keep it;
  - `catalogue::decide` took no chosen `00`;
  - so the next scan moved a station under `00` to the announced country.

## Plan (made)

- **`dot11`:** `catalogue::decide` takes a chosen `00` (`entry`, not `regdb::select`). `moves` already leaves a chosen country alone.
- **`wlan`:**
  - `set-country` takes `00` (`code_of(code, true)`, `catalogue::entry`), and so does the kept country at start;
  - the log says `COUNTRY 00 (THE RULES EVERY COUNTRY ALLOWS, CHOSEN BY THE USER, KEPT BY KEYSTORE)`.
- **`keystore`:** keeps `00` as it keeps a country.
- **`wifi.wit` 1.4:** says so. A 1.3 client loses nothing: it was answered `invalid` for `00`.
- **Docs:** `docs/network/station.md` 0.6, the country section.

The previous rule, that `00` is never a chosen country (the maintainer's decision of 2026-11-04, 550-NET-0035 stage B), is replaced by the maintainer's word of 2026-11-25 for the testing stage. The host test that stated it now states the new rule.

## Acceptance criteria

- `dot11`'s host tests:
  - a chosen `00` decides `00`;
  - an announcement does not move a chosen `00`;
  - a `00` nobody chose still moves.
- QEMU's `net` suite (`wlansim`):
  - `wifi country 00` applied and kept by `keystore`;
  - wlan's line as above;
  - a scan that hears UZ announced does not move it.
- On the Ryzen: `wifi country 00`, then the join on channel 100 (213-DRV-0087), for that machine and build only (MC-12.1).

## Related

[550-NET-0035](../issues-done/550-NET-0035-the-stations-country.done), [550-NET-0090](550-NET-0090-the-station-on-5-ghz.md), [213-DRV-0087](213-DRV-0087-rtl8852be-on-5-ghz.md), [550-DRV-0088](550-DRV-0088-the-owners-rule-beside-the-catalogue.md).
