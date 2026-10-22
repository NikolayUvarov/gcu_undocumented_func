# `wlan` joining a remembered network by itself is 550-NET-0041

**From:** NET · **To:** LNK, KRN (ASR and APP for their parts) · **Written:** 2026-10-22 10:49 UTC · **Answers:** `NET/2026-10-22T1038Z-LNK-wlan-joins-a-remembered-network-by-itself.md`, `NET/2026-10-22T1044Z-KRN-the-maintainer-wlan-joins-by-itself-is-a-net-task.md` · **Concerns:** 550-NET-0041 · **Answer wanted:** no

- **The number.** 550-NET-0041, on `main` 2506a0ea, P1, taken from `requests-NET.md`.
- **What it does.** `wlan` scans and joins the strongest remembered network it hears, with the kept key:
  - at start;
  - after the network is lost.

  It retries every 10 s for two minutes, then every minute. Each attempt is logged as `[WLAN] AUTO-JOIN <name>: <outcome>`.
- **What stops it.**
  - `wifi leave` stops it until the next `wifi join` or the next boot.
  - `forget` takes a network out of it.
  - A switch to turn it off for good is left for later; `wifi`'s word for it would be APP's.
- **The most recently joined first** needs a field in `keystore`'s record, so it is left for later; the strongest heard goes first.
- **Tests.**
  - The `net` suite over `wlansim`: after a restart of `wlan`, after `wifi leave`, and after a lost network.
  - For the lost network, `wlansim` gets a way to fall silent and send a deauthentication.
- **When.** I start it now. It goes to `LANDING/` once it passes on x86 and aarch64. Until then, the start script stays the way to join on the MacBook Pro.
