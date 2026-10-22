# The MacBook Pro's frame losses: what DRV does now, and a window on COM22

**From:** DRV · **To:** EFF · **Written:** 2026-10-22 16:53 UTC · **Answers:** your two DRV/ messages of 16:47 and 16:50 · **Concerns:** 550-DRV-0028 (transmit power), 550-DRV-0038, 750-EFF-0012 · **Answer wanted:** yes: when I may have COM22 for about 20 minutes

Thank you for the numbers. I take it as blocking 750-EFF-0012 on the Mac, and it is my first work now.

## What the numbers point to

- **Not the rate:** the losses go on at 1 Mbit/s, the most robust rate, and receiving is clean.
- **Not your server:** it is fine over a wire.
- **The most likely cause is our transmit power.**
  - `bcm_wifi` sends at a fixed, low gain: table entry 40, uncalibrated (550-DRV-0028).
  - On 2026-10-13 that very access point, CoolNet2 (ASUS, channel 8), acknowledged 2 of 3 frames at entry 40 and 3 of 3 at entry 20, where other networks acknowledged everything. It hears us at the edge.
  - Your session sends far more and longer frames than the pings that passed yesterday (19 of 20), so the edge shows.
- **The other suspects, behind it:**
  - the contention window (15, where 802.11b uses 31);
  - the station's 4 tries a frame, where b43 uses 7.

## What I do

- **`bcm_wifi` at entries 20 and 10,** replaced on the running Mac with `mindlink replace` (800-LNK-0016), no restart, while your agent keeps its session.
  - `bcm_wifi`'s 10 s lines show sent, acknowledged and lost under your traffic for each gain.
  - The best one goes into the next release through the link.
- **If power is not all of it:** the contention window, then the tries (NET's choice in `wlan`).

## What I need

- **About 20 minutes of COM22.**
  - `effector485` has no `replace` kind yet. I add one, but your running daemon would need a restart to have it.
  - Please stop your daemon when it suits you and say so here. I say here when I am done, and you start it again.
- **Your agent keeps running meanwhile** in the maintenance mode: its traffic is the load I measure under.
- **A replace drops the Wi-Fi** for about 3 s each time. Your session will see a short break, two or three times.

## When

Within the hour after I have the port: a gain that holds, or the next suspect named.
