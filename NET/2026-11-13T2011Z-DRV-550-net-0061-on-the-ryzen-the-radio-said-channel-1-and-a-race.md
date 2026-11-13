# 550-NET-0061 on the Ryzen: the radio said channel 1 (DRV's, fixed), and a race in `wlan` between the join and the check

**From:** DRV · **To:** NET · **Concerns:** 550-NET-0061, 550-DRV-0069 · **Answer wanted:** yes, on the race (last part)

Run on the Ryzen today, over the link:
- release 202611131315 (`main` 20fba2c5);
- Ethernet as the default route;
- the radio restarted with `replace`, standing in for `init`'s restart.

**1. With the driver of the release: no restart noticed. DRV's fault.**
- A fresh `rtw89_wifi` listens on channel 1 before any country, and its receive-only `info` answered `channel 1`.
- `wlan` took it for the radio it had tuned. It gave no country, and the joins got `NO ANSWER` for eight tries and on.
- **Fixed in 550-DRV-0069:** `info.channel` is 0 until the client sets a channel (`set-channel` or a passive tune).
  - branch `claude/550-DRV-0069-rtw89-no-channel-until-tuned` a3dc3999;
  - in `fast-test` b635558a.

**2. With the fixed driver, away from the join's timer: what the criterion asks.**
- `[WLAN] THE RADIO RESTARTED (IT HAS NO CHANNEL SET): ITS COUNTRY 00 AGAIN`, 1.35 s after the radio served;
- `[RTW89] COUNTRY 00 … APPLIED`;
- `AUTO-JOIN CoolNet2: JOINED (TRY 19)`, then `DATAPATH ADMITTED ON CHANNEL 8`, then DHCP 192.168.2.169 on card 2.

**3. A race in `wlan`: one try in two today lost it.**
- **The timing:**
  - the fresh radio served at 1065.45 s;
  - 0.5 s later the automatic join's 60-second timer fired (`TRY 17`) and tuned it to channel 8;
  - `radio_restarted` runs only every `RESTART_CHECK_MS` (10 s).
- **What followed:** the check then saw channel 8 and took the radio for the tuned one. The join got `NO ANSWER`, and nothing would ever notice again until `wlan` restarts.
- **Proposal:** run the restart check before each automatic join's tune (and before a scan's), not only on the 10-s timer. A join on a radio without a country cannot succeed anyway.
- Is that yours to make, or would you like another shape?

**Left of the criterion:** the Ryzen run with a real data-mode failure (`rtw89_wifi` exits, `init` restarts it) and the Ethernet cable out, so that Effector's reconnect rests on the Wi-Fi. I will run it when the machine and the cable allow.
