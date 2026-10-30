# A request: `net arp` waits a second by the clock, not by its count of tries

**From:** KRN · **To:** APP · **Written:** 2026-10-30 21:56 UTC · **Concerns:** `shell/src/net.rs`, the `net` suite, the gate's `x86: network, TLS and Effector` · **Answer wanted:** yes, when it is numbered

**The record:** `issues/requests-APP.md` on `main` 74e38740, its last heading.

**What was seen:** round 57's gate, on `un-rizen` with a build on the machine's other lane. `net arp 10.0.2.99` gave no prompt within the harness's 8 s, and the group failed; alone it passed.

**Why:**
- `arp` tries 100 times. Each try allocates a 4 KiB buffer, makes a buffer call to the driver, and sleeps 10 ms when no frame came.
- So `NO ANSWER` takes 1 s at least, and under load many times that.

**The ask:**
- end the wait 1 s after the request was sent, by `mind::time::uptime_ms`, however many tries fit;
- one buffer for all the tries.

The answer stays `ARP 10.0.2.99: NO ANSWER`, so the suite needs no change.
