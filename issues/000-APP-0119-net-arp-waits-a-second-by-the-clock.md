# 000-APP-0119 — `net arp` waits a second by the clock

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P1 (a gate's failure) · **Status:** open (written; the `net` suite's run left) · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-12.2

**Recorded by:** the tools track, 2026-10-31, from KRN's request in `issues/requests-APP.md` (2026-10-30, as the landing queue's integrator) and its message `APP/2026-10-30T2156Z-KRN-a-request-net-arp-waits-a-second-by-the-clock.md`.

## Problem

- **What was seen:** round 57's gate, on `un-rizen` with a build on the machine's other lane. The `net` suite's `net arp 10.0.2.99` gave no prompt within the harness's 8 s, and the group failed. Alone it passed.
- **Why:** `arp` in `shell/src/net.rs` tried 100 times. Each try made a buffer call to the card's driver (`net::receive`, whose binding allocates a 4 KiB buffer) and slept 10 ms when no frame came. `NO ANSWER` took 1 s at least, and under load many times that.

## Plan

1. **The wait ends by the clock:** 1 s after the request (`ARP_WAIT_MS`, by `mind::time::uptime_ms`), however many tries fit.
2. **No buffer call while no frame is there.** The shell asks the driver's `net::wait`, which answers once a frame is waiting, bounded by the time left. Only then does it `receive`.
   - A driver that keeps no more waiters answers 0 at once; the frames are then looked for every 10 ms, still within the second.
   - **Not done:** one buffer for all the frames received. The generated binding allocates one per call, for each of `libmind/src/idl`'s 186 buffered calls; a variant taking the caller's buffer would be a change to `scripts/mind_idl.py` for all of them. With `wait`, an empty try makes no buffer call at all.
3. **The answers stay as they are:** `ARP 10.0.2.2 IS AT …`, `ARP 10.0.2.99: NO ANSWER`.

## Acceptance criteria

- The `net` suite's `net arp 10.0.2.2` and `net arp 10.0.2.99` answer as before, on x86 QEMU with 4 CPUs.
- `net arp 10.0.2.99` ends within its 8 s while the machine is loaded (a build beside it).

## Related

The gate's `x86: network, TLS and Effector`; [000-APP-0049](../issues-done/000-APP-0049-network-traffic-on-wms-background.done) (the card client).
