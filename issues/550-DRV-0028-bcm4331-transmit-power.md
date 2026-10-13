# 550-DRV-0028 — `bcm_wifi`: transmit power controlled from the SPROM, and answers on every channel

**Type:** driver · **Owner:** `DRV` · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md), through [550-DRV-0006](550-DRV-0006-broadcom-wifi.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.3

## Problem

Stage 5 ([550-DRV-0026](../issues-done/550-DRV-0026-bcm4331-transmits.done)) sent a probe request on each of channels 1 to 11 at a fixed, low gain (entry 80 of the gain table) with power control off. Every one was sent, and three probe responses came back on channel 1; none came on channels 2 to 11, where networks sent beacons at −39 to −54 dBm. A station needs its frames heard on any channel, at a power that is known and within what the SPROM allows.

## Plan

- **The SPROM's power data** (revision 9): the per-core 2.4 GHz maximum power (quarter dBm) and PA parameters at byte bases 0xC0, 0xE0 and 0x100, and the board flags, read and logged.
- **Power control as intended,** not as b43 writes it: the idle TSSI measured with sample playback, the 64-entry TSSI-to-power estimate per core from the PA parameters, the target power at most the SPROM's maximum, hardware power control on. The fact sheet's defects D1 to D8 in b43 are not copied.
- **The answers per channel:** the probe request's time and the wait for answers logged per channel; a second probe request after a longer wait; the gain or the target power raised in steps only while no answer comes and the SPROM's maximum is not reached.

## Acceptance criteria

On the MacBook Pro, one boot on request logs the SPROM's power data, the idle TSSI per core, power control on with its target, and probe responses on most channels with a network that answers probes. The power used never passes the SPROM's maximum. Evidence for this machine only (MC-12.1).

## Progress

**2026-10-13, the maintainer's run of `fast-test` 28d9c3d** (`log:boot0004.log`, stage 4 through the radio's calls of 550-DRV-0027):

| Test | Gain entry | Result |
|---|---|---|
| Broadcast probe requests, channels 1 to 11, at 1 and at 6 Mbit/s | 80 | all 22 sent once; no probe response |
| Directed probe requests, 7 tries each: `TP-LINK_POCKET` (channel 6, −38 dBm), `Xiaomi_7C34` (channel 6, −39 dBm), `David_2.4G` (channel 1, −41 dBm) | 80 | none acknowledged |
| The same | 40 (b43's) | `David_2.4G` acknowledged at the first try and answered; the two on channel 6 not acknowledged |

- **What it shows.** At entry 80 (`10090010`: the baseband multiplier at 0x10) the radio is too weak to be heard. At entry 40 (`10190033`) an access point on channel 1 hears it. Entry 40 is now the service's gain.
- **Still open.** The networks on channel 6 heard nothing, though they are the strongest and are received well there. This is the third run in which only channel 1 has answered.
- **The next run tells these apart:** the channel, the access point, the rate or the gain. It sends three directed probe requests to every network heard (channels 1, 3 and 6 here), at entries 40 and 20 and at 1 and 6 Mbit/s.

## Related

[550-DRV-0026](../issues-done/550-DRV-0026-bcm4331-transmits.done), [550-DRV-0027](550-DRV-0027-bcm4331-frame-service.md), `bcm_wifi/`.
