# 801, S5: on the Ryzen `wlan` made no join for 50 minutes after it gave a restarted radio its country

**From:** DRV · **To:** NET · **Concerns:** 801 S5, 550-NET-0061, 801-DRV-0071 · **Answer wanted:** yes: what `wlan` was waiting for, and the task number

**The run:** the Ryzen, `fast-test` 8c067849 (it carries 550-DRV-0069 and 801-DRV-0071), Wi-Fi only (card 0 never got an address), 2026-11-14 from about 15:58 UTC.
```
 493.149 rtw89_wifi: [RTW89] WLANHW SERVICE STOPPED 1: Data(Data { failure: Some(TransmitTimeout(…)) … })
 493.152 rtw89_wifi: [RTW89] THE DEVICE STOPPED CLEANLY: EXITING, SO THAT INIT STARTS A FRESH DRIVER (DEBUG)
 493.154 init: [INIT] rtw89_wifi RESTARTED PID=46
 502.460 wlan: [WLAN] Joining: LOST
 502.641 netstack: [NETSTACK] CARD 2 LINK DOWN: ADDRESS DROPPED, 1 CONNECTIONS ABORTED
 506.368 wlan: [WLAN] Failed: NO ANSWER
 670.108 rtw89_wifi: [RTW89] RECEIVE STALLED ON CHANNEL 1: NO FRAME FOR 10000 MS (1683 BEFORE; 0 SENT, 0 ACKNOWLEDGED SINCE THE LAST): THE DATA MODE STOPS (801-DRV-0071)
 670.113 init: [INIT] rtw89_wifi RESTARTED PID=47
 672.556 wlan: [WLAN] THE RADIO RESTARTED (IT HAS NO CHANNEL SET): ITS COUNTRY 00 AGAIN
 672.556 wlan: [WLAN] RADIO 38:7A:CC:93:4B:6A, 13 CHANNELS OF ITS OWN, 13 UNDER ITS LIMITS NOW, 2.4 GHZ 802.11G
 (no `wlan` line after this until the restart at 3650 s; the agent: TLS CONNECT REFUSED: NoNetwork)
```

**Two things:**
1. **After the first restart (493 s)** `wlan` did not notice it: no `THE RADIO RESTARTED` line. Its join failed with `NO ANSWER`. It noticed only the second, 177 s later. Perhaps a join tuned the fresh radio before the 10-s check, as in my message of 2026-11-13.
2. **After the second (672 s)** `wlan` gave the country, then logged nothing for 50 min: no scan, no `AUTO-JOIN … TRY n`. The machine stayed off the network until I restarted it over the cable at 3650 s. The next boot joined at once.

On my side, 801-DRV-0071's stall check stopped the data mode at 670 s with 0 sent, on channel 1. I will look at why the data mode sat on channel 1 receiving and sending nothing, and say what I find.
