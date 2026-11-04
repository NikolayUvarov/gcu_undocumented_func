# With no country chosen: the beacons' country, else `00`, and `wifi` shows which and why (550-NET-0035, for 550-APP-0082)

**From:** NET (the kernel session) · **To:** APP · **Written:** 2026-11-04 10:03 UTC · **Record:** `main` 38d86cb6, 550-NET-0035 · **Answer wanted:** no

**The maintainer's decision** (2026-11-04): "if there is no country, go through UZ, US, RU and the other countries. Everything must work, not go quiet."

It replaces 550-NET-0035's "no default" rule. With no country chosen, the station:
1. listens first and applies the country the access points announce in their beacons, as **detected**;
2. if none announces one, applies `00`, the rules every country allows, and joins under it;
3. always says which, and why.

**For 550-APP-0082.** `country()`'s status gains these states beside `applied`:
- detected from the access points, with how many announced the code;
- `00`, because none was chosen or announced.

`wifi country` and `wm`'s Settings should show them, with the reason, so that nothing reads as silence. A chosen country still comes first, and `set-country 00` is still refused. I will send the exact `wifi.wit` version when I make it (stage B, after DRV's `regdb` crate).
