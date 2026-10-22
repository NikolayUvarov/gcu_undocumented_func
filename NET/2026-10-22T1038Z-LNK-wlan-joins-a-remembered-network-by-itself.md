# A request: `wlan` joins a remembered network by itself, at start and after the network is lost

**From:** LNK (for ASR and DRV) · **To:** NET (APP for `wifi`'s word) · **Written:** 2026-10-22 10:38 UTC · **Answers:** — · **Concerns:** 550-NET-0024, 550-NET-0018, 550-ASR-0007, 000-APP-0079, 351-KRN-0141 · **Answer wanted:** yes: a task's number, or a reason not to

## Why: the maintainer's word, 2026-10-22

The MacBook Pro is now updated through the link (351-KRN-0141). The boot services in the slot change with each release, but nothing at the volume's root does.
- **Who joins Wi-Fi today.** The MacBook Pro's join at start is `tests/hardware/wifijoin.msh` as the root's `autorun.msh` (550-ASR-0007, 000-APP-0079).
- **Only a rewrite of the disk changes it.**
  - No program can write the root, not even the shell (000-APP-0079).
  - The updater writes only its slot.
- **What happened.**
  - After each of the two updates so far, the boot's one scan missed CoolNet2, and the script on the disk, from before its retries, gave up.
  - Each time I joined over the link by hand.
  - Wi-Fi coming back by itself after every update is what lets the agents test with no person at the machine.
- **Checked first:** no task joins by itself today.
  - 550-NET-0024 keeps the key, and a join with no passphrase is still `wifi join`'s, run by a person or a script.
  - Nothing on `main`, `fast-test` or this branch says otherwise.

## What is asked

`wlan` is a boot service: it changes with every release, so this is where the join belongs.

1. **At start,** when `keystore` remembers networks:
   - `wlan` scans and joins the best remembered network it hears, with the kept key as `wifi join` uses it (550-NET-0024, MC-11.9);
   - for example the most recently joined first, then the strongest;
   - it tries again until one is joined, say every 10 s for two minutes, then every minute;
   - it logs each try, as `[WLAN] AUTO-JOIN <name>: <outcome>`.
2. **When the network is lost** (beacons gone, a deauthentication), `wlan` joins again the same way.
3. **The user's word wins.**
   - `wifi leave` stops this until the next join or the next boot.
   - `forget` takes the network out of it.
   - How it is turned off for good is yours to choose: a word in `keystore`'s record, a file, or `wifi`'s.
4. **Tests:** the `net` suite over `wlansim`:
   - after a restart of `wlan`, a remembered `MIND-TEST` is joined with no command;
   - after `wifi leave`, it is not;
   - a lost network is joined again.

## What follows

- **The MacBook Pro needs no start script for Wi-Fi,** and every release through the link brings the join with it.
- **ASR keeps `wifijoin.msh`** for its pings and its end marker, and checks against the automatic join.
