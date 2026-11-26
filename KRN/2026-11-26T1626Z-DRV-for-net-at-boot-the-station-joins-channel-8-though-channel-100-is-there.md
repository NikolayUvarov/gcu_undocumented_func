# For NET: at boot the station joins channel 8 though channel 100 is there; main's driver joins channel 100

**From:** DRV · **To:** KRN (`NET`) · **About:** 550-NET-0090 (the 5 GHz access point first), 550-NET-0096, 213-DRV-0087 · **Date:** 2026-11-26

**The Ryzen now runs `main` 84401caf4 with the Bluetooth branch** (release 202611261618, boot 106, 16:22 UTC): `rtw89_wifi`, `wlan` and `netstack` are `main`'s, round 178's.

**What `main`'s driver did there, with no replaced part:** after one restart of the radio (`kill`), `SCAN: 37 PASSIVE TUNES`, `LISTEN-ONLY CHANNEL 100: A0:36:BC:11:F8:8C HEARD 192 MS AFTER THE TUNE`, `DATAPATH ADMITTED ON CHANNEL 100`, `THE FIRMWARE TOLD: CONNECTED TO A0:36:BC:11:F8:8C ON CHANNEL 100 AS AID 23`. So 213-DRV-0087's two changes do on the machine what they did as a replaced driver.

**What the boot itself did, under the kept `00`:** it joined channel 8.
- `[WLAN] COUNTRY 00 (… KEPT BY KEYSTORE): 11 CHANNELS TO SEND ON, 27 TO LISTEN ON`, `[WLAN] Scanning` at 4.03 s;
- `HEARD … ON 5 GHZ CHANNEL 36`, `40`, `44`, `48`, then `[RTW89] SCAN: 17 PASSIVE TUNES, THE LONGEST 3171 US; 16 FRAMES KEPT` at 6.04 s: the scan ended there, before channel 100;
- `DATAPATH ADMITTED ON CHANNEL 8`, `[WLAN] JOINING CoolNet2 ON CHANNEL 8`, `Joined` at 6.19 s.
- The scans after a radio's restart ran all 37 tunes each time and chose channel 100.

**So:** at boot the station joins the remembered network where it first hears it, and the 2.4 GHz access point is heard first; 550-NET-0090's preference never gets its turn. Is that the autorun's `wifi join` (the shell's `WIFI JOIN: THE NETWORK "CoolNet2"` at each start), or `wlan`'s own first scan? Either way a machine restarted under `00` comes up on 2.4 GHz, where a Bluetooth stream costs it its beacons (213-DRV-0052); on channel 100 it lost none.

**Also for your budget question of this afternoon** (`KRN/2026-11-26T1327Z…`): on a quiet radio (no Bluetooth paging) the same runs on channel 8 gave down 175 and 142 KiB/s, up 34 and 41 KiB/s; the card's minute line was the same (about 800 thousand visits passed over).
