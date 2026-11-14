# 550-DRV-0069 — `rtw89_wifi` says no channel until its client sets one, so `wlan` sees a restarted radio

**Type:** driver · **Owner:** `DRV` · **Priority:** P1 · **Status:** in progress (2026-11-13) · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md), for [550-NET-0061](550-NET-0061-a-restarted-radio-gets-its-country-again.md) · **Roadmap:** tracks H and D · **Constitution:** MC-6.4, MC-12.1, MC-12.3

## Problem

[550-NET-0061](550-NET-0061-a-restarted-radio-gets-its-country-again.md) makes `wlan` notice a radio that started again behind the same endpoint: `wlanhw`'s `info.channel` is 0 on a fresh radio. `wlan` then gives it the country again and joins again.

**On the Ryzen** (2026-11-13, release 202611131315, `main` 20fba2c5), `replace unit=rtw89_wifi back=true` restarted the radio, and `wlan` noticed nothing:
- A fresh `rtw89_wifi` listens on channel 1 before any country (`RECEIVE-ONLY CHANNEL 1`), and `ReceiveOnly::info` answered that channel.
- So `wlan` saw channel 1, took the radio for the one it had tuned, and gave it no country.
- The radio stayed receive-only: `AUTO-JOIN CoolNet2: NO ANSWER`, eight tries and on.

## Plan

`ReceiveOnly` answers `info.channel` 0 until its client sets a channel: either a `set-channel` to the channel it listens on, or a passive tune done for one. The channel it listens on alone is its own, not the client's.

## Acceptance criteria

- Host tests: a fresh receive-only service says channel 0; after `set-channel 1` it says 1, after a tune to 6 it says 6.
- On the Ryzen: after `rtw89_wifi` starts again, `wlan` logs `THE RADIO RESTARTED (IT HAS NO CHANNEL SET)`, gives it the country, and joins CoolNet2 again by itself.

## Progress (2026-11-13)

**Made** on `claude/550-DRV-0069-rtw89-no-channel-until-tuned`:
- `rtw89_wifi/src/receive_service.rs`: a `set` flag, set by a `set-channel` to the channel it listens on and by a passive tune; `info` says 0 until then.
- `tests/rtw89_receive_service_cases.rs`: the first `info` says 0, then 1 after `set-channel 1`, then 6 after the tune.

**Tested:**
- `tests/rtw89_service_host.rs`: 333 passed.
- **On the Ryzen,** the built `rtw89_wifi.elf` (on `main` 23f1f33e, ABI 32) put in with `effector485`'s `replace unit=rtw89_wifi`, over release 202611131315 (`main` 20fba2c5), Ethernet as the default route:
  - **The first try** (20:07 UTC) raced `wlan`'s automatic join. The fresh radio served at 1065.45 s. 0.5 s later the join's 60-second timer tuned it to channel 8, before `wlan`'s 10-second restart check. The check then saw channel 8, and the join got `NO ANSWER`. The race is `wlan`'s: its check runs only every 10 s, while a join tunes the radio at once. Told to NET.
  - **The second try** (20:09 UTC, away from the join's timer) did everything the criterion asks:
    - `[WLAN] THE RADIO RESTARTED (IT HAS NO CHANNEL SET): ITS COUNTRY 00 AGAIN`, 1.35 s after the radio served;
    - `[RTW89] COUNTRY 00 … APPLIED`;
    - `AUTO-JOIN CoolNet2: JOINED (TRY 19)`;
    - `DATAPATH ADMITTED ON CHANNEL 8`;
    - DHCP 192.168.2.169 on card 2.

**Left:**
- the full host tests and the `net` suite before the landing request;
- 550-NET-0061's own run: a data-mode failure that makes `rtw89_wifi` exit, with the Ethernet cable out, so that Effector's reconnect depends on the Wi-Fi.

## Related

[550-NET-0061](550-NET-0061-a-restarted-radio-gets-its-country-again.md), [213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md), [213-DRV-0033](213-DRV-0033-rtl8852be-firmware-and-probe.md).
