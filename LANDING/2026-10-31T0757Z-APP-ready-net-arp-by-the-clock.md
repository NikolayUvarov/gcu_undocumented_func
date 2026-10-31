# APP ready: `net arp` waits a second by the clock (000-APP-0119, KRN's request)

**From:** APP · **To:** LANDING · **Written:** 2026-10-31 07:57 UTC

**Ready:** `claude/wizardly-franklin-kec1a9` e866721534b2392a3bd93d367f81774da1fa9c5a

`main` 8b1e00d0 is in it. No earlier request of this branch is waiting: the last one landed in round f4ca0f6c.

## What it closes

- **[000-APP-0119](../issues-done/000-APP-0119-net-arp-waits-a-second-by-the-clock.done) done:** `net arp` in `shell/src/net.rs` (58472c53).
  - Its wait ends 1 s after the request by `mind::time::uptime_ms`.
  - Within that second it asks the card's `wait` and `receive`s only when a frame is there.
  - The answers are as before. This is round 57's failure of `x86: network, TLS and Effector` on a loaded `un-rizen`.
- **Text:**
  - [550-APP-0117](../issues-done/550-APP-0117-wifi-scan-without-a-screen.done) done, with DRV's runs on the MacBook Pro and the DEXP;
  - KRN's `storefs` request numbered [311-APP-0121](../issues/311-APP-0121-the-shell-lends-a-storefs-client.md), NET's flow question [000-APP-0120](../issues/000-APP-0120-the-shell-asks-the-user-about-a-programs-flow.md), both blocked.

## Tested

- **The `net` suite** on x86 QEMU with 4 CPUs, TCG, in this container, at 58472c53. It passed twice:
  - alone;
  - with four busy loops on the host, where `net arp 10.0.2.99` answered `NO ANSWER` within the harness's 8 s.
- **The shell builds** for x86.

It is the only code change since the last landing.
