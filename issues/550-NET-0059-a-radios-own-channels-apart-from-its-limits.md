# 550-NET-0059 — A radio's own channels, apart from the limits a country left it

**Type:** interface (`idl/wlanhw.wit`) and network (`wlan`) · **Owner:** `NET` · **Priority:** P2 · **Status:** open · **Blocked by:** `DRV`'s answer (`DRV/2026-11-04T1335Z-NET-wlanhw-1-3-takes-00-and-a-question-on-a-radios-channels.md` on `internal-dev-messages`) · **Main task:** [550](550-network-on-real-hardware.md) · **Constitution:** MC-3.3, MC-12.4, MC-12.7

**Split from:** [550-NET-0035](../issues-done/550-NET-0035-the-stations-country.done), its first point of "not done".

## Problem

- **`info` answers the limits' channels.** `wlanhw.wit` 1.2's `info` gives the channels the radio's last limits left.
- **So a restart narrows the station.** A station restarted while a country holds plans over those channels only.
  - `wlansim` shows it: after the US, 11 channels.
  - A later choice of UZ then keeps to those 11 until the radio itself restarts, though UZ allows 13.
- **Which channels a country allows is the station's to decide** (`wlanhw.wit` 1.0). The radio's own channels are what it decides over.

## Plan

1. **`wlanhw.wit` 1.4,** agreed with `DRV`:
   - `info` names the radio's own channels apart from the limits, for example a field `supported`;
   - `channels` keeps its meaning;
   - a new version and an explicit transition (MC-12.4), and `bcm_wifi` and `rtw89_wifi` answer it in the same landing, since they match over every request.
2. **`wlan`** plans each country over `supported`.
3. **`wlansim`** answers its 13 channels whatever the limits.

## Acceptance criteria

- **QEMU (`net` suite with `wlansim`):** after US and a restart of `wlan`, a change to UZ gives 13 channels to send on.
- **Host tests:** the generated `wlanhw` types in the IDL fuzzer.
- **Not claimed:** any real radio's channels (MC-12.1). That is `DRV`'s, a radio and a machine at a time.

## Related

[550-NET-0035](../issues-done/550-NET-0035-the-stations-country.done); [213-KRN-0123](213-KRN-0123-the-rtl8852be-radio-endpoint.md) (`rtw89_wifi`'s `wlanhw` endpoint).
