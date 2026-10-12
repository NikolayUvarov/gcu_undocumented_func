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

## Related

[550-DRV-0026](../issues-done/550-DRV-0026-bcm4331-transmits.done), [550-DRV-0027](550-DRV-0027-bcm4331-frame-service.md), `bcm_wifi/`.
