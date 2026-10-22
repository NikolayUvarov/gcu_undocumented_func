# 550-DRV-0036 — `bcm_wifi` logs its traffic, so a lost ping can be placed

**Type:** driver · **Owner:** `DRV` · **Priority:** P1 · **Status:** in progress · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md), through [550-DRV-0006](550-DRV-0006-broadcom-wifi.md) · **Constitution:** MC-10.2, MC-12.1, MC-12.3

## Problem

On 2026-10-20 the MacBook Pro joined CoolNet2 by hand and got a DHCP lease ([550-ASR-0007](550-ASR-0007-wifi-join-tests.md)). The maintainer then found that `ping` mostly answers 2 of its 3 requests.

Nothing in the log says where the third goes. It could be:
- a data frame the access point never acknowledged within `wlan`'s 4 tries at one rate;
- an answer the radio did not receive. [550-DRV-0029](550-DRV-0029-bcm4331-receives-ofdm.md) found OFDM at 54 Mbit/s received in only 0 to 3 of 3 tries;
- an answer received and then lost above the driver.

`netstack` holds a request until ARP resolves rather than dropping it, so ARP alone is not the likely cause.

## Plan

1. **Every 10 s in which unicast data went out,** `bcm_wifi` logs one line:
   - **the data frames it sent:** acknowledged, acknowledged after a retry, lost; the tries; and by rate, sent and acknowledged;
   - **what came in meanwhile:** frames by modulation (CCK, OFDM, HT) and outcome (good, bad FCS, bad PLCP); queued for the station, filtered, bad, acknowledgements to us, DMA errors.

   With no traffic it says nothing, so an idle radio does not fill the log.
2. **A ping run on the MacBook Pro:** the log read over MIND Link (800-LNK-0002, 0003), and the loss placed on one side.
3. **The fix follows from it,** in this track or in `NET`'s `wlan` (its rates and tries), by request.

## Acceptance criteria

- [ ] On the MacBook Pro, a run of pings and the lines for it: the share of lost data frames, and of answers that never reached the host, at each rate.
- [ ] What the numbers show, recorded here, and the next task named.

Evidence for this machine only (MC-12.1). The line counts what the driver sees; it is not a measure of the air (MC-12.3).

## Progress

- **2026-10-20, written** (`bcm_wifi/src/serve.rs`, `Tally`). It builds. Only a real BCM4331 runs it: QEMU has no such device.
- **2026-10-21, the MacBook Pro** (`fast-test` b09b7bb, core 2's front end off), joined to CoolNet2 on channel 8. The lines were read over MIND Link. Seven periods of 10 s, from the join to the DHCP lease and after:
  - **Sent:** 1 to 6 data frames each period, at 24 and 36 Mbit/s, 16 of 19 acknowledged. 3 were lost after their 4 tries, 2 of them at 36 Mbit/s.
  - **Received:** 210 to 428 good CCK frames each period, and **not one good OFDM frame**. 2 to 8 OFDM frames came with a bad PLCP header each period. No HT frame came at all.
  - **DHCP took 50 s** from the join (911 s to 961 s).
- **What it shows:**
  - The radio does not receive OFDM from this access point. The answers it sends at OFDM rates are lost, and only those it retries at CCK rates arrive. That fits a ping answering 2 of 3.
  - Sending works.
  - Next: [550-DRV-0037](../issues-done/550-DRV-0037-802-11b-until-ofdm-is-received.done) offers the station 802.11b only, so the access point keeps to CCK. [550-DRV-0029](550-DRV-0029-bcm4331-receives-ofdm.md) goes on looking for why OFDM fails.
- **The line was cut:** a log record keeps 200 bytes, so the counts after `HT … BAD` were lost. They are now two lines, `SENT` and `RECEIVED`.
- **2026-10-22, the MacBook Pro under the Effector session: the lost data frames are the sending side's, and 7 tries keep them.**
  - **The build:** a `bcm_wifi` of the disk's release (50995da9, ABI 16), put in with `replace` through `effector485` on COM22 and put back after each run. It logged each lost data frame (time, length, rate, tries, the status word, the time since the last beacon), and every other 10 s gave unicast data up to 7 tries in place of `wlan`'s 4.
  - **Where the loss is.** Of 200 pings, 11 were lost, and 11 of the lost frames were 84-byte frames, a ping's request. So a lost ping is a request that never got out, not a reply that never came back.
  - **What a lost frame looks like.**
    - All 4 tries fail within 4 to 14 ms: status `00004001`, four tries and no ACK. The frames are protected data of every length, mostly at 11 Mbit/s.
    - The losses come in episodes of a few seconds, then quiet for a minute.
    - They are not tied to the access point's beacons: 6.8%, 2.7% and 6.1% lost at 0-10, 10-30 and 30-110 ms after one.
  - **Not measurable this way.** With control frames kept, the radio passes up no ACK addressed to us, not even for acknowledged frames. So whether the access point's ACK came and was missed cannot be counted.
  - **4 tries against up to 7, in alternate 10 s periods, two runs:**

| Tries | Periods | Data frames | Lost | Tries a frame |
|---|---|---|---|---|
| 4, as `wlan` asks | 27 | 909 | 73 (8.0%) | 1.62 |
| up to 7 | 16 | 922 | 7 (0.8%) | 1.63 |

  - **What it shows.**
    - An episode outlasts 4 quick tries but rarely 7: when 7 fail, they span 33 to 41 ms.
    - Tries a frame are the same, so 7 cost no airtime when the air is clear.
    - 7 is 802.11's default short retry limit.
  - **The next task:** `wlan`'s `ATTEMPTS` from 4 to 7 (`wlan/src/main.rs`), asked of `NET` on `internal-dev-messages`. This track's criterion, a run with the share lost at each rate, is met by these runs.
  - **Seen on the way.** After a `replace` of `bcm_wifi`, the restarted radio is on channel 1 while `wlan` still sends on the network's channel. Its first frames are lost at every rate until `wlan` gives the join up and joins again, about 30 s in all. That is the replace's own cost, not the losses above.

## Related

[550-DRV-0029](550-DRV-0029-bcm4331-receives-ofdm.md), [550-DRV-0028](550-DRV-0028-bcm4331-transmit-power.md), [550-ASR-0007](550-ASR-0007-wifi-join-tests.md), [550-DRV-0027](550-DRV-0027-bcm4331-frame-service.md).
