# 550-NET-0091 — The station's rate chosen by the airtime a delivered frame takes, from each try's outcome

**Type:** `dot11` (the station), `wlan` · **Owner:** `NET` (the kernel session) · **Priority:** P1 · **Status:** in progress (made, 2026-11-21) · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.3

## Problem

DRV's runs on the Ryzen on 2026-11-21 (`NET/2026-11-21T1213Z-DRV-what-the-ryzens-wi-fi-runs-showed-of-netstack-and-wlan.md` on `internal-dev-messages`) measured frames of 400 bytes and more:

| Rate | Tries not acknowledged |
|---|---|
| 54 Mb/s, RTS on | about 30 % |
| 24 Mb/s | 16 % |
| 12 Mb/s, no RTS | 13 % |

`wlan` sent nearly everything at 54 Mb/s. A failed try costs more than its own airtime: the radio tries its RTS up to its own limit before it gives the frame back.

**The cause.** `wlan` asks the radio for up to 15 tries a frame, so nearly every frame is acknowledged in the end. The station counted only that end:
- it stepped down after two frames that no try delivered, which almost never happens;
- it stepped up after ten acknowledged frames.

So it climbed to 54 Mb/s and stayed there.

## Plan

1. **`Station::sent(acked, attempts)`:** `wlan` passes the radio's `sent.attempts` too.
2. **Per rate, the success of one try,** averaged over windows of 20 unicast data frames. A rate's tries join its average once there are 4 of them.
3. **The rate chosen is the one whose delivered frame takes the least airtime:**
   - the airtime is that of 1500 bytes, plus 200 µs a try, plus 1000 µs for each failed try;
   - another rate is taken only when it saves a sixteenth;
   - a rate whose tries succeed less than 5 % of the time is never chosen.
4. **Samples:** every 8th frame goes at a neighbour of the chosen rate, faster and slower in turn, so that both stay measured.
5. **A frame that no try delivered** steps the rate down at once.
6. **Unchanged:** the start at 24 Mb/s; group frames at the chosen rate; management frames at the band's lowest rate.

The 200 µs and the 1000 µs are assumptions, to be set from DRV's runs.

## Acceptance criteria

- [ ] Host (`dot11/tests/station.rs`):
  - frames that each go at the first try climb to 54 Mb/s;
  - one that no try delivered steps the rate down at once;
  - an 802.11b station stays on its four rates;
  - with the Ryzen's figures (70 % a try at 54 Mb/s, 84 % at 24), 36 and 48 Mb/s carry most frames and 54 Mb/s at most a fifth, over four seeds.
- [ ] The `net` suite's Wi-Fi checks with `wlansim`, on both architectures, in the gate.
- [ ] **On the Ryzen** (DRV, MC-12.1): the rate `wifi status` shows, and the throughput of 6 MB from the LAN, against the figures of 2026-11-21.

## Related

[550-NET-0028](../issues-done/550-NET-0028-rates-the-radio-has.done) (the rates the radio has), [550-NET-0090](550-NET-0090-the-station-on-5-ghz.md), 213-DRV-0032.
