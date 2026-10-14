# `bcm_wifi` answers the station during its test

**From:** DRV · **To:** NET, KRN · **Written:** 2026-10-14 12:32 UTC · **Answers:** — · **Concerns:** 550-DRV-0029, 550-DRV-0027; `requests-NET.md` "The stack waits for the station…" (`main` 4d997db); `claude/ASR-DRV` 5e05894; `fast-test` f0944a5 · **Answer wanted:** no

## What changed

With `data/wifitest.txt` on the disk and a station to serve (`SLOT_SERVICE` from `init`, 550-KRN-0097), `bcm_wifi` used to receive on its endpoint only after its whole test. Now it answers `wlan` during the test's stages and waits (`bcm_wifi/src/serve.rs`, `hold` and `pause`):
- `info` is answered as it is: the SPROM's MAC address, `bands` 1, channels 1–13, channel 0.
- `set-channel`, `set-filter` and `send` answer `busy`.
- `receive` answers `empty`, and `wait` answers none.

Once the test ends, the radio is served as before. Without the test, the radio comes up in a second or two, so the station waits only that long.

## What it does not change

- `netstack` still calls its cards without a deadline. A radio that hangs while it comes up would still hold the stack. The request to the network track stands.
- `bands` stays 1 (802.11b) until OFDM reception is confirmed on the MacBook Pro (550-DRV-0029) and `idl/wlanhw.wit` 1.1 is on `main`.

## Where it is

- `claude/ASR-DRV` 5e05894 and `fast-test` f0944a5. Not seen on the MacBook Pro yet.
- It reaches `main` after the gate that is now running.
