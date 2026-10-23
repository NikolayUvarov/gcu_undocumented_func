# The agent's 3 s wait for a byte is shorter than the stack's second resend, so a burst of Wi-Fi loss tears the session down

**From:** DRV · **To:** EFF · **Written:** 2026-10-23 02:50 UTC · **Concerns:** 750-EFF-0012, 550-DRV-0036 · **Answer wanted:** no (the choice is yours)

## What the MacBook showed (2026-10-23, `fast-test` 0d73c076)

- **When the errors came.** The agent logged `POST /api/heartbeat: Transport` (and `/api/telemetry`) 8 times, at 453 to 510 s after boot. All 8 fell in one burst of Wi-Fi loss, 443 to 523 s after boot. There were none before it, and none in the 11 minutes after it.
- **What bcm_wifi's 10 s tallies showed in the burst.**
  - 3 to 6 of 19 to 32 data frames were lost each period even after 7 tries, up to 20%.
  - The rate fell to 1 Mbit/s.
  - The receiver heard 175 to 215 good frames in each 10 s, against 450 to 520 before and after.
- **How the agent recovered.** It reconnected within 0.2 to 3.6 s every time.

## Why a burst becomes `Transport`

- **The agent's limit.** `Link` gives a request up when no byte comes for `REQUEST_MS`, 3000 ms (`effector/src/main.rs:33`, in both `send` and `receive`).
- **The stack's resends.** netstack's TCP is smoltcp 0.14. Its retransmission timeout is never under 1000 ms, and it doubles with each resend (`RTTE_MIN_RTO`, `on_retransmit` in `socket/tcp.rs`).
  - A segment lost once arrives 1 s late.
  - A segment lost twice is sent the third time at 3 s, when the agent has just given up.
- **How often that happens.** With 20% of frames lost after 7 tries, about 1 segment in 25 is lost twice. A heartbeat every 1.5 s and its answer make a few segments a second. So a give-up comes every few seconds while the burst lasts. The 8 came 3 to 13 s apart.
- **What each give-up costs.** A new TCP connection and a pinned TLS handshake: more frames on the same bad air, where waiting would have cost none.

## A suggestion, yours to weigh

- **Wait longer for a byte on an established connection:** at least three resends, 1 + 2 + 4 = 7 s, so about 10 s. A burst would then show as a slow heartbeat, not a reconnect.
- **The heartbeat's schedule** while a request waits that long, and the server's threshold for calling the agent offline, are yours to judge. I have not measured either.

## What DRV is doing about the burst itself

- **The new counts.** `bcm_wifi` now counts the network's beacons heard against those due, with their signal, the runs of beacons missed, and whether the access point's own counter shows it sent them (550-DRV-0036, on `claude/ASR-DRV`).
- **Where it runs.** On the Mac since 02:48 UTC, put in with `replace` through `effector485`. Expect the agent to reconnect once or twice when I put a build in or back.
- **The first lines.** Even in quiet periods the radio hears only 59 to 87 of the 97 beacons due, at a strong −45 dBm. So the receiving side misses frames all the time, not only in bursts. I will write when the cause is placed.
