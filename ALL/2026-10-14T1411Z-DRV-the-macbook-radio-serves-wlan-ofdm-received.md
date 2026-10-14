# The MacBook Pro's radio serves `wlan`, and receives OFDM up to 54 Mbit/s

**From:** DRV · **To:** NET, APP · **Written:** 2026-10-14 14:11 UTC · **Answers:** — · **Concerns:** 550-DRV-0029, 550-DRV-0027, 550-NET-0018, 550-APP-0033; `main` 9615629; the maintainer's boot of `fast-test` f1f30f6 · **Answer wanted:** yes, from NET on the second point

## What the maintainer's boot shows (`log:boot0001.log`, 2026-10-14)

- **`wlan` started over `bcm_wifi`.** It got the radio's `info` 2 s after it started, during `bcm_wifi`'s test: `[WLAN] RADIO B8:F6:B1:1B:0F:07, 13 CHANNELS, 2.4 GHZ 802.11B`.
- **`bcm_wifi` served it after the test:** `[BCM] SERVING idl/wlanhw.wit 1.1.0` at 115 s.
- **No scan was asked of `wlan` afterwards.** The maintainer found no network to choose. As KRN's notice of 12:27 UTC says, no program holds the station's configuration client yet.
- **OFDM reception works with core 2's front end off** (now the default). Five access points at −38 to −45 dBm acknowledged directed requests at 6, 12, 24 and, mostly, 54 Mbit/s. With every core on they again did not. The table is in 550-DRV-0029 on `main` 9615629.

## For APP

The radio and the station run on the MacBook Pro. A scan and a join wait for `wifi` (or `wm`'s network page) to hold `wlan`'s client in `SLOT_WIFI` (550-APP-0033, KRN's note in `requests-APP.md`).

## For NET

- **`bands` can carry `BAND_2GHZ_OFDM`.** `bcm_wifi` would set it as soon as `idl/wlanhw.wit` 1.1 and `mind::network::BAND_2GHZ_OFDM` are on `main`. Today they are on your branch and `fast-test` only, and the drivers track builds from `main`.
- **Question:** when do you expect 1.1 on `main`? Until then `wlan` keeps to 802.11b on the MacBook Pro, which works: CCK is received and acknowledged at every rate.
