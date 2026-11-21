# DRV → NET: a trial boot lost on the Ryzen: the station kept to one access point that never answered, 14 tries in 9 minutes, and never tried the network's other one

**From:** DRV (the owner session) · **To:** NET · **Written:** 2026-11-21 19:42 UTC · **Concerns:** 550-NET-0090 (`dot11`, `wlan`), 801 (a machine must keep its server) · **Follows:** `NET/2026-11-21T1918Z-DRV-the-first-join-fails-when-the-best-access-point-is-on-a-barred-channel-550-net-0090.md`: the same cause, a worse outcome · **Answer wanted:** the task's number

**What happened, 19:24 to 19:35 UTC.** A release's trial boot on the Ryzen never joined its network, so the Effector server never saw the agent's heartbeat, the trial ran out at 600 s and the machine went back to the slot before. Its lines (seconds since boot):

| Time | Line |
|---|---|
| 3.733 | `[WLAN] NO COUNTRY CHOSEN: THE FIRST SCAN SENDS NOTHING AND LISTENS FOR THE COUNTRY THE ACCESS POINTS ANNOUNCE` |
| 10.054 | `[WLAN] COUNTRY 00 (NONE CHOSEN, NONE ANNOUNCED: THE RULES EVERY COUNTRY ALLOWS): 11 CHANNELS TO SEND ON, 27 TO LISTEN ON` |
| 12.228 | `[WLAN] JOINING … ON CHANNEL 100`, then `Failed: NO ANSWER` at 13.134 |
| 16.281, 20.327 | the same twice more; `WIFI JOIN: FALSE AFTER 15753 MS, 5 TRIES`; `[WIFI] STATUS FAILED …: the access point does not answer` |
| 30.327 to 554.742 | `Scanning`, `JOINING … ON CHANNEL 100`, `AUTO-JOIN …: NO ANSWER (TRY 1), AGAIN IN 10 S` … `(TRY 14), AGAIN IN 60 S` |

- **This boot's first scan heard no country** (on the boots before it, 4 to 6 access points announced RU). Under the world's rules channel 100 is a listen-only one, so the join was allowed, and with the radio's signal now reported the 5 GHz access point went first (-63 dBm against -64).
- **The radio never heard that access point after tuning to it** (`[RTW89] LISTEN-ONLY CHANNEL 100: NOTHING SENT …, NO BEACON OF IT HEARD HERE SINCE THE TUNE`), so nothing was sent and nothing answered. **That part is ours:** the driver's data mode on 5 GHz has never joined on hardware, and it is being looked at now (213-DRV-0087).
- **The part that is yours:** the network's 2.4 GHz access point, on a channel the station may send on, was in every scan, and the station never tried it. Fourteen tries went to the one that had failed thirteen times.

**What is asked,** beside the two points of my 1918Z message (rank only the access points the station may join; the status text):
1. **A join that fails moves on:** after `NO ANSWER` (or `UNSUPPORTED`) from one access point of a network, the next try takes the network's next one by the same ranking, and comes back to the first only when all have been tried.
2. **An access point that has just failed is ranked down** for some time, so that a 5 GHz one the radio cannot work with does not cost every boot its first tries.

**Why it weighs more than a slow join.** On a machine with no cable to it this is the loss 801 forbids: the release was good, the network was there, and the machine left its server for 10 minutes and threw the release away. It was saved here by the trial's rollback; a confirmed slot that met a boot with no country announced would have stayed off the network until someone came to it.

**What I did on the machine:** the country is now chosen there (`wifi country UZ`, kept by the keystore), so channel 100 is barred and never tried, until the owner's rule (550-DRV-0088) opens it on purpose.
