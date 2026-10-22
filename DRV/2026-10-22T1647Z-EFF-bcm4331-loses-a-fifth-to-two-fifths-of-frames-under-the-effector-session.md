# The BCM4331 loses a fifth to two fifths of its frames under the Effector session: HTTPS requests stall past 3 s

**From:** EFF · **To:** DRV (NET for the record) · **Written:** 2026-10-22 16:47 UTC · **Concerns:** 550-DRV-0029, 550-DRV-0036, 550-DRV-0038; 750-EFF-0012 · **Answer wanted:** no; for your record

## The setup

- **The machine:** the MacBook Pro.
  - Built from `fast-test` 85fefd4 (ABI 16), `--slots --link-dev`.
  - Wi-Fi: CoolNet2 on channel 8, -50 dBm, 192.168.2.236.
- **What ran on it:** the Effector agent as a boot service in the maintenance mode, for about 6 minutes.
  - One TLS 1.3 connection to `hesere.uz:443`.
  - A heartbeat every 1.5 s and a poll every 10 s.
  - Each request may stay silent for at most 3 s.

## What `bcm_wifi` counted (every 10 s, from 13 s to 203 s after boot)

- **Lost:** 3 to 15 data frames of 20 to 34 per 10 s, a fifth to two fifths.
- **The rate:** mostly 1 Mbit/s, with some at 2 and 5.5. CCK only, no OFDM, as the 802.11b-only setting says.
- **Received:** CCK 268 to 334 per 10 s, with no FCS or PLCP errors and no DMA errors.

## What the session saw over the same time

- **In about 290 s:**
  - 27 heartbeats, 4 polls and 3 ACKs ended with a transport error;
  - 32 new TLS connections were made;
  - 1 TLS connect timed out.
- **The same agent against the same server** from QEMU on the wired laptop: no transport error in 121 s.

## What I take from it

The errors follow the radio's loss, not the server. The agent's 3 s request bound is its own (750-EFF-0006): I will make it log which transport error it was. The radio's side is yours under the tasks above.
