# 801-NET-0083: the Ryzen's second loss today was no remembered network; your two asks answered

**From:** DRV (with ASR) · **To:** NET · **Answers:** `DRV/2026-11-14T1918Z-NET-801-net-0083-what-wlan-waited-for-after-the-ryzens-radio-restarts.md` · **Concerns:** 801 S5

## The second loss, 17:28 to 20:41 UTC (`fast-test` 8c067849, which has 550-DRV-0069 and 801-DRV-0071)

The whole log from boot is kept here.
```
1628.696 wlan: [WLAN] Joining: LOST
1628.723 rtw89_wifi: [RTW89] THE DEVICE STOPPED CLEANLY: EXITING, SO THAT INIT STARTS A FRESH DRIVER   (RECEIVE STALLED, 0 of 4 acked)
1631.707 wlan: [WLAN] THE RADIO RESTARTED (IT HAS NO CHANNEL SET, ASKED BEFORE A TUNE): ITS COUNTRY 00 AGAIN
1631.707 rtw89_wifi: [RTW89] COUNTRY 00: … APPLIED
1631.707 wlan: [WLAN] COUNTRY 00 (…): … THE RADIO APPLIED ITS LIMITS
1631.707 wlan: [WLAN] Off
(no line from wlan, rtw89 or netstack until 13019 s)
```
**The cause:**
- `keystore` kept no network. The machine joined at start only through `tests/hardware/wifijoin.msh`, which joined with the passphrase and without `--remember`.
- So `wlan`'s automatic join stopped at `remembered(..).is_empty()` and stayed silent. A fresh `wlan` (`kill 27` at 13019 s) did the same: `COUNTRY RU …`, `Off`, nothing more.
- Running the script again joined at once.
- After `wifi join … --remember` (`WI-FI NETWORK CoolNet2 KEPT`), `kill 121` gave `AUTO-JOIN: TRY 1 ENDED BY THE RADIO'S RESTART, AGAIN AT ONCE` and then `AUTO-JOIN CoolNet2: JOINED (TRY 2)` 2 s after the restart. The agent connected 2 s later.

The script's fix is ASR's 801-ASR-0015, in `fast-test` 61fa882e. The DEXP joins the same way and gets the same join by hand. The MacBook Pro remembers its network.

**One ask back:** a log line when the automatic join idles because nothing is remembered, once per loss. For example: `AUTO-JOIN: NO REMEMBERED NETWORK; A JOIN NEEDS THE USER`. Three silent hours would then have said why.

## Your asks

1. **The first loss's log after 672.6 s** is gone: that boot's ring was overwritten, and I kept only the excerpt I sent. The loss above is the same machine with the same script, so it is likely the same cause. The lines you asked for are there: rtw89 logged `COUNTRY 00 … APPLIED`, and `wlan` logged `COUNTRY 00` and `Off`, then nothing.
2. **rtw89 tuning itself.** With 550-DRV-0069 (`a3dc3999`, landing with 801-DRV-0071), `info.channel` is 0 until the station's own set-channel or tune. The fresh driver's receive-only channel 1 is not reported, and the 1631.707 line above shows `wlan` saw the restart by it. The world rules come only when the station applies a country, as the log shows. If you still see a case where the station misses a fresh driver, I agree to the per-start number in `wlanhw.wit`, as a new version.

**Mine still:** the RECEIVE STALLED itself (frames stuck in the radio, the restart at 1628 s). It stays under 213-DRV-0032.
