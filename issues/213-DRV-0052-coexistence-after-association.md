# 213-DRV-0052 — RTL8852BE coexistence once the station has joined: Wi-Fi and Bluetooth share the time

**Type:** driver · **Owner:** `DRV` (with `BLT` for `bt_hci`'s scans) · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md), [550](550-network-on-real-hardware.md) · **Roadmap:** tracks H and D · **Constitution:** MC-6.1, MC-12.1, MC-12.3

## Problem

On the Ryzen (2026-11-08, [213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md), build 8e3913fd) the station joined CoolNet2 and got an address over the RTL8852BE. **Six minutes later it lost the network:**
- `bt_hci`'s round ran a 10-second inquiry (one every 320 s);
- the station heard nothing from the access point for 10 s (`SILENCE_MS`) and began to scan;
- its next frame timed out, and the data mode stopped (`Tx(Io(Timeout))`). The BB in reset and the host DMA stopped that the stop line showed then were the data mode's own shutdown, read after it (corrected 2026-11-08 evening, [213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md)).

**The cause.** `rtw89_wifi`'s coexistence keeps the `NoLink` policy and antenna after association, as at startup. It has only `ResetDefaults`, `NoLink` and `Rfk` (`btc_codec.rs`). Under `NoLink` the firmware gives the shared RF to Bluetooth whenever Bluetooth asks, a whole inquiry long.

**What Linux does.** After association, Linux's rtw89 tells the coexistence of the station:
- `rtw89_btc_ntfy_role_info` with `BTC_ROLE_MSTS_STA_CONN_END`;
- the role and link H2Cs.

The coexistence then picks a policy for a connected station (`_action_wl_2g_sta` and the TDMA and slot tables of `coex.c`). Wi-Fi and Bluetooth then share the time.

## Plan

1. **A check of the cause first:** a `bt_hci` with one round a day, on the Ryzen. Wi-Fi should then hold well past six minutes. This is a trial build, not committed.
2. **The role notice after the join:** from the data mode, once `wlan` reports the network joined (or on the first data frame acknowledged): the role, its link and channel, with Linux's H2C layouts from the pinned sources.
3. **A policy for a station connected on 2.4 GHz:**
   - **The idle case first:** Bluetooth idle or only scanning. Linux's TDMA and slots for it, from `coex.c`, the tables recorded with their sources.
   - **The antenna:** for a connected station, `Antenna` gains `Connected`.
4. **Leaving:** back to `NoLink` when the station leaves or the data mode stops.
5. **Until 2–4 exist:** `bt_hci`'s inquiry rounds pause while a Wi-Fi network is joined on a combined chip (`BLT`). This is a stopgap, logged as such.

## Acceptance criteria

- **The Ryzen with `bt_hci` running its rounds:** joined to CoolNet2 for an hour across at least ten inquiries, with no loss of the network and ping answered.
- **Host tests:** the role notice's encoding and the policy's TDMA and slots against Linux's values; the transitions from `NoLink` to connected and back.

## Progress (2026-11-08)

**Plan step 1, the check of the cause.** The Ryzen ran `bt_hci` with one round a day (a trial build, replaced over the link), then joined CoolNet2 again with 8e3913fd.
- **The result:** Wi-Fi held for the 12 minutes watched, past two of the old rounds' times. The card received 595 to 1403 frames, and its address stayed.
- **Before**, with the rounds every 320 s, the network was lost at the first inquiry after six minutes.

One run on that machine (MC-12.1): this supports the cause, it does not prove it.

## Progress (2026-11-08, evening): plan steps 2 and 3's idle case

**Built** (fb10b69d). The data mode's first coexistence reconcile sends what Linux sends at `BTC_ROLE_MSTS_STA_CONN_END` for one 2.4 GHz station (pinned `05f7e89a`):
- ROLE v2, the station connected on the admitted channel;
- the policy `Station`, Linux's `OFF_BWB1`: the OFF slot's table `0xea5a5a5a` instead of `NoLink`'s `0xe5555555`, TDMA off;
- AFH around the channel;
- path B's S1 standby, and the WLAN scoreboard's bits 13 and 12.

**What the source settled about the plan:**
- **The antenna.** Linux's antenna is the same connected as not (`BTC_ANT_W2G`), so `Antenna` gains no `Connected`: plan step 3's second point is not needed.
- **Leaving (step 4).** Linux too keeps the connected policy after a disconnect, and returns to no-link only at power-off. Here a data mode that stops ends the driver (`RESTART_AFTER_FAILURE`, [213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md)).

**Not ported:** Linux's TDMA branches:
- `FIX_TD7010` while linking;
- `FIX_TD4010ISO` for a busy Wi-Fi;
- `PFIX_TD5050` for a busy Wi-Fi during an inquiry, which also needs null-data templates.

The source and the layouts: [hwdocs/rtl8852be-btc-runtime.md](../hwdocs/rtl8852be-btc-runtime.md).

**On the Ryzen** (`rtw89_conn1`, the cable out, `bt_hci` from its boot image with rounds every 300 s):
- **The inquiry.** A whole 10-second inquiry (`INQUIRY COMPLETE AFTER 10263 MS`) passed with the station joined. Before, the first inquiry after six minutes lost the network.
- **The gateway pings,** every 2 minutes: 0 to 33 % lost.
- **After 9 minutes** the data mode stopped: one frame's release did not come in its time (`Tx(Io(Timeout))` after 5 submissions). That is [213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md)'s open problem, not the inquiry.

**With `KEEP_LATE`** (`rtw89_late1`, 1b4e3942):
- **About 20 minutes:** four inquiries passed, and five late frames were answered as not acknowledged, their releases coming in the extra second. The gateway pings lost 0 to 33 %.
- **Then the queue's own budget** ran out inside a step, before the service's 90 ms (`Tx(Io(Timeout))`, 7 submissions).
- **Since then** the queue gets the late second at once, so only the service's deadlines end a frame (`rtw89_late2`).

**`rtw89_late2`** (c9cbde29):
- **About 25 minutes:** five inquiries passed, and 15 late frames were answered and released. The gateway pings lost 0 to 33 % (one round 870 ms at most).
- **Then two firmware recoveries, 60 ms apart** (`CMAC_ERR_ISR` 8), were outlasted. The next coexistence reconcile found the radio off (`Coexistence(RadioOff)`), and the data mode stopped.
- **Since then** `OUTLAST_SER` outlasts a recovery found in the reconcile too, and the reconcile runs again (`rtw89_late3`).

**`rtw89_late3`** (2169b65c):
- **42 minutes:** eight inquiries passed, and 27 late frames were answered and released. The gateway pings lost nothing in most rounds.
- **Then the session's permit check found the scheduler at zero** (`Session(Permit(PowerState at 0xc348))`), a firmware recovery under way. That check runs in every operation, past the outlasting at its start.
- **Since then,** under `OUTLAST_SER`, the data permit takes a zeroed scheduler as the next operation's to outlast (`rtw89_late4`).

**`rtw89_late4`** (5cba4875) stopped after 16 s:
- a recovery at 5.6 s was outlasted;
- then the coexistence reconcile found SYS_FUNC_EN at `0xDE` (`Coexistence(Register(PowerState at 2))`).

So the reconcile now retries on any coexistence error during which a recovery is found, and only then (`rtw89_late6`). One admission between them failed in TSSI `ConfigureA`, path A's table read back changed, and the driver restarted.

**`rtw89_late6`** (ef5a29b2) ran the whole hour without a stop (2026-11-08, 20:39–21:45 UTC; 64 minutes joined in the log):
- **12 whole inquiries passed** with the station joined.
- **Outlasted:** 142 late frames, answered as not acknowledged and released, and 108 firmware recoveries.
- **The gateway pings,** 3 every 2 minutes, 33 rounds:
  - from 20:53 nothing lost in all but three rounds (one lost all three, two one each), 15–131 ms;
  - **the first 13 minutes were poor:** 33–100 % lost and up to 870 ms, with the recoveries coming fast.
- **Against the criteria:**
  - **The hardware criterion:** an hour across at least ten inquiries, no loss of the network. Met once, on that machine and build (MC-12.1). "Ping answered" held in 31 rounds of 33.
  - **The host criterion:** the role notice's encoding and the policy's slot match Linux's worked values. `Station` has no TDMA, and the TDMA policies are not ported. The transition from `NoLink` to connected is tested; back to `NoLink` is a driver restart (`RESTART_AFTER_FAILURE`), where Linux returns to no-link only at power-off. So this issue stays open: closing it, or amending that criterion, is the maintainer's.

**Across these runs:** 30 whole inquiries (1, 4, 5, 8 and 12) passed with the station joined, and none lost the network. Each stop was another cause: a late frame's queue budget, or a recovery during the reconcile.

One machine and one AP (MC-12.1). The acceptance criterion, an hour across ten inquiries in one run, is not met yet.

## Related

[213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md), [hwdocs/rtl8852be-btc-runtime.md](../hwdocs/rtl8852be-btc-runtime.md), [700](700-bluetooth.md).
