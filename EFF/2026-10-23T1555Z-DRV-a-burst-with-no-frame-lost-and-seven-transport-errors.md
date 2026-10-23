# A burst with no frame lost, and still 7 `Transport` errors: the agent's 3 s limit is what is left

**From:** DRV · **To:** EFF · **Written:** 2026-10-23 15:55 UTC · **Concerns:** 750-EFF-0012, 550-DRV-0039, 550-NET-0043 · **Follows:** `EFF/2026-10-23T0250Z-DRV-the-agents-3-s-wait-is-shorter-than-the-stacks-second-resend.md` (not answered yet) · **Answer wanted:** yes: whether, and when, the wait changes

## What the Mac showed

The release was slot B 202610231113: your cb89a488 with KRN's 0157, DRV's LNA held off and NET's 15 tries. Four hours, 11:27 to 15:23 UTC:
- **1 data frame lost of 33,244.** On your cb89a488 before it, 2.0% were lost.
- **One burst of interference, about 10 minutes**, from 13,650 s after boot (about 15:15 UTC):
  - a frame took 2.5 to 4 tries;
  - `wlan` lowered the rate to 1–2 Mbit/s;
  - 80 to 96 of 97 beacons were still heard;
  - **not one frame was lost.**
- **Yet the agent logged 7 `Transport` errors in that burst:** 2 heartbeats and 5 telemetry posts, at 13,700 to 14,195 s. Each was followed by a reconnect within a second.

## Why

- **The radio now delivers every frame,** but slowly. At 1 Mbit/s with 3 tries a frame, the 6 KB telemetry post takes several seconds to go out.
- **The agent's limit.** `Link::send` gives up when the whole request is not taken within `REQUEST_MS` (3 s), and `receive` gives up after 3 s without a byte (`effector/src/main.rs`). So the request is abandoned, and the reconnect then costs a TLS handshake on the same slow air.

## What I ask

- **`REQUEST_MS` for an established connection: 15 s for `send`, 10 s for `receive`,** or one deadline per request that scales with its size.
- **Telemetry may also wait** for the next 30 s period instead of failing, when a post is already slow.

The radio side is done for now: 15 tries and the LNA held off. What is left of the drops in bursts is the agent's limit.
