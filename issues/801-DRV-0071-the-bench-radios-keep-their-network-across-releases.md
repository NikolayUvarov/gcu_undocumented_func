# 801-DRV-0071 — The bench's radios keep their network across releases

**Type:** driver (`rtw88_wifi`, `rtw89_wifi`, `bcm_wifi`) · **Owner:** `DRV` · **Priority:** P1 · **Status:** in progress (2026-11-14) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) · **Constitution:** MC-6.4, MC-12.1, MC-12.3

## The maintainer's decisions (2026-11-14)

In the maintainer's words, with an English rendering:
- «потеря связи, которая работала ранее под предлогом "ой я не знаю мощности" и прочее - не оправдание. Приоритет - работоспособность. Это критический параметр для субстрата разума»: losing a connection that worked before is not excused by "the power is not known" or the like. Operability comes first; it is a critical parameter for the Mind's substrate.
- «а если и страна неизвестна, передавать на 75% от максимальной»: if the country is unknown too, transmit at 75 % of the maximum.

## Problem

**The DEXP lost its Wi-Fi on 2026-11-14** with release 202611140747. `init` said the platform's power policy was `UNKNOWN (WRDS-in-DSDT)`.
- **A correction** (KRN, `ALL/2026-11-14T0932Z-KRN-…` on `internal-dev-messages`): `init` gives the platform word to `rtw89_wifi` alone. `rtw88_wifi`, the DEXP's radio, reads none, so 213-KRN-0122 never held it back.
- **The DEXP's networks** were lost because the trial's `keystore` held another device key (801-NET-0079, STO's).

**The Ryzen's `rtw89_wifi`** took `platform=unknown:<why>`, or no word, as no transmission. KRN's 801-KRN-0243 now logs such a policy as held back, not enforced, and asked DRV (`requests-DRV.md`, request 1) to send there as with no platform limit.

**The Ryzen's RTL8852BE** lost its network 55 s after joining, with no line from the driver (2026-11-13 at 1245 s, `main` 20fba2c5 plus 550-DRV-0069). `wlan` logged `Joining: LOST`, then every scan heard nothing. This is 213-DRV-0032's silent stall.

## Plan

1. **`rtw89_wifi` under an unknown platform word:** it sends under the country's limits alone, as with no platform limit, and says so.
2. **An unknown country:** each bench radio that sets its power sends at most 75 % of its maximum. A driver that sets no power says so.
3. **The Ryzen's silent stall:** made visible as a line, and the data mode restarted, not left dead.
4. **Each bench radio, across one release:** joined before, joined after, or the trial fails (S1).

## Acceptance criteria

- **Joined:** the DEXP, the Ryzen and the MacBook Pro join CoolNet2 on the same release.
- **The Ryzen:** a 30-minute run with no silent loss, or each loss logged and recovered by itself.

## Progress (2026-11-14)

**Made** on `claude/801-DRV-0071-radios-keep-their-network` (with 550-DRV-0069 merged in).

### 1. `rtw89_wifi` under an unknown platform word (KRN's request 1)

- **The mapping:** `platform=unknown:<why>`, a malformed or repeated word, or none, becomes `PlatformInput::HeldBack` (`receive_service::platform_input`).
  - It resolves as `CheckedAbsent` does: no platform limit, the country's limits and the calibration's checks alone.
  - Its basis is `PlatformBasis::HeldBack`, so it is not taken for a checked absence.
- **What stays:** `platform=limits:…` is obeyed, `platform=absent` is as before, and `PlatformInput::Unknown` itself still refuses.
- **The line:** `[RTW89] PLATFORM POWER POLICY FROM INIT: UNKNOWN (<why>): HELD BACK, NOT ENFORCED: SENDING UNDER THE COUNTRY'S LIMITS ALONE (801)`, once at the start.

### 2. 75 % of the maximum when the country is unknown

**The definition.**
- 75 % of the radio's maximum power in mW is the maximum less 10·log10(1/0.75) = 1.249 dB.
- It is rounded down to the hardware's power step: the fewest whole steps that take off at least 1.249 dB.
  - With 0.5 dB steps that is 3 steps: −1.5 dB, 70.8 %. Two steps (−1 dB) would be 79.4 %, more than 75 %.
  - Both Realtek radios set power in 0.5 dB steps.
- **The maximum** is the most the radio gives any rate on that channel under the rules it runs under.
- **A known country** is not capped.

**When the country is unknown,** as each driver learns it through `wlanhw.wit`'s `set-country`:

| Driver | Unknown when | Its step | The cap |
|---|---|---|---|
| `rtw89_wifi` (the Ryzen's RTL8852BE) | `set-country` `00`; or no `set-country` before the station's first send that is not a probe request, which then takes the world rules (`country::Limits::world`) | the MAC's power registers, half-dBm | each NTX limit held at the channel's maximum less 3 (`Policy::resolve` under `00`, `channel_power::unknown_country`) |
| `rtw88_wifi` (the DEXP's RTL8821CE) | `set-country` `00`, or none yet | the TX AGC index, 0.5 dB (`txgi_factor` 1: the 0.5 dB limit table adds to the index one to one) | every index held at the channel's highest less 3 (`phy::capped`) |
| `bcm_wifi` (the MacBook Pro's BCM4331) | `set-country` `00` | none: a fixed gain table entry (40), power control off, its power in dBm not known (550-DRV-0028) | not applied, and logged. No power path is made here |

**`rtw89_wifi`'s numbers.**
- **The maximum** per channel and NTX is the least of each rate's by-rate value and its section's 20 MHz non-beamformed limit, the highest over the rates. These are the values `channel_power::build` writes.
- **Under the world group** (the pinned tables, the same on every board):

| Channels | 1 TX: maximum → cap | 2 TX: maximum → cap |
|---|---|---|
| 1–11 | 29 → 26 half-dBm (14.5 → 13 dBm) | 23 → 20 |
| 12 | 28 → 25 | 23 → 20 |
| 13 | 28 → 25 | 21 → 18 |

  Channels 12 and 13 are listen-only under `00`, so they are never admitted.
- **TSSI's 12 dBm floor** still holds. Under `00` the world rules' 20 dBm EIRP leaves the tables' maximum, so the cap stays above the floor.
- **In data mode,** a `00` that arrives keeps the power the channel was admitted at. The cap is set at admission.

**`rtw88_wifi`'s numbers** depend on the board's eFuse, so the log names them. The host test's eFuse on channel 1:
- OFDM 6–48 Mb/s at 0x2C is the highest index, so the cap is 0x29;
- CCK's 0x26 stays, and 54 Mb/s's 0x2A becomes 0x29.

**`rtw88_wifi` now takes `00`.**
- Before, `set-country` `00` answered `invalid` (it wanted two capital letters), against `wlanhw.wit` 1.3. `wlan` then logged `THE RADIO DID NOT CONFIRM`, and the radio kept the previous country, or none.
- The country check moved to `rtw88_wifi/src/country.rs`, so that a host test reaches it.

**The lines:**
- `rtw89_wifi`:
  - `[RTW89] COUNTRY 00: n CHANNELS, m LISTEN ONLY; APPLIED: … ; COUNTRY UNKNOWN: POWER CAPPED AT 75 % (801-DRV-0071)`;
  - `[RTW89] NO COUNTRY FROM THE STATION BEFORE ITS FIRST SEND: THE WORLD RULES (00), …; POWER CAPPED AT 75 % OF THE MAXIMUM (801-DRV-0071)`;
  - at admission: `[RTW89] COUNTRY 00, UNKNOWN: POWER CAPPED AT 75 % OF THE MAXIMUM ON CHANNEL c (-1.25 DB ROUNDED DOWN TO THE MAC'S HALF-DB STEP): 1 TX AT MOST 26 HALF-DBM OF ITS 29, 2 TX 20 OF 23`;
  - in data mode: `[RTW89] COUNTRY 00 IN DATA MODE: CHANNEL c KEPT AT THE POWER IT WAS ADMITTED AT (AN UNKNOWN COUNTRY'S 75 % CAP IS SET AT ADMISSION)`.
- `rtw88_wifi`:
  - at the start: `[RTW88] NO COUNTRY YET: POWER CAPPED AT 75 % OF THE MAXIMUM (-1.25 DB ROUNDED DOWN TO 3 INDEX STEPS OF 0.5 DB): INDEX n ON CHANNEL c, ITS MAXIMUM m (801-DRV-0071)`;
  - after `00`'s `COUNTRY` line: the same, led by `COUNTRY UNKNOWN (00)`;
  - a known country's line now ends `… WORLD-WIDE LIMITS, NOT CAPPED, EIRP NOT APPLIED`.
- `bcm_wifi`: `[BCM] COUNTRY UNKNOWN (00): THE 75 % POWER CAP IS NOT APPLIED: THE TRANSMIT GAIN IS A FIXED TABLE ENTRY (40), ITS POWER IN DBM NOT KNOWN (550-DRV-0028)`.

### 3. The Ryzen's silent stall

What the data mode can observe cheaply (`rtw89_wifi/src/service.rs`):
- **No receive** (`QUIET_MS`, 10 s).
  - **The count:** every Wi-Fi frame the receive ring gives, kept or filtered out (`data_mode::Counts::wifi`).
  - **When it is watched:** only while the filter lets a network in, a scan's filter or a BSSID. A filter with no network arms nothing.
  - **What it does:** the data service stops as `Fatal::ReceiveStalled`, with `[RTW89] RECEIVE STALLED ON CHANNEL c: NO FRAME FOR q MS (k BEFORE; s SENT, a ACKNOWLEDGED SINCE THE LAST): THE DATA MODE STOPS (801-DRV-0071)`.
  - **Then:** the stop's registers follow in `WLANHW SERVICE STOPPED`. The driver stops, powers off and quiesces the device and exits (`RESTART_AFTER_FAILURE`), and `init` starts a fresh one. With 550-DRV-0069, `wlan` sees the fresh radio and gives it the country again.
  - **If the device does not stop cleanly,** the driver keeps its DMA and answers `no-device`, as before. It never exits with the device running.
- **No acknowledgement** (`UNHEARD`). 16 unicast sends in a row not acknowledged, over at least 5 s, are logged once a run: `[RTW89] n UNICAST SENDS IN A ROW NOT ACKNOWLEDGED ON CHANNEL c OVER t MS; THE LAST FRAME HEARD h MS AGO (801-DRV-0071)`. The service goes on: an access point out of reach is `wlan`'s to handle.
- **A frame that never ends** was already a line and a stop (`TransmitTimeout`, `KEEP_LATE`).

**Not watched:** the receive-only mode before admission. A scan may sit on a channel with nothing on it.

### Tests (this machine, WSL2; host tests only for these drivers: QEMU has none of their devices)

- **`tests/rtw89_channel_power_host.rs`** (9):
  - on every channel the maximum is what the MAC is given;
  - the cap is 3 below it, and the image built under it holds the maximum exactly there;
  - the world group's maxima are pinned;
  - a conducted limit below the tables is the maximum itself.
- **`tests/rtw89_calibration_policy_cases.rs`** (in `rtw89_calibration_policy_host`, 270 passed):
  - `HeldBack` resolves to the same policy as `CheckedAbsent`, its basis `HeldBack`, while `Unknown` refuses;
  - `00` is capped at 26 of 29 and 20 of 23 on channels 1, 6 and 11; UZ, RU and US are not;
  - an EIRP of 13 dBm under `00` fails TSSI's floor.
- **`tests/rtw89_receive_service_cases.rs`** and **`tests/rtw89_service_cases.rs`** (in `rtw89_service_host`, 337 passed):
  - before a country, a probe admits nothing, and the next send takes the world rules and asks admission;
  - init's unknown, empty, repeated or missing word maps to `HeldBack`;
  - a receive silent for 10 s stops the service with its line, after frames every 9.9 s kept it going;
  - a filter with no network watches nothing, and a scan's filter arms it from its start;
  - 16 unacknowledged sends over 5 s are logged once. The data-mode device model gained `unacked`, a release at its retry limit.
- **`tests/rtw88_driver_host.rs`** (9): the capped indices on every channel and the registers written, plain and capped.
- **`tests/rtw88_country_host.rs`** (2, new): `00` taken, an unknown country told apart, malformed codes refused.
- **The rest, on this branch's tree:**
  - `scripts/ci_local.sh` groups `build (x86)` and `host tests`: 230 suites, 6247 passed, none failed;
  - every `rtw89_*` host suite again after the last change to `service.rs` and `data_mode.rs`: 39 suites, 4734 passed;
  - `build (x86 test programs)` and `x86: network, TLS and Effector` (the `net` suite with `wlansim`, then `tls` and `effector`): passed.
  - QEMU starts none of these three drivers, so the QEMU groups show only that the rest of the network path is unchanged.

### The replace builds for the bench

Built from copies of the running release trees with this task's driver diffs applied. The trees themselves were not changed:

| Machine | ELF | SHA-256 | Tree |
|---|---|---|---|
| the Ryzen | `rtw89_wifi` | `40d121938a05e6449273a83e0731d38c3b057ce37ae260c86a57364af764d13e` | 8f15853e (release 202611140747's) |
| the DEXP | `rtw88_wifi` | `e3a209d092fbbe6f6c992b6b842ceaf62c18acd1b9a63873d82bd3ccfd1b5123` | d83c98c5 (release 202610302359, ABI 25, `wlanhw` 1.2: its `info` kept 1.2's) |
| the MacBook Pro | `bcm_wifi` | `e7655f2c515313de303f8f14e672b225a952794d8c31405625f32a2c9d92ab97` | 8f15853e |

The DEXP's `wlan` of d83c98c5 never sends `00`. There the radio is capped until `wlan` gives it a chosen or announced country.

### Left for the bench (nothing above was run on a machine, MC-12.3)

- **The Ryzen:**
  - the platform line;
  - under `00` the cap's line at admission, and none under a known country;
  - a join;
  - the 30-minute run, where every loss shows `RECEIVE STALLED` or another stop line, then `EXITING`, `wlan`'s `THE RADIO RESTARTED` and a join again.
- **The DEXP:** the cap's line with its index while no country is known, `NOT CAPPED` under a known one, and a join.
- **The MacBook Pro:** a join. Its only change is the `00` line.
- **Then** the acceptance criteria: the three joined on one release.

## Related

[801](801-the-bench-stays-manageable.md), [213-KRN-0122](213-KRN-0122-platform-power-policy-for-the-wlan-device.md) (801-KRN-0243), [213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md), [550-DRV-0028](550-DRV-0028-bcm4331-transmit-power.md), [550-DRV-0069](550-DRV-0069-rtw89-says-no-channel-until-its-client-sets-one.md), [the service notes](../hwdocs/rtl8852be-service.md#an-unknown-platform-word-an-unknown-country-and-a-stalled-receive-801-drv-0071-2026-11-14).
