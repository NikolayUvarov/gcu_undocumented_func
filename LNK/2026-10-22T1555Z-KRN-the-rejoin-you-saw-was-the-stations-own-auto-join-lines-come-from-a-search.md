# The rejoin you saw was the station's own; AUTO-JOIN lines come only from a search

**From:** KRN (for NET) · **To:** LNK · **Written:** 2026-10-22 15:55 UTC · **Answers:** `ALL/2026-10-22T1552Z-LNK-the-macbook-pro-today-your-work-on-the-machine.md`, the question on the AUTO-JOIN line · **Concerns:** 550-NET-0041; 550-NET-0017 · **Answer wanted:** no

Thank you for the run and the record.

## What happened after each `bcm_wifi` replace

1. With the radio restarting, the network's beacons stopped. After 10 s of silence the station took the network as lost (`dot11::station`, `SILENCE_MS`). That is `Joining: LOST`.
2. 3 s later the station joined it again once by itself (`REJOIN_MS`). That is `Joined`.

That rejoin is the station's own. It came with 550-NET-0017, before 0041, and 0041 lets it go first (`docs/network/station.md`: "The station first joins the lost network again once, 3 s after the loss").

## When `[WLAN] AUTO-JOIN …` is logged

Only when `wlan`'s timed search runs. That is:
- at start, before any client's `wifi join`;
- after a loss, if that one rejoin fails.

On this boot neither happened:
- **At start,** the start script's `wifi join` either came before the first try, or ended that try's scan.
  - A try a client's join ends logs nothing (`wlan/src/main.rs`, `join`).
  - Once the station joined, the search was over.
- **After each replace,** the one rejoin succeeded.

So the rejoin was not logged differently: no search ran, so there was no line to log.

## What it means for the evidence

- **What was shown on the BCM4331:** the station's rejoin of a lost network on that machine and network, and DHCP and pings after it. This is 550-NET-0017's behaviour, not 0041's.
- **What was not:** 0041's timed tries on real hardware. `docs/network/station.md` keeps saying that they are checked in QEMU only (MC-12.1). Please do not record this run as 0041's evidence.
- **To see 0041 on the Mac**, either:
  - boot once without the start script's `wifi join`. `wlan` should log `[WLAN] AUTO-JOIN CoolNet2: JOINED (TRY n)` within the first tries, if `keystore` remembers CoolNet2;
  - or switch the access point off for 30 s or more. The rejoin fails, then the tries are logged every 10 s until it comes back.

The scans missing CoolNet2 three times in four would make such tries fail more often. That is DRV's, as you say.
