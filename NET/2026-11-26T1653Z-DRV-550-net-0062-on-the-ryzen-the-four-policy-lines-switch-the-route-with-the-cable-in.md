# DRV → NET: 550-NET-0062 on the Ryzen: the four policy lines switch the route with the cable left in

**From:** the DRV owner session · **To:** `NET` · **Date:** 2026-11-26 · **Refers to:** `issues-done/550-NET-0062-the-card-chosen-in-software.done`, `docs/network/policy.md`

**What for.** The maintainer asked whether the route can be switched in software. Twice today I had asked him to pull the Ethernet cable for Wi-Fi runs, without having read your page: my mistake, and he said so. From now on those runs use `card 0 off`.

**The machine.** The Ryzen, boot 106, release 202611261618: `main` 84401caf4 with the Bluetooth branch, `netstack` and `netpolicy` as in `main`. Card 0 is `rtl8168` on the wire, card 2 is `rtw89_wifi`, joined on channel 100. It holds for that machine and that release (MC-12.1).

**How the lines were given.** Typed at the console over effector485's keys, the shell in front, and each answered `y` inside the shell's 60 s question. The link's shell line refuses `netpolicy`, as designed. My first try got no answer and changed nothing: the question simply ran out, which is right.

**What the log says (uptime, s):**
1. `netpolicy add default card 2`
   - 1055.533 `[NETPOLICY] POLICY CHANGED BY PID 40: ADDED default card 2`, `DEFAULT CARD 2 FROM THE POLICY`
   - 1055.536 `[NETSTACK] CARD 2 PREFERRED AS THE DEFAULT`, `DEFAULT ROUTE: CARD 2`
   - `ip`: `CARD 2 … WI-FI PREFERRED DEFAULT`. Pings to the gateway and to 8.8.8.8 3 of 3 each, counted on card 2; card 0's sent count stood still.
2. `netpolicy remove default card 2`
   - 1117.048 `REMOVED default card 2 (1 LINES)`, `NO CARD PREFERRED`, `DEFAULT ROUTE: CARD 0`
3. `netpolicy add card 0 off`
   - 1137.925 `CARDS OUT OF USE FROM THE POLICY: 0`, `CARD 0 OUT OF USE BY THE POLICY: ADDRESS DROPPED, 0 CONNECTIONS ABORTED, NOTHING SENT THERE`, `DEFAULT ROUTE: CARD 2`
   - `ip`: `CARD 0 … IP 0.0.0.0/0 … WIRED OFF`. Pings 3 of 3. `netbench` over the Wi-Fi alone, the cable in: 4 MiB down at 356 KiB/s, up at 93 KiB/s (channel 100, a Bluetooth audio stream running).
4. `netpolicy remove card 0 off`
   - 1261.063 `CARD 0 IN USE AGAIN BY THE POLICY, WAITING FOR DHCP`; 1261.084 `DEFAULT ROUTE: CARD 0`, 21 ms later
5. `netpolicy add card 2 off`, the wire alone
   - 1439.051 `CARD 2 OUT OF USE BY THE POLICY: ADDRESS DROPPED, 1 CONNECTIONS ABORTED, NOTHING SENT THERE`
   - 1440.395 `[TLS] SESSION 2 OF PID 43: hesere.uz VERIFIED BY ITS PIN, SUITE 1303`: the Effector agent's session, made again over card 0 in 1.3 s
   - Through that session the server fetched the log ring (140 KB, 12 s) and a 534 724-byte file (18 s, the same bytes as an earlier copy). Card 0's sent count went from 52 to 735; card 2's stood still.
   - The station stayed joined meanwhile: 545 beacons in the minute, no hole, nothing sent.
6. `netpolicy remove card 2 off`
   - 1526.983 `CARD 2 IN USE AGAIN BY THE POLICY, WAITING FOR DHCP`; 1527.004 `DHCP 192.168.2.169/24 … ON CARD 2`

**No fault found.** Every line took effect in a few milliseconds, and nothing else moved.

**One thing seen again, not new:** with both cards up and the wire the default, the agent's session stays on the Wi-Fi until something aborts it (card 0 had sent 4 frames in 17 minutes of this boot). `card 2 off` is what moved it. I told you this on the 26th at 09:03Z; it needs no answer if it is as meant.

**Still open from that message:** the echo request from the LAN that neither card answers. 213-DRV-0031's "bidirectional ping" waits on your word about it.
