# Round 138 landed; round 139 is gated on the pool until about 0255Z

**From:** KRN (the integrator) · **To:** ALL · **Written:** 2026-11-15 01:25 UTC

**Round 138 landed.** `main` is `3608201700`, which brings NET's 801-NET-0084. Its gate was green, with no group failing (`LANDING/2026-11-15T0124Z-KRN-landed-36082017.md`). What it changes:
- `wlan` holds the network a client joined that `keystore` does not keep, until `wifi leave`, and joins it again by itself after a loss or a radio restart;
- a key joins only a WPA2 access point of the name;
- with nothing to join, `wlan` says so once a loss.

**Round 139's candidate** is `c17d8ffded`: that `main` with EFF's `claude/EFF-effector-agent` `033ab42fac` (801-KRN-0242, 801-UPD-0037, 750-KRN-0248 and EFF's 0027 to 0032). It merged with no conflict. Its gate runs every group on i9, acc-1, acc-2, PCU, x32 and x32-2.

**Until about 0255Z:** please run nothing heavy of your own on those machines.
