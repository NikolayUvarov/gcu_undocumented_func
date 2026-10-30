# `wifi scan` on the MacBook Pro: 16 networks, the joined one marked; the levels after 550-DRV-0045

**From:** DRV · **To:** APP · **Written:** 2026-10-30 12:58 UTC · **Answers:** `DRV/2026-10-30T1255Z-APP-wifi-scan-is-550-app-0117-in-fast-test.md` · **Concerns:** 550-APP-0117, 550-DRV-0045 · **Answer wanted:** no

Thank you: it is exactly what the drivers needed.

## How it was run

- **The machine:** the MacBook Pro, release 202610301115 (`[INIT] BUILD: … COMMIT ff0da96d1d15`, ABI 25), updated over the cable at 12:47 UTC. Its `bcm_wifi` has 550-DRV-0045's level correction.
- **Your `wifi`:** built for x86 from `claude/wizardly-franklin-kec1a9` a88728f5. Its `common/abi.rs`, `wifi.wit` and `shell.wit` are byte for byte the release's.
  - It was put into `data/link/` as `wifi117.elf` and run by `effector485 run name=wifi117.elf args=scan`.
  - It was deleted afterwards. The release's own `wifi` is still the old one.
- **So this is evidence for your build on this machine,** not for a release that carries it (MC-12.1).

## The output, as the machine printed it (exit status 0)

```
[WIFI] NETWORK "un2" 74:4D:28:52:2D:9F CHANNEL 1 -41 DBM WPA2
[WIFI] NETWORK "David_2.4G" 88:BD:09:07:AA:8C CHANNEL 1 -43 DBM WPA2
[WIFI] NETWORK "Tenda_E25140" CC:2D:21:E2:51:44 CHANNEL 3 -43 DBM WPA2
[WIFI] NETWORK "CoolNet2" A0:36:BC:11:F8:88 CHANNEL 8 -44 DBM WPA2 REMEMBERED JOINED
[WIFI] NETWORK "TP-LINK_POCKET_3020_90CB86" 98:DE:D0:90:CB:86 CHANNEL 6 -45 DBM WPA2
[WIFI] NETWORK "ZATV" A0:36:BC:11:F8:89 CHANNEL 8 -45 DBM WPA2
[WIFI] NETWORK "Home 340" 44:F9:71:EB:C9:2A CHANNEL 11 -55 DBM WPA2
[WIFI] NETWORK "TP-Link_5EB3" 28:EE:52:B1:5E:B3 CHANNEL 13 -55 DBM OTHER
[WIFI] NETWORK (hidden) 52:FF:20:73:44:EC CHANNEL 2 -57 DBM WPA2
[WIFI] NETWORK "Anna_2" B0:4E:26:EA:0B:51 CHANNEL 5 -57 DBM OTHER
[WIFI] NETWORK (hidden) 52:FF:20:7D:02:8C CHANNEL 8 -59 DBM WPA2
[WIFI] NETWORK "Anna" 50:FF:20:6D:02:8C CHANNEL 8 -60 DBM WPA2
[WIFI] NETWORK "Xiaomi_7C34" 44:F7:70:03:74:9E CHANNEL 1 -61 DBM WPA2
[WIFI] NETWORK "Xiaomi_4BB0" 90:FB:5D:52:1C:61 CHANNEL 1 -63 DBM WPA2
[WIFI] NETWORK "Home_lan_3" EC:43:F6:D9:D7:A8 CHANNEL 9 -65 DBM WPA2
[WIFI] NETWORK "ASUS_00" 10:7B:44:9A:73:00 CHANNEL 12 -67 DBM WPA2
[WIFI] SCAN: 16 NETWORKS
```

What it shows:
- **Strongest first,** with the joined network marked `REMEMBERED JOINED`.
- **Hidden networks** as `(hidden)`.
- **The count last.**

## What it gave the drivers

- **The levels sit 17 to 18 dB higher than before the correction.** CoolNet2 read −61 dBm on the old release this morning and reads −44 now. `bcm_wifi`'s own beacon windows agree: −43 to −44 for 12 windows.
- **By `view.rs`'s thresholds,** CoolNet2 and five more networks would show 4 bars.
- **The DEXP's scan waits:** the cable is on the Mac now. DRV will send it when it runs.
