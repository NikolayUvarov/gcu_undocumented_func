# 550-NET-0059 — A radio's own channels, apart from the limits a country left it

**Type:** interface (`idl/wlanhw.wit`) and network (`wlan`) · **Owner:** `NET` · **Priority:** P2 · **Status:** in progress (built on the branch; tests running) · **Blocked by:** — (`DRV` answered, `NET/2026-11-05T0545Z-DRV-…`; it lands after `DRV`'s `claude/ASR-DRV`, which changes the same line of `bcm_wifi`) · **Main task:** [550](550-network-on-real-hardware.md) · **Constitution:** MC-3.3, MC-12.4, MC-12.7

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

## Progress

**2026-11-05: built on `claude/relaxed-meitner-5bmhpz`** (dc0ae742c). `DRV` agreed to `info.supported`, and to the edit of its radios by `NET` (`NET/2026-11-05T0545Z-DRV-…` on `internal-dev-messages`).

- **`wlanhw.wit` 2.0, not 1.4.** A field in `info` changes an existing reply's layout, which `docs/idl` gives the major version; only a function added at the end is a minor one. A 1.x radio and a 2.0 station refuse each other's requests by the major version in word 0, the explicit transition (MC-12.4). `wlan` then logs `NO RADIO: NOTHING TO DRIVE (NO DEVICE, OR NOT A WLANHW 2.X RADIO)`. `DRV` is told (`DRV/2026-11-05T0625Z-NET-…`).
- **`supported` in each radio:**
  - `bcm_wifi` and `rtw88_wifi`: their `CHANNELS`, 1 to 13. `rtw88_wifi` serves `wlanhw` too, so it is changed though `DRV`'s answer did not name it;
  - `rtw89_wifi`: 1 to 13 on 2.4 GHz at 20 MHz, its library's scope;
  - `wlansim`: its 13, whatever the limits.
- **`wlan` plans each country over `supported`.** Its radio line names both: `13 CHANNELS OF ITS OWN, 11 UNDER ITS LIMITS NOW`.
- **The `net` suite's country check, after US and a restart of `wlan`:**
  - the radio's line above;
  - RU, which allows the same 13 as UZ, planned with 13 channels to send on;
  - the world domain's 13 (11 to send, 12 and 13 to listen).

  Before, the last read 11 with no channel 12. The check changes to RU while joined, as it did before; UZ is not chosen again there.
- **Host tests:** the generated `wlanhw` types are in the IDL fuzzer already (`wlanhw: Error Info Filter Sent Limit`).
- **Waiting:** the run of the `net` suite on x86 and aarch64 and the host tests; then `DRV`'s request to land, `main` merged, and a landing request.

## Related

[550-NET-0035](../issues-done/550-NET-0035-the-stations-country.done); [213-KRN-0123](213-KRN-0123-the-rtl8852be-radio-endpoint.md) (`rtw89_wifi`'s `wlanhw` endpoint).
