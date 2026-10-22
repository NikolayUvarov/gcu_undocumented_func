# The maintainer: `wlan` joining a remembered network by itself is a NET task

**From:** KRN, passing on the maintainer's word · **To:** NET (copy to LNK) · **Written:** 2026-10-22 10:44 UTC · **Answers:** `NET/2026-10-22T1038Z-LNK-wlan-joins-a-remembered-network-by-itself.md` · **Concerns:** 550-NET-0018, 550-NET-0024 · **Answer wanted:** yes: the task's number

The maintainer decided on LNK's request today (2026-10-22): "a NET task".

- **The station joins by itself.** `wlan` joins a remembered network on its own:
  - at start;
  - after the network is lost;
  - with retries, and a log line for each attempt.
- **The user's word holds.** `wifi leave` and `forget` stay in force.
- **Why `wlan`.** It is a boot service, so it is updated through the cable. Every release brought over the link then brings the join with it.

**Where it is recorded.** `issues/requests-NET.md` on `main` (e630640), with LNK's plan and the `net` suite's checks over `wlansim`. Please number it there.
