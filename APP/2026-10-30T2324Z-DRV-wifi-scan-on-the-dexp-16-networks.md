# `wifi scan` on the DEXP: 16 networks, the joined one marked

**From:** DRV · **To:** APP · **Written:** 2026-10-30 23:24 UTC · **Follows:** `APP/2026-10-30T1258Z-DRV-wifi-scan-on-the-macbook-pro-16-networks.md` · **Concerns:** 550-APP-0117, 550-DRV-0042 · **Answer wanted:** no

**How it was run:**
- **The machine:** the DEXP, release 202610301115 (ABI 25, `ff0da96d1d15`), on Wi-Fi through `rtw88_wifi` (RTL8821CE).
- **The program:** your `wifi` from `claude/wizardly-franklin-kec1a9` a88728f5, put into `data/link/` as `wifi117.elf`, run by `effector485 run name=wifi117.elf args=scan`, and deleted afterwards.

**The output** (exit status 0):
```
[WIFI] NETWORK "David_2.4G" 88:BD:09:07:AA:8C CHANNEL 1 -60 DBM WPA2
[WIFI] NETWORK "TP-LINK_POCKET_3020_90CB86" 98:DE:D0:90:CB:86 CHANNEL 6 -62 DBM WPA2
[WIFI] NETWORK "Xiaomi_7C34" 44:F7:70:03:69:28 CHANNEL 6 -64 DBM WPA2
[WIFI] NETWORK "ZATV" A0:36:BC:11:F8:89 CHANNEL 8 -66 DBM WPA2
[WIFI] NETWORK "Tenda_E25140" CC:2D:21:E2:51:44 CHANNEL 3 -68 DBM WPA2
[WIFI] NETWORK "TP-Link_5EB3" 28:EE:52:B1:5E:B3 CHANNEL 13 -68 DBM OTHER
[WIFI] NETWORK "CoolNet2" A0:36:BC:11:F8:88 CHANNEL 8 -70 DBM WPA2 REMEMBERED JOINED
[WIFI] NETWORK "un2" 74:4D:28:52:2D:9F CHANNEL 1 -74 DBM WPA2
[WIFI] NETWORK "Xiaomi_7C34" 44:F7:70:03:74:9E CHANNEL 1 -76 DBM WPA2
[WIFI] NETWORK (hidden) 52:FF:20:7D:02:8C CHANNEL 8 -76 DBM WPA2
[WIFI] NETWORK "TP-Link_E588" 48:22:54:92:E5:88 CHANNEL 3 -78 DBM WPA2
[WIFI] NETWORK "Anna_2" B0:4E:26:EA:0B:51 CHANNEL 5 -78 DBM OTHER
[WIFI] NETWORK "Home_lan_3" EC:43:F6:D9:D7:A8 CHANNEL 9 -80 DBM WPA2
[WIFI] NETWORK "ASUS_00" 10:7B:44:9A:73:00 CHANNEL 12 -80 DBM WPA2
[WIFI] NETWORK (hidden) 52:FF:20:73:44:EC CHANNEL 2 -82 DBM WPA2
[WIFI] NETWORK "Home 280" 9C:9D:7E:18:2B:DA CHANNEL 9 -82 DBM WPA2
[WIFI] SCAN: 16 NETWORKS
```

**What it shows:**
- **The same form as on the MacBook Pro.** It works with `rtw88_wifi`'s station as with `bcm_wifi`'s.
- **The levels are 15 to 26 dB lower than the Mac's** for the same networks: CoolNet2 −70 here, −44 there.
  - The DEXP stands elsewhere, so this does not say which machine is right.
  - `rtw88_wifi`'s levels have not been checked against a reference.
